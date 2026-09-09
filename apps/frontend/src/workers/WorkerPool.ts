/**
 * WorkerPool - 共有Web Workerプール
 *
 * Issue 12修正: PostItemごとに独立Worker生成→共有プール化
 *
 * - CPUコア数に基づくプールサイズ
 * - Round-robinタスク配分
 * - 最大8ワーカー制限
 * - worker crash (onerror) 時: 当該 worker の pending タスクを reject し、
 *   worker を作り直す (旧実装はログのみで pending が永久に解決されなかった)
 * - terminate() 時: 全 pending タスクを reject (旧実装は clear のみで
 *   await 側が永久に固まっていた)
 * - タスクごとのタイムアウト (デフォルト 120s) で reject + クリーンアップ
 */

export interface WorkerPoolConfig {
  /** ワーカー数（デフォルト: navigator.hardwareConcurrency || 4） */
  size?: number;
  /** 最大ワーカー数（デフォルト: 8） */
  maxSize?: number;
  /** タスクごとのタイムアウト ms（デフォルト: 120_000） */
  taskTimeoutMs?: number;
}

export interface WorkerTask {
  id: string;
  type: string;
  payload: unknown;
}

export interface WorkerResult {
  id: string;
  success: boolean;
  result?: unknown;
  error?: string;
}

interface PendingTask {
  resolve: (value: unknown) => void;
  reject: (reason: Error) => void;
  /** どの worker に post したか (crash 時に当該 worker のタスクだけ reject する) */
  workerIndex: number;
  /** タスクタイムアウト用タイマー */
  timer: ReturnType<typeof setTimeout> | null;
}

/** デフォルトのタスクタイムアウト (ms)。 */
export const DEFAULT_TASK_TIMEOUT_MS = 120_000;

/**
 * 共有Web Workerプール
 */
export class WorkerPool {
  private workers: Worker[] = [];
  private pendingTasks: Map<string, PendingTask> = new Map();
  private currentWorkerIndex = 0;
  private idCounter = 0;
  private readyWorkers: Set<number> = new Set();
  private workerReadyPromises: Promise<void>[] = [];
  private taskTimeoutMs: number;
  private terminated = false;

  constructor(config: WorkerPoolConfig = {}) {
    this.taskTimeoutMs = config.taskTimeoutMs ?? DEFAULT_TASK_TIMEOUT_MS;

    // SSR/SSG環境では初期化しない
    if (typeof window === 'undefined') {
      return;
    }

    const defaultSize = typeof navigator !== 'undefined' ? navigator.hardwareConcurrency || 4 : 4;
    const maxSize = config.maxSize ?? 8;
    const size = Math.min(config.size ?? defaultSize, maxSize);

    // ワーカーを作成
    for (let i = 0; i < size; i++) {
      this.spawnWorker(i);
    }
  }

  /**
   * index 位置に worker を生成しハンドラを張る。
   * crash 後の作り直し (respawn) にも使う。
   */
  private spawnWorker(index: number): void {
    const worker = new Worker(
      new URL('./crypto.ts', import.meta.url),
      { type: 'module' }
    );

    // ready イベント用のPromise
    this.readyWorkers.delete(index);
    const readyPromise = new Promise<void>((resolve) => {
      const readyHandler = (event: MessageEvent) => {
        if (event.data.type === 'ready') {
          this.readyWorkers.add(index);
          worker.removeEventListener('message', readyHandler);
          resolve();
        }
      };
      worker.addEventListener('message', readyHandler);
    });
    this.workerReadyPromises[index] = readyPromise;

    // 通常のメッセージハンドラ
    worker.onmessage = (event) => {
      const data = event.data as WorkerResult | { type: string };

      // ready イベントは上のハンドラで処理済み
      if ('type' in data && data.type === 'ready') {
        return;
      }

      const result = data as WorkerResult;
      const pending = this.pendingTasks.get(result.id);
      if (pending) {
        this.clearPending(result.id, pending);
        if (result.success) {
          pending.resolve(result.result);
        } else {
          pending.reject(new Error(result.error || 'Unknown worker error'));
        }
      }
    };

    worker.onerror = (error) => {
      console.error(`[WorkerPool] Worker ${index} error:`, error);
      // crash した worker に post 済みの pending タスクは永久に応答しないので
      // ここで reject する。
      const message = (error as ErrorEvent)?.message || 'Worker crashed';
      this.rejectTasksForWorker(index, new Error(`[WorkerPool] Worker ${index} crashed: ${message}`));
      // worker を作り直して pool を回復させる (古いインスタンスは terminate)。
      if (!this.terminated && this.workers[index] === worker) {
        try {
          worker.terminate();
        } catch {
          // already dead
        }
        this.spawnWorker(index);
      }
    };

    this.workers[index] = worker;
  }

  /** 指定 worker 宛ての pending タスクをすべて reject する。 */
  private rejectTasksForWorker(workerIndex: number, reason: Error): void {
    for (const [id, pending] of Array.from(this.pendingTasks.entries())) {
      if (pending.workerIndex === workerIndex) {
        this.clearPending(id, pending);
        pending.reject(reason);
      }
    }
  }

  /** pending エントリとタイムアウトタイマーを破棄する。 */
  private clearPending(id: string, pending: PendingTask): void {
    if (pending.timer !== null) {
      clearTimeout(pending.timer);
      pending.timer = null;
    }
    this.pendingTasks.delete(id);
  }

  /**
   * 全ワーカーの準備完了を待つ
   */
  async waitUntilReady(): Promise<void> {
    await Promise.all(this.workerReadyPromises);
  }

  /**
   * ワーカー数を取得
   */
  get size(): number {
    return this.workers.length;
  }

  /**
   * 準備完了したワーカー数
   */
  get readyCount(): number {
    return this.readyWorkers.size;
  }

  /**
   * 全ワーカーが準備完了か
   */
  get isReady(): boolean {
    return this.readyWorkers.size === this.workers.length && this.workers.length > 0;
  }

  /**
   * Round-robinでワーカーを選択してタスクを実行
   */
  async execute<T = unknown>(type: string, payload: unknown): Promise<T> {
    // acquireWorker が空プール時に明示エラーを投げる
    const workerIndex = this.acquireWorker();

    // 全ワーカーが準備完了するまで待機
    await this.waitUntilReady();

    return this.executeOnWorker<T>(workerIndex, type, payload);
  }

  /**
   * 特定のワーカーでタスクを実行（セッションベースの操作用）
   * merkle_build と merkle_generate_proof など、
   * ワーカーローカルキャッシュに依存する操作で使用
   */
  async executeOnWorker<T = unknown>(
    workerIndex: number,
    type: string,
    payload: unknown,
    id?: string
  ): Promise<T> {
    if (this.workers.length === 0) {
      throw new Error('WorkerPool not initialized (no workers available)');
    }

    if (workerIndex < 0 || workerIndex >= this.workers.length) {
      throw new Error(`Invalid worker index: ${workerIndex}`);
    }

    await this.waitUntilReady();

    const taskId = id ?? `pool-${++this.idCounter}`;

    return new Promise<T>((resolve, reject) => {
      // タスクタイムアウト: worker が応答しないまま放置されるのを防ぐ
      const timer = this.taskTimeoutMs > 0
        ? setTimeout(() => {
            const pending = this.pendingTasks.get(taskId);
            if (pending) {
              this.clearPending(taskId, pending);
              pending.reject(new Error(
                `[WorkerPool] Task ${taskId} (${type}) timed out after ${this.taskTimeoutMs}ms`,
              ));
            }
          }, this.taskTimeoutMs)
        : null;

      this.pendingTasks.set(taskId, {
        resolve: resolve as (value: unknown) => void,
        reject,
        workerIndex,
        timer,
      });

      this.workers[workerIndex].postMessage({
        id: taskId,
        type,
        payload,
      });
    });
  }

  /**
   * Round-robinでワーカーを選択し、インデックスを返す
   * 後続の操作で同じワーカーを使用するため
   */
  acquireWorker(): number {
    if (this.workers.length === 0) {
      // 旧実装は length 0 のとき NaN index (0 % 0) を返していた
      throw new Error('WorkerPool not initialized (no workers available)');
    }
    const index = this.currentWorkerIndex;
    this.currentWorkerIndex = (this.currentWorkerIndex + 1) % this.workers.length;
    return index;
  }

  /**
   * 全ワーカーを終了。pending タスクはすべて reject される。
   */
  terminate(): void {
    this.terminated = true;
    for (const worker of this.workers) {
      worker.terminate();
    }
    this.workers = [];
    // 旧実装は clear のみで await 側が永久に解決されなかった
    for (const [id, pending] of Array.from(this.pendingTasks.entries())) {
      this.clearPending(id, pending);
      pending.reject(new Error('[WorkerPool] terminated'));
    }
    this.pendingTasks.clear();
    this.readyWorkers.clear();
    this.workerReadyPromises = [];
    this.currentWorkerIndex = 0;
  }
}

// シングルトンインスタンス（アプリ全体で共有）
let sharedPool: WorkerPool | null = null;

/**
 * 共有WorkerPoolインスタンスを取得（遅延初期化）
 */
export function getSharedWorkerPool(config?: WorkerPoolConfig): WorkerPool {
  if (!sharedPool) {
    sharedPool = new WorkerPool(config);
  }
  return sharedPool;
}

/**
 * 共有WorkerPoolを破棄（テスト用）
 */
export function resetSharedWorkerPool(): void {
  if (sharedPool) {
    sharedPool.terminate();
    sharedPool = null;
  }
}

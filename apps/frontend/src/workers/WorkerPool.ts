/**
 * WorkerPool - 共有Web Workerプール
 * 
 * Issue 12修正: PostItemごとに独立Worker生成→共有プール化
 * 
 * - CPUコア数に基づくプールサイズ
 * - Round-robinタスク配分
 * - 最大8ワーカー制限
 */

export interface WorkerPoolConfig {
  /** ワーカー数（デフォルト: navigator.hardwareConcurrency || 4） */
  size?: number;
  /** 最大ワーカー数（デフォルト: 8） */
  maxSize?: number;
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
}

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

  constructor(config: WorkerPoolConfig = {}) {
    // SSR/SSG環境では初期化しない
    if (typeof window === 'undefined') {
      return;
    }

    const defaultSize = typeof navigator !== 'undefined' ? navigator.hardwareConcurrency || 4 : 4;
    const maxSize = config.maxSize ?? 8;
    const size = Math.min(config.size ?? defaultSize, maxSize);

    // ワーカーを作成
    for (let i = 0; i < size; i++) {
      const worker = new Worker(
        new URL('./crypto.ts', import.meta.url),
        { type: 'module' }
      );

      // ready イベント用のPromise
      const readyPromise = new Promise<void>((resolve) => {
        const readyHandler = (event: MessageEvent) => {
          if (event.data.type === 'ready') {
            this.readyWorkers.add(i);
            worker.removeEventListener('message', readyHandler);
            resolve();
          }
        };
        worker.addEventListener('message', readyHandler);
      });
      this.workerReadyPromises.push(readyPromise);

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
          this.pendingTasks.delete(result.id);
          if (result.success) {
            pending.resolve(result.result);
          } else {
            pending.reject(new Error(result.error || 'Unknown worker error'));
          }
        }
      };

      worker.onerror = (error) => {
        console.error(`[WorkerPool] Worker ${i} error:`, error);
      };

      this.workers.push(worker);
    }
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
    if (this.workers.length === 0) {
      throw new Error('WorkerPool not initialized (no workers available)');
    }

    // 全ワーカーが準備完了するまで待機
    await this.waitUntilReady();

    const id = `pool-${++this.idCounter}`;
    const workerIndex = this.currentWorkerIndex;
    this.currentWorkerIndex = (this.currentWorkerIndex + 1) % this.workers.length;

    return this.executeOnWorker<T>(workerIndex, type, payload, id);
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
      this.pendingTasks.set(taskId, {
        resolve: resolve as (value: unknown) => void,
        reject,
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
    const index = this.currentWorkerIndex;
    this.currentWorkerIndex = (this.currentWorkerIndex + 1) % this.workers.length;
    return index;
  }

  /**
   * 全ワーカーを終了
   */
  terminate(): void {
    for (const worker of this.workers) {
      worker.terminate();
    }
    this.workers = [];
    this.pendingTasks.clear();
    this.readyWorkers.clear();
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

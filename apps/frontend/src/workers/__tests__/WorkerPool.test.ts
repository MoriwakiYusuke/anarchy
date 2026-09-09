/**
 * WorkerPool 単体テスト (code-review finding 5)。
 *
 * - 空プール時の明示エラー (旧実装は NaN index を返していた)
 * - terminate() で pending タスクが reject される (旧実装は永久に固まっていた)
 * - worker crash (onerror) で当該 worker の pending タスクが reject され、
 *   worker が作り直される
 * - タスクタイムアウトで reject + クリーンアップ
 *
 * jsdom に Worker は無いので MockWorker を global に注入する。
 */

import { WorkerPool } from '../WorkerPool';

/** WorkerPool が必要とする最小限の Worker mock。 */
class MockWorker {
  static instances: MockWorker[] = [];

  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: ((event: { message?: string }) => void) | null = null;
  posted: Array<{ id: string; type: string; payload: unknown }> = [];
  terminated = false;
  /** 'silent': タスクに応答しない / 'echo': payload をそのまま success で返す */
  static behavior: 'silent' | 'echo' = 'silent';

  private listeners: Array<(event: { data: unknown }) => void> = [];

  constructor(_url: unknown, _opts?: unknown) {
    MockWorker.instances.push(this);
    // 実 worker と同様、生成直後に非同期で ready を通知する
    queueMicrotask(() => this.emit({ type: 'ready' }));
  }

  addEventListener(_type: string, fn: (event: { data: unknown }) => void): void {
    this.listeners.push(fn);
  }

  removeEventListener(_type: string, fn: (event: { data: unknown }) => void): void {
    this.listeners = this.listeners.filter((l) => l !== fn);
  }

  postMessage(data: { id: string; type: string; payload: unknown }): void {
    this.posted.push(data);
    if (MockWorker.behavior === 'echo') {
      queueMicrotask(() =>
        this.emit({ id: data.id, success: true, result: data.payload }),
      );
    }
  }

  terminate(): void {
    this.terminated = true;
  }

  emit(data: unknown): void {
    const event = { data };
    for (const l of [...this.listeners]) l(event);
    this.onmessage?.(event);
  }

  emitError(message: string): void {
    this.onerror?.({ message });
  }
}

/** microtask / macrotask を flush する。 */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('WorkerPool', () => {
  beforeEach(() => {
    MockWorker.instances = [];
    MockWorker.behavior = 'silent';
    (globalThis as unknown as { Worker: unknown }).Worker = MockWorker;
  });

  afterEach(() => {
    delete (globalThis as unknown as { Worker?: unknown }).Worker;
  });

  it('空プールでは acquireWorker / execute が明示エラーを投げる', async () => {
    const pool = new WorkerPool({ size: 0 });
    expect(pool.size).toBe(0);
    expect(() => pool.acquireWorker()).toThrow('WorkerPool not initialized');
    await expect(pool.execute('anything', {})).rejects.toThrow(
      'WorkerPool not initialized',
    );
  });

  it('タスクが成功で resolve される (echo)', async () => {
    MockWorker.behavior = 'echo';
    const pool = new WorkerPool({ size: 1 });
    const result = await pool.execute<{ v: number }>('echo', { v: 42 });
    expect(result).toEqual({ v: 42 });
    pool.terminate();
  });

  it('terminate() で pending タスクが reject される', async () => {
    const pool = new WorkerPool({ size: 1 });
    const task = pool.execute('never-answered', {});
    // execute が waitUntilReady → postMessage まで進むのを待つ
    await flush();
    expect(MockWorker.instances[0].posted.length).toBe(1);

    pool.terminate();
    await expect(task).rejects.toThrow('[WorkerPool] terminated');
    expect(MockWorker.instances[0].terminated).toBe(true);
  });

  it('worker crash (onerror) で当該 worker の pending タスクが reject され respawn される', async () => {
    const pool = new WorkerPool({ size: 1 });
    const task = pool.execute('never-answered', {});
    await flush();
    expect(MockWorker.instances.length).toBe(1);

    MockWorker.instances[0].emitError('boom');
    await expect(task).rejects.toThrow('crashed');
    // 旧 worker は terminate され、新しい worker が作り直されている
    expect(MockWorker.instances[0].terminated).toBe(true);
    expect(MockWorker.instances.length).toBe(2);

    // respawn 後もタスクを実行できる
    await flush(); // 新 worker の ready を待つ
    MockWorker.behavior = 'echo';
    await expect(pool.execute('after-respawn', { ok: true })).resolves.toEqual({ ok: true });
    pool.terminate();
  });

  it('タスクタイムアウトで reject + pending がクリーンアップされる', async () => {
    const pool = new WorkerPool({ size: 1, taskTimeoutMs: 25 });
    const task = pool.execute('never-answered', {});
    await expect(task).rejects.toThrow('timed out after 25ms');

    // タイムアウト後に worker が遅れて応答しても何も起きない (cleanup 済み)
    const worker = MockWorker.instances[0];
    const posted = worker.posted[0];
    expect(() => worker.emit({ id: posted.id, success: true, result: 1 })).not.toThrow();
    pool.terminate();
  });

  it('round-robin: acquireWorker がインデックスを循環して返す', () => {
    const pool = new WorkerPool({ size: 2 });
    expect(pool.acquireWorker()).toBe(0);
    expect(pool.acquireWorker()).toBe(1);
    expect(pool.acquireWorker()).toBe(0);
    pool.terminate();
  });
});

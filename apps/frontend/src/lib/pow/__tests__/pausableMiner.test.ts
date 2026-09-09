/**
 * pausableMiner 単体テスト (code-review findings 1-2: Foreground PoW only)。
 *
 * - タブ隠蔽で worker が terminate される (旧実装は UI 表示だけで PoW が回り続けた)
 * - autoResume: false → PowPausedError(lastNonce) で reject (reaction 用)
 * - autoResume: true  → 復帰時に保存 nonce から新 worker で再開 (faucet 用)
 * - 隠蔽中に届いた solution は無視される (gate)
 */

import {
  minePow,
  PowPausedError,
  PowCancelledError,
} from '../pausableMiner';

class MockWorker {
  static instances: MockWorker[] = [];

  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: ((event: { message?: string }) => void) | null = null;
  posted: Array<Record<string, unknown>> = [];
  terminated = false;

  constructor() {
    MockWorker.instances.push(this);
  }

  postMessage(data: Record<string, unknown>): void {
    this.posted.push(data);
  }

  terminate(): void {
    this.terminated = true;
  }

  emit(data: unknown): void {
    this.onmessage?.({ data });
  }
}

interface VisibilityHarness {
  impl: { isHidden: () => boolean; addListener: (fn: () => void) => () => void };
  setHidden: (hidden: boolean) => void;
  removed: boolean;
}

function makeVisibility(): VisibilityHarness {
  let hidden = false;
  let listener: (() => void) | null = null;
  const harness: VisibilityHarness = {
    removed: false,
    impl: {
      isHidden: () => hidden,
      addListener: (fn) => {
        listener = fn;
        return () => {
          harness.removed = true;
        };
      },
    },
    setHidden: (h) => {
      hidden = h;
      listener?.();
    },
  };
  return harness;
}

const challenge = new Uint8Array(64).fill(7);

describe('minePow', () => {
  beforeEach(() => {
    MockWorker.instances = [];
  });

  it('ready で startNonce 付き start を送り、solution で resolve する', async () => {
    const vis = makeVisibility();
    const handle = minePow({
      createWorker: () => new MockWorker() as unknown as Worker,
      challenge,
      difficulty: 8,
      startNonce: 5n,
      autoResume: false,
      visibilityImpl: vis.impl,
    });

    const worker = MockWorker.instances[0];
    worker.emit({ type: 'ready' });
    expect(worker.posted[0]).toMatchObject({
      type: 'start',
      difficulty: 8,
      startNonce: 5n,
    });

    worker.emit({ type: 'solution', nonce: '777', hashRate: 100, elapsed: 250 });
    await expect(handle.promise).resolves.toEqual({
      nonce: 777n,
      hashRate: 100,
      elapsed: 250,
    });
    expect(worker.terminated).toBe(true);
    expect(vis.removed).toBe(true);
  });

  it('autoResume: false — タブ隠蔽で worker を terminate し PowPausedError(lastNonce) で reject する', async () => {
    const vis = makeVisibility();
    const onProgress = jest.fn();
    const handle = minePow({
      createWorker: () => new MockWorker() as unknown as Worker,
      challenge,
      difficulty: 8,
      autoResume: false,
      onProgress,
      visibilityImpl: vis.impl,
    });

    const worker = MockWorker.instances[0];
    worker.emit({ type: 'ready' });
    worker.emit({ type: 'progress', nonce: '123', hashRate: 10, elapsed: 50 });
    expect(onProgress).toHaveBeenCalledWith({ nonce: 123n, hashRate: 10, elapsed: 50 });

    vis.setHidden(true);
    // Foreground PoW only: worker は即 terminate される
    expect(worker.terminated).toBe(true);

    await expect(handle.promise).rejects.toMatchObject({
      name: 'PowPausedError',
      lastNonce: 123n,
    });
    await handle.promise.catch((e) => {
      expect(e).toBeInstanceOf(PowPausedError);
    });
  });

  it('autoResume: true — 復帰時に保存 nonce から新 worker で再開する', async () => {
    const vis = makeVisibility();
    const handle = minePow({
      createWorker: () => new MockWorker() as unknown as Worker,
      challenge,
      difficulty: 8,
      autoResume: true,
      visibilityImpl: vis.impl,
    });

    const worker1 = MockWorker.instances[0];
    worker1.emit({ type: 'ready' });
    worker1.emit({ type: 'progress', nonce: 50n, hashRate: 10, elapsed: 20 });

    vis.setHidden(true);
    expect(worker1.terminated).toBe(true);
    expect(MockWorker.instances.length).toBe(1);

    // 隠蔽中に worker1 から遅延 solution が届いても無視される (gate)
    worker1.emit({ type: 'solution', nonce: '999', hashRate: 1, elapsed: 1 });

    vis.setHidden(false);
    expect(MockWorker.instances.length).toBe(2);
    const worker2 = MockWorker.instances[1];
    worker2.emit({ type: 'ready' });
    // 保存済み nonce (50n) から再開
    expect(worker2.posted[0]).toMatchObject({ type: 'start', startNonce: 50n });

    worker2.emit({ type: 'solution', nonce: 70n, hashRate: 5, elapsed: 30 });
    await expect(handle.promise).resolves.toMatchObject({ nonce: 70n });
  });

  it('cancel() で worker を terminate し PowCancelledError で reject する', async () => {
    const vis = makeVisibility();
    const handle = minePow({
      createWorker: () => new MockWorker() as unknown as Worker,
      challenge,
      difficulty: 8,
      autoResume: true,
      visibilityImpl: vis.impl,
    });

    const worker = MockWorker.instances[0];
    worker.emit({ type: 'ready' });
    handle.cancel();
    expect(worker.terminated).toBe(true);
    await expect(handle.promise).rejects.toBeInstanceOf(PowCancelledError);
  });

  it('error メッセージで reject する', async () => {
    const vis = makeVisibility();
    const handle = minePow({
      createWorker: () => new MockWorker() as unknown as Worker,
      challenge,
      difficulty: 8,
      autoResume: false,
      visibilityImpl: vis.impl,
    });

    const worker = MockWorker.instances[0];
    worker.emit({ type: 'ready' });
    worker.emit({ type: 'error', message: 'boom' });
    await expect(handle.promise).rejects.toThrow('boom');
  });
});

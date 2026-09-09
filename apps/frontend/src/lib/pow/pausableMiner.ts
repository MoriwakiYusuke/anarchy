/**
 * Foreground-only PoW マイニングランナー (CLAUDE.md Security Principle #4)。
 *
 * 旧実装の問題: reaction / faucet の PoW worker は `while(true)` ループで
 * postMessage を受信できないため、タブが隠れても **worker が全速で回り続けて
 * いた** (Foreground PoW only 原則違反)。「pause」は UI 表示だけだった。
 *
 * 本ランナーは Page Visibility API でタブの隠蔽を検知したら **worker を
 * terminate して即座に計算を止め**、直近の progress nonce を保存する。
 * 復帰時はその nonce を `startNonce` として worker を作り直すので進捗は失わない。
 *
 * - `autoResume: true`  — タブ復帰時に自動で再開する (faucet 用)。
 * - `autoResume: false` — hide 時に `PowPausedError(lastNonce)` で reject する。
 *   呼び出し側が 'paused' 状態を表示し、ユーザー操作 (resume) で
 *   `startNonce: err.lastNonce` を渡して再度 mine する (reaction 用)。
 *
 * Worker プロトコル (faucet/worker.ts と reaction/miningWorker.ts 共通):
 *   in : { type:'start', challenge, difficulty, startNonce }
 *   out: { type:'ready' } | { type:'progress'|'solution', nonce, hashRate?, elapsed }
 *      | { type:'error', message }
 * nonce は bigint (faucet) / string (reaction) の両方が来るため BigInt() で正規化。
 * startNonce は bigint で送る (structured clone 対応、両 worker とも受理可能)。
 */

/** タブが隠れてマイニングを中断したことを表すエラー (autoResume: false 時)。 */
export class PowPausedError extends Error {
  /** 中断時点の nonce。resume 時に `startNonce` として渡す。 */
  readonly lastNonce: bigint;

  constructor(lastNonce: bigint) {
    super('PoW mining paused: page hidden');
    this.name = 'PowPausedError';
    this.lastNonce = lastNonce;
  }
}

/** cancel() によりマイニングが中止されたことを表すエラー。 */
export class PowCancelledError extends Error {
  constructor() {
    super('PoW mining cancelled');
    this.name = 'PowCancelledError';
  }
}

export interface PowProgress {
  nonce: bigint;
  hashRate: number;
  elapsed: number;
}

export interface PowSolution {
  nonce: bigint;
  hashRate: number;
  elapsed: number;
}

export interface MinePowOptions {
  /** Worker 生成関数。`new Worker(new URL(...))` は bundler の静的解析の都合で
   *  呼び出し側 (hooks) に置く必要があるため注入する。 */
  createWorker: () => Worker;
  challenge: Uint8Array;
  difficulty: number;
  /** 再開時の開始 nonce (デフォルト 0)。 */
  startNonce?: bigint;
  /** true: hide→terminate / visible→再開。false: hide→PowPausedError で reject。 */
  autoResume: boolean;
  onProgress?: (progress: PowProgress) => void;
  /** Page Visibility 注入 (テスト用)。 */
  visibilityImpl?: {
    isHidden: () => boolean;
    addListener: (fn: () => void) => () => void;
  };
}

export interface MinePowHandle {
  promise: Promise<PowSolution>;
  /** worker を terminate し、promise を PowCancelledError で reject する。 */
  cancel: () => void;
}

function defaultVisibility(): NonNullable<MinePowOptions['visibilityImpl']> {
  return {
    isHidden: () => typeof document !== 'undefined' && document.hidden,
    addListener: (fn) => {
      if (typeof document === 'undefined') return () => undefined;
      document.addEventListener('visibilitychange', fn);
      return () => document.removeEventListener('visibilitychange', fn);
    },
  };
}

/**
 * Foreground-only PoW マイニングを開始する。
 */
export function minePow(options: MinePowOptions): MinePowHandle {
  const visibility = options.visibilityImpl ?? defaultVisibility();

  let worker: Worker | null = null;
  let lastNonce: bigint = options.startNonce ?? 0n;
  let paused = false;
  let settled = false;
  let removeListener: () => void = () => undefined;

  let resolveFn: (s: PowSolution) => void = () => undefined;
  let rejectFn: (e: Error) => void = () => undefined;
  const promise = new Promise<PowSolution>((resolve, reject) => {
    resolveFn = resolve;
    rejectFn = reject;
  });

  const terminateWorker = (): void => {
    if (worker) {
      worker.terminate();
      worker = null;
    }
  };

  const settle = (fn: () => void): void => {
    if (settled) return;
    settled = true;
    removeListener();
    terminateWorker();
    fn();
  };

  const spawn = (startNonce: bigint): void => {
    const w = options.createWorker();
    worker = w;
    w.onmessage = (event: MessageEvent) => {
      const message = event.data as {
        type: string;
        nonce?: bigint | string;
        hashRate?: number;
        elapsed?: number;
        message?: string;
      };
      // terminate 済み / 一時停止中の遅延メッセージは無視する
      // (solution がタブ隠蔽中に処理されるのを防ぐゲート)。
      if (settled || paused || worker !== w) return;

      if (message.type === 'ready') {
        // startNonce は bigint で送る (structured clone 可、両 worker とも
        // BigInt() / そのまま利用のどちらでも受理できる)。
        w.postMessage({
          type: 'start',
          challenge: options.challenge,
          difficulty: options.difficulty,
          startNonce,
        });
        return;
      }
      if (message.type === 'progress') {
        lastNonce = BigInt(message.nonce ?? 0n);
        options.onProgress?.({
          nonce: lastNonce,
          hashRate: Number(message.hashRate ?? 0),
          elapsed: Number(message.elapsed ?? 0),
        });
        return;
      }
      if (message.type === 'solution') {
        const solution: PowSolution = {
          nonce: BigInt(message.nonce ?? 0n),
          hashRate: Number(message.hashRate ?? 0),
          elapsed: Number(message.elapsed ?? 0),
        };
        settle(() => resolveFn(solution));
        return;
      }
      if (message.type === 'error') {
        settle(() => rejectFn(new Error(message.message ?? 'PoW worker error')));
      }
    };
    w.onerror = (err: ErrorEvent) => {
      if (settled || worker !== w) return;
      settle(() => rejectFn(new Error(`PoW worker error: ${err.message ?? 'unknown'}`)));
    };
  };

  const onVisibilityChange = (): void => {
    if (settled) return;
    if (visibility.isHidden()) {
      if (paused) return;
      paused = true;
      // Foreground PoW only: タブが隠れたら worker ごと止める。
      terminateWorker();
      if (!options.autoResume) {
        // settled=true にして reject。paused フラグは settle 内では見ないが、
        // 呼び出し側が PowPausedError.lastNonce から再開できる。
        settle(() => rejectFn(new PowPausedError(lastNonce)));
      }
    } else if (paused && options.autoResume) {
      paused = false;
      spawn(lastNonce);
    }
  };

  removeListener = visibility.addListener(onVisibilityChange);

  // 開始時点で既に隠れている場合: foreground になるまで開始しない。
  if (visibility.isHidden()) {
    paused = true;
    if (!options.autoResume) {
      settle(() => rejectFn(new PowPausedError(lastNonce)));
    }
  } else {
    spawn(lastNonce);
  }

  return {
    promise,
    cancel: () => settle(() => rejectFn(new PowCancelledError())),
  };
}

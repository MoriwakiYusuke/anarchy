/**
 * DM scan loop controller (T046) — `scanDmInbox` を周期実行し、新着を `useDmStore`
 * へ反映する。フォアグラウンド 15 秒 / バックグラウンド 5 分 (Page Visibility API)。
 *
 * Contract: contracts/frontend-ui.md §1.5
 *
 * **MVP 実装メモ**: 仕様書は "Web Worker" を指定しているが、`scanDmInbox` は PAPI
 * の WebSocket クライアント (関数を含むため postMessage に乗らない) と wasm-engine
 * を要する。MVP では main thread の `setTimeout` ループで動作させ、後続フェーズで
 * dedicated Worker + comlink/serialize 化に置き換える想定。Page Visibility 対応と
 * `isScanning` フラグ反映は本実装で完結する。
 */

import type { ScanContext } from './scanner';
import { scanDmInbox } from './scanner';
import { useDmStore } from './store';

export const FOREGROUND_INTERVAL_MS = 15_000;
export const BACKGROUND_INTERVAL_MS = 5 * 60_000;

export interface DmScanLoopOptions {
  /** Scan 1 回ぶんを実行するための context 工場。呼び出し毎に最新の
   *  `lastScannedBlock` を反映するため、関数で受け取る (毎ループ再評価)。 */
  buildContext: () => ScanContext | null;
  /** 失敗時の handler。デフォルトは `console.error`。 */
  onError?: (err: unknown) => void;
  /** タイマー注入 (テスト用)。 */
  timerImpl?: {
    setTimeout: (fn: () => void, ms: number) => unknown;
    clearTimeout: (handle: unknown) => void;
  };
  /** Page Visibility 注入 (テスト用)。 */
  visibilityImpl?: {
    isHidden: () => boolean;
    addListener: (fn: () => void) => () => void;
  };
}

export interface DmScanLoopHandle {
  /** ループを停止し、登録した listener を解除する。 */
  stop: () => void;
  /** 現在の interval (ms)。テスト/UI 表示用。 */
  currentIntervalMs: () => number;
  /** 即座に 1 回スキャンを走らせる (visibility 切替時 などに利用)。 */
  triggerNow: () => void;
}

/**
 * `startDmScanLoop` — scan loop を起動する。
 *
 * - `buildContext()` が `null` を返したサイクルは skip (鍵未公開時)。
 * - `useDmStore` の `lastScannedBlock` / `isScanning` を更新し、新着 message を
 *   `addIncoming` で push する (FR-004 の signature_valid フィルタは scanner 側で
 *   既に適用済み)。
 */
export function startDmScanLoop(options: DmScanLoopOptions): DmScanLoopHandle {
  const timer = options.timerImpl ?? {
    setTimeout: (fn, ms) => globalThis.setTimeout(fn, ms),
    clearTimeout: (h) => globalThis.clearTimeout(h as ReturnType<typeof setTimeout>),
  };
  const visibility = options.visibilityImpl ?? defaultVisibility();
  const onError = options.onError ?? ((e) => console.error('[dm-scan]', e));

  let stopped = false;
  let pending: unknown = null;
  let inFlight = false;

  const currentIntervalMs = (): number =>
    visibility.isHidden() ? BACKGROUND_INTERVAL_MS : FOREGROUND_INTERVAL_MS;

  const runOnce = async (): Promise<void> => {
    if (stopped || inFlight) return;
    const ctx = options.buildContext();
    if (!ctx) return;
    inFlight = true;
    useDmStore.setState({ isScanning: true });
    try {
      const result = await scanDmInbox(ctx);
      const store = useDmStore.getState();
      for (const msg of result.newMessages) {
        store.addIncoming(msg);
      }
      if (result.scannedToBlock >= ctx.lastScannedBlock) {
        store.setLastScannedBlock(result.scannedToBlock);
      }
    } catch (err) {
      onError(err);
    } finally {
      inFlight = false;
      useDmStore.setState({ isScanning: false });
    }
  };

  const schedule = (): void => {
    if (stopped) return;
    pending = timer.setTimeout(async () => {
      await runOnce();
      schedule();
    }, currentIntervalMs());
  };

  // Page Visibility 切替時は pending を破棄して即時再スキャン → 新間隔で schedule。
  const removeListener = visibility.addListener(() => {
    if (stopped) return;
    if (pending) timer.clearTimeout(pending);
    pending = null;
    void runOnce().then(() => schedule());
  });

  // 初回は即時実行 → 以降 schedule。
  void runOnce().then(() => schedule());

  return {
    stop: () => {
      stopped = true;
      if (pending) timer.clearTimeout(pending);
      pending = null;
      removeListener();
    },
    currentIntervalMs,
    triggerNow: () => {
      if (pending) timer.clearTimeout(pending);
      pending = null;
      void runOnce().then(() => schedule());
    },
  };
}

function defaultVisibility(): NonNullable<DmScanLoopOptions['visibilityImpl']> {
  return {
    isHidden: () => typeof document !== 'undefined' && document.hidden,
    addListener: (fn) => {
      if (typeof document === 'undefined') return () => undefined;
      document.addEventListener('visibilitychange', fn);
      return () => document.removeEventListener('visibilitychange', fn);
    },
  };
}

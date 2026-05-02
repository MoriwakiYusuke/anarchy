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
import { useDmStore, receiptKey } from './store';
import type { DmMessageRecord } from './types';

export const FOREGROUND_INTERVAL_MS = 15_000;
export const BACKGROUND_INTERVAL_MS = 5 * 60_000;

export interface DmScanLoopOptions {
  /** Scan 1 回ぶんを実行するための context 工場。呼び出し毎に最新の
   *  `lastScannedBlock` を反映するため、関数で受け取る (毎ループ再評価)。 */
  buildContext: () => ScanContext | null;
  /** 新着 incoming message が入るごとに呼ばれる (FR-016a `delivered` 遷移用)。
   *  実装側で `sendDmReceipt({kind:'delivered'})` を発火する想定。副作用のみで
   *  戻り値は使わない。エラーは呼び出し側で握り潰して欲しい。 */
  onNewIncoming?: (msg: DmMessageRecord) => void;
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
        // FR-016a: incoming を store に入れたら、相手 (=counterparty) へ 'delivered' を返す。
        // idempotent: sentReceipts に記録済みなら skip (再スキャン / hot reload 耐性)。
        // ブロック中の相手には receipt を送らない (受信メタデータ漏洩 + MORAL 浪費を防ぐ)。
        if (options.onNewIncoming && msg.direction === 'incoming') {
          if (store.blockList.has(msg.counterparty)) continue;
          // GC placeholder (ciphertext を storage から再構成できなかった dispatch) は
          // counterparty が `gc:<hex>` という擬似値で SS58 ではない。受信内容も
          // 復号できていないので、相手に届いた事実すら確実ではない。receipt は出さない。
          if (msg.bodyState === 'garbage_collected') continue;
          const key = receiptKey(msg.counterparty, msg.messageId, 'delivered');
          if (!store.sentReceipts.has(key)) {
            store.rememberReceiptSent(key);
            try {
              options.onNewIncoming(msg);
            } catch (err) {
              onError(err);
            }
          }
        }
      }
      // T077/T078: receipt は送信側の outgoing 履歴に当たる。送信側 (= 自分) から見て
      // counterparty へ向けた outgoing message の deliveryState を前進させる。
      for (const r of result.newReceipts) {
        if (r.kind === 'delivered') {
          store.markAsDelivered(r.counterparty, r.refMessageId);
        } else {
          store.markAsRead(r.counterparty, r.refMessageId);
        }
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

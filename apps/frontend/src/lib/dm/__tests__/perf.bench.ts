/**
 * T084: DM 性能ベンチ (SC-004 / SC-005)。
 *
 * 目的: pallet ↔ scanner ↔ store の上に乗る純データ層が SC-004
 * (≤ 3 s で受信箱を構築できる) と SC-005 (10k メッセージ単一スレッドが
 * 実用的なレイテンシで挿入できる) の余裕を持って通ることを確認する。
 *
 * 注意:
 *  - これはエンドツーエンドのレンダリング時間ではなく、`useDmStore` の
 *    `addIncoming` / 並び替え / グルーピング コストを測る合成ハーネス。
 *    実際の inbox 表示までには PAPI scan + 復号 + React render が
 *    乗るが、それらはいずれもこの "下限" よりは重い処理ではないため、
 *    ここで余裕があれば SC-004 の 3 s 予算は十分賄える。
 *  - **`DM_PERF_BENCH=1` 時のみ実行する** (env-gated)。CI runner の
 *    クラス差や負荷ばらつきで wall-clock しきい値が flaky 化することを
 *    避けるため、デフォルト `pnpm test` ではスキップする。意図的にベンチを
 *    走らせる場合 (PR の SC モニタ更新時や local 計測) のみ環境変数で有効化。
 *  - ベンチ結果は console.info に出して、PR レビューや SC モニタで
 *    回帰を見つけられるようにする。
 *
 * 既知の制約 (T084):
 *  - 現在の `upsertMessage` は受信のたびに `messages.sort` を走らせるため
 *    挿入が O(n log n) per message → 単一スレッド 10k 投入は O(n² log n)。
 *    SC-005 を満たす範囲では実用上問題ないが、Polish 後段 (virtualization)
 *    に合わせて挿入アルゴリズムも O(n) on append に最適化したい。
 */

import { useDmStore } from '../store';
import type { AccountId, DmMessageRecord } from '../types';

function makeMessage(
  counterparty: string,
  blockNumber: bigint,
  messageId: bigint,
): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction: 'incoming',
    counterparty: counterparty as AccountId,
    timestampMs: Number(blockNumber) * 6000,
    body: new TextEncoder().encode(`m${messageId}`),
    bodyState: 'plaintext',
    signatureValid: true,
  };
}

function resetStore(): void {
  useDmStore.setState({
    conversations: new Map(),
    blockList: new Set(),
    lastScannedBlock: 0n,
    isScanning: false,
    receiptOptOut: false,
    sentReceipts: new Set<string>(),
  });
}

function elapsed<T>(label: string, fn: () => T): { result: T; ms: number } {
  const start = performance.now();
  const result = fn();
  const ms = performance.now() - start;
  // eslint-disable-next-line no-console
  console.info(`[dm-perf] ${label}: ${ms.toFixed(1)} ms`);
  return { result, ms };
}

// CI 上の wall-clock しきい値 flaky 回避のため、env-gated にする。
// 走らせるとき: `DM_PERF_BENCH=1 pnpm --filter anarchy-frontend test perf.bench`
const PERF_BENCH_ENABLED = process.env.DM_PERF_BENCH === '1';
const describeBench = PERF_BENCH_ENABLED ? describe : describe.skip;

describeBench('dmStore performance budgets (T084)', () => {
  beforeEach(() => {
    resetStore();
  });

  /**
   * SC-004: 受信箱が ≤ 3 s で構築できる。
   *
   * 1000 件の counterparty × 10 通 (= 10 000 件) を `addIncoming` で投入し、
   * 最後にスレッド一覧 (Map → Array → 並び替え) を作る。
   *
   * 予算: 3000 ms (SC-004 の "≤ 3 s" 一致)。データ層だけで 1 s 以下に
   * 収めたいので、警戒線として `expect(<3000)` を残し、実測は console に出す。
   */
  it('SC-004: builds 1k-conversation inbox (10k messages) under 3 s', () => {
    const { ms: insertMs } = elapsed('insert 1k×10', () => {
      const add = useDmStore.getState().addIncoming;
      for (let c = 0; c < 1000; c += 1) {
        const counterparty = `5Conv${c}`;
        for (let i = 0; i < 10; i += 1) {
          add(makeMessage(counterparty, BigInt(c * 100 + i), BigInt(c * 100 + i)));
        }
      }
    });

    const { result: list, ms: sortMs } = elapsed('sort inbox list', () => {
      const conversations = useDmStore.getState().conversations;
      return Array.from(conversations.values()).sort((a, b) => {
        if (a.lastActivityBlock === b.lastActivityBlock) return 0;
        return a.lastActivityBlock > b.lastActivityBlock ? -1 : 1;
      });
    });

    expect(list).toHaveLength(1000);
    expect(insertMs + sortMs).toBeLessThan(3000);
  });

  /**
   * SC-005: 単一スレッド 10k メッセージスクロール。
   *
   * ここでは React の DOM render 時間ではなく、データ層が 10k 件を保持できる
   * ことと、スクロール中に発生する追加挿入 (例: scanner からの新規メッセ)
   * が実用的に高速であることを保証する。
   *
   * 予算: 10000 ms (合成ハーネスの "上限"。実 UI では virtualization 前提
   * なので DOM コストは別途 polish フェーズで計測)。
   */
  it('SC-005: accumulates 10k messages into one thread under 10 s', () => {
    const counterparty = '5Heavy';
    const { ms } = elapsed('insert 10k single thread', () => {
      const add = useDmStore.getState().addIncoming;
      for (let i = 0; i < 10000; i += 1) {
        add(makeMessage(counterparty, BigInt(i), BigInt(i)));
      }
    });

    const conv = useDmStore.getState().conversations.get(counterparty as AccountId);
    expect(conv?.messages.length).toBe(10000);
    expect(ms).toBeLessThan(10000);
  });

  /**
   * SC-005 補助: 既に 10k 件入っているスレッドへの追加 1 件は < 50 ms。
   *
   * scanner が新着を流し込んだときの "追加コスト" の上限。実 UI では
   * これが React render の 1 frame に乗るので、データ層は 1 frame
   * (≈ 16 ms) よりは遅くても良いが、確実に upper-bound を持っておく。
   */
  it('SC-005: appending one message to a 10k-thread stays under 50 ms', () => {
    const counterparty = '5Heavy';
    const add = useDmStore.getState().addIncoming;
    for (let i = 0; i < 10000; i += 1) {
      add(makeMessage(counterparty, BigInt(i), BigInt(i)));
    }
    const { ms } = elapsed('append +1 to 10k-thread', () => {
      add(makeMessage(counterparty, 10000n, 10000n));
    });
    expect(ms).toBeLessThan(50);
  });
});

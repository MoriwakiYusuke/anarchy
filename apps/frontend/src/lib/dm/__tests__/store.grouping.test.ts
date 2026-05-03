/**
 * T054: dmStore グルーピング & 並び順テスト。
 *
 * Asserts:
 *   - 3 人の counterparty から DM が来ると 3 スレッドにグルーピングされる。
 *   - スレッド一覧 (ConversationList ソース) は最終アクティビティ block の降順。
 *   - 同 counterparty 内のメッセージは blockNumber 昇順 (時系列)。
 *
 * Contract: contracts/frontend-ui.md §1.4 / §2.1。
 */

import { useDmStore } from '../store';
import type { AccountId, DmMessageRecord } from '../types';

function msg(
  counterparty: string,
  blockNumber: bigint,
  messageId: bigint,
  body: string,
  direction: 'incoming' | 'outgoing' = 'incoming',
): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction,
    counterparty: counterparty as AccountId,
    timestampMs: Number(blockNumber) * 6000,
    body: new TextEncoder().encode(body),
    bodyState: 'plaintext',
    signatureValid: direction === 'incoming' ? true : undefined,
  };
}

beforeEach(() => {
  // Zustand v5 の create<T>(...) は reset API を持たないので手動で初期化。
  useDmStore.setState({
    conversations: new Map(),
    blockList: new Set(),
    lastScannedBlock: 0n,
    isScanning: false,
  });
});

describe('dmStore — conversation grouping (T054)', () => {
  it('groups 3 senders into 3 threads', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'hi from alice'));
    useDmStore.getState().addIncoming(msg('5Bob', 11n, 2n, 'hi from bob'));
    useDmStore.getState().addIncoming(msg('5Charlie', 12n, 3n, 'hi from charlie'));

    const conversations = useDmStore.getState().conversations;
    expect(conversations.size).toBe(3);
    expect(conversations.get('5Alice' as AccountId)?.messages).toHaveLength(1);
    expect(conversations.get('5Bob' as AccountId)?.messages).toHaveLength(1);
    expect(conversations.get('5Charlie' as AccountId)?.messages).toHaveLength(1);
  });

  it('sorts threads by lastActivityBlock descending when ConversationList derives from store', () => {
    // 古い順で受信 → スレッド一覧では最新が先頭。
    useDmStore.getState().addIncoming(msg('5Alice', 100n, 1n, 'old'));
    useDmStore.getState().addIncoming(msg('5Bob', 200n, 2n, 'mid'));
    useDmStore.getState().addIncoming(msg('5Charlie', 300n, 3n, 'newest'));

    const threads = Array.from(useDmStore.getState().conversations.values()).sort(
      (a, b) => Number(b.lastActivityBlock - a.lastActivityBlock),
    );
    expect(threads.map((t) => t.counterparty)).toEqual(['5Charlie', '5Bob', '5Alice']);
  });

  it('keeps messages within a thread chronologically (block asc, then messageId asc)', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 2n, 'second'));
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'first-same-block'));
    useDmStore.getState().addIncoming(msg('5Alice', 12n, 3n, 'third'));

    const conv = useDmStore.getState().conversations.get('5Alice' as AccountId);
    expect(conv?.messages.map((m) => Number(m.messageId))).toEqual([1, 2, 3]);
  });

  it('mixes incoming and outgoing in the same thread', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'in'));
    useDmStore.getState().addOutgoing(msg('5Alice', 11n, 2n, 'out', 'outgoing'));
    useDmStore.getState().addIncoming(msg('5Alice', 12n, 3n, 'in2'));

    const conv = useDmStore.getState().conversations.get('5Alice' as AccountId);
    expect(conv?.messages).toHaveLength(3);
    expect(conv?.messages.map((m) => m.direction)).toEqual([
      'incoming',
      'outgoing',
      'incoming',
    ]);
  });

  it('lastActivityBlock tracks the maximum block seen for the counterparty', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'a'));
    useDmStore.getState().addIncoming(msg('5Alice', 50n, 2n, 'b'));
    // 古い block を後から入れても lastActivityBlock は退行しない。
    useDmStore.getState().addIncoming(msg('5Alice', 5n, 3n, 'c'));

    const conv = useDmStore.getState().conversations.get('5Alice' as AccountId);
    expect(conv?.lastActivityBlock).toBe(50n);
  });
});

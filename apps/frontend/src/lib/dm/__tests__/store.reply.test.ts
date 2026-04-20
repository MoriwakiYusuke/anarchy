/**
 * T071: dmStore reply threading テスト (US3)。
 *
 * Asserts:
 *   - 受信済み counterparty への outgoing reply は同じ thread に attach される。
 *     (= conversations.size は増えない、messages は incoming + outgoing 両方含む)
 *   - lastActivityBlock は reply の block まで前進する。
 *   - 2 番目の counterparty へ送った場合は別 thread として分離される (sanity check)。
 *
 * Contract: spec.md FR-005 / FR-008 (thread grouping), contracts/frontend-ui.md §1.4。
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
    deliveryState: direction === 'outgoing' ? 'sent' : undefined,
  };
}

beforeEach(() => {
  useDmStore.setState({
    conversations: new Map(),
    blockList: new Set(),
    lastScannedBlock: 0n,
    isScanning: false,
  });
});

describe('dmStore — reply threading (T071 / US3)', () => {
  it('attaches an outgoing reply to the existing counterparty thread (no new thread)', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'hi from alice'));

    useDmStore
      .getState()
      .addOutgoing(msg('5Alice', 12n, 2n, 'reply from bob', 'outgoing'));

    const conversations = useDmStore.getState().conversations;
    expect(conversations.size).toBe(1);

    const conv = conversations.get('5Alice' as AccountId);
    expect(conv).toBeDefined();
    expect(conv?.messages).toHaveLength(2);
    expect(conv?.messages.map((m) => m.direction)).toEqual(['incoming', 'outgoing']);
    expect(conv?.lastActivityBlock).toBe(12n);
  });

  it('keeps replies chronological within the same thread across multiple turns', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'a1'));
    useDmStore.getState().addOutgoing(msg('5Alice', 11n, 2n, 'b1', 'outgoing'));
    useDmStore.getState().addIncoming(msg('5Alice', 12n, 3n, 'a2'));
    useDmStore.getState().addOutgoing(msg('5Alice', 13n, 4n, 'b2', 'outgoing'));

    const conv = useDmStore.getState().conversations.get('5Alice' as AccountId);
    expect(conv?.messages.map((m) => Number(m.messageId))).toEqual([1, 2, 3, 4]);
    expect(conv?.messages.map((m) => m.direction)).toEqual([
      'incoming',
      'outgoing',
      'incoming',
      'outgoing',
    ]);
  });

  it('does NOT increment unreadCount for outgoing replies', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'hi'));
    const unreadBefore =
      useDmStore.getState().conversations.get('5Alice' as AccountId)?.unreadCount ?? 0;

    useDmStore
      .getState()
      .addOutgoing(msg('5Alice', 11n, 2n, 'reply', 'outgoing'));

    const unreadAfter =
      useDmStore.getState().conversations.get('5Alice' as AccountId)?.unreadCount ?? 0;
    expect(unreadAfter).toBe(unreadBefore);
  });

  it('separates replies to a different counterparty into their own thread (sanity)', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'hi'));
    useDmStore
      .getState()
      .addOutgoing(msg('5Bob', 11n, 2n, 'hi bob', 'outgoing'));

    const conversations = useDmStore.getState().conversations;
    expect(conversations.size).toBe(2);
    expect(conversations.get('5Alice' as AccountId)?.messages).toHaveLength(1);
    expect(conversations.get('5Bob' as AccountId)?.messages).toHaveLength(1);
  });
});

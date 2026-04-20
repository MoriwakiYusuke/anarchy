/**
 * T075: dmStore.markAsDelivered / markAsRead 遷移テスト (US4 / FR-016a)。
 *
 * Asserts:
 *   - sent → delivered → read の一方向遷移のみ許す (退行しない)。
 *   - 同じ状態を再度当てても no-op (exactly once semantics)。
 *   - outgoing message にのみ deliveryState が更新される (incoming は不変)。
 *   - 存在しない messageId / counterparty は no-op。
 *   - setReceiptOptOut は state を切り替える (FR-016c)。
 *
 * Contract: spec.md FR-016a / FR-016c, data-model.md §2.4。
 */

import { useDmStore } from '../store';
import type { AccountId, DmMessageRecord } from '../types';

function outgoing(
  counterparty: string,
  blockNumber: bigint,
  messageId: bigint,
  body = 'hi',
): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction: 'outgoing',
    counterparty: counterparty as AccountId,
    timestampMs: Number(blockNumber) * 6000,
    body: new TextEncoder().encode(body),
    bodyState: 'plaintext',
    deliveryState: 'sent',
  };
}

function incoming(
  counterparty: string,
  blockNumber: bigint,
  messageId: bigint,
  body = 'hi',
): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction: 'incoming',
    counterparty: counterparty as AccountId,
    timestampMs: Number(blockNumber) * 6000,
    body: new TextEncoder().encode(body),
    bodyState: 'plaintext',
    signatureValid: true,
  };
}

beforeEach(() => {
  useDmStore.setState({
    conversations: new Map(),
    blockList: new Set(),
    lastScannedBlock: 0n,
    isScanning: false,
    receiptOptOut: false,
  });
});

describe('dmStore — delivery state transitions (T075 / US4)', () => {
  it('markAsDelivered advances sent → delivered', () => {
    useDmStore.getState().addOutgoing(outgoing('5Alice', 10n, 1n));

    useDmStore.getState().markAsDelivered('5Alice' as AccountId, 1n);

    const conv = useDmStore.getState().conversations.get('5Alice' as AccountId);
    expect(conv?.messages[0].deliveryState).toBe('delivered');
  });

  it('markAsRead advances delivered → read', () => {
    useDmStore.getState().addOutgoing(outgoing('5Alice', 10n, 1n));
    useDmStore.getState().markAsDelivered('5Alice' as AccountId, 1n);

    useDmStore.getState().markAsRead('5Alice' as AccountId, 1n);

    const conv = useDmStore.getState().conversations.get('5Alice' as AccountId);
    expect(conv?.messages[0].deliveryState).toBe('read');
  });

  it('markAsRead advances sent → read directly (delivered step can be skipped)', () => {
    useDmStore.getState().addOutgoing(outgoing('5Alice', 10n, 1n));

    useDmStore.getState().markAsRead('5Alice' as AccountId, 1n);

    const conv = useDmStore.getState().conversations.get('5Alice' as AccountId);
    expect(conv?.messages[0].deliveryState).toBe('read');
  });

  it('does NOT regress read → delivered (one-way only, FR-016a)', () => {
    useDmStore.getState().addOutgoing(outgoing('5Alice', 10n, 1n));
    useDmStore.getState().markAsRead('5Alice' as AccountId, 1n);

    useDmStore.getState().markAsDelivered('5Alice' as AccountId, 1n);

    const conv = useDmStore.getState().conversations.get('5Alice' as AccountId);
    expect(conv?.messages[0].deliveryState).toBe('read');
  });

  it('is a no-op when the same state is applied twice (exactly once)', () => {
    useDmStore.getState().addOutgoing(outgoing('5Alice', 10n, 1n));
    useDmStore.getState().markAsDelivered('5Alice' as AccountId, 1n);

    const snapshotBefore = useDmStore.getState().conversations;
    useDmStore.getState().markAsDelivered('5Alice' as AccountId, 1n);
    const snapshotAfter = useDmStore.getState().conversations;

    // state が変わらない場合、参照も変わらない (upsert しない) ことを要求する。
    expect(snapshotAfter).toBe(snapshotBefore);
  });

  it('leaves incoming messages unaffected (direction=outgoing guard)', () => {
    useDmStore.getState().addIncoming(incoming('5Alice', 10n, 1n));

    useDmStore.getState().markAsDelivered('5Alice' as AccountId, 1n);
    useDmStore.getState().markAsRead('5Alice' as AccountId, 1n);

    const conv = useDmStore.getState().conversations.get('5Alice' as AccountId);
    expect(conv?.messages[0].direction).toBe('incoming');
    expect(conv?.messages[0].deliveryState).toBeUndefined();
  });

  it('is a no-op for unknown counterparty or messageId', () => {
    useDmStore.getState().addOutgoing(outgoing('5Alice', 10n, 1n));
    const before = useDmStore.getState().conversations;

    useDmStore.getState().markAsDelivered('5NotExist' as AccountId, 1n);
    useDmStore.getState().markAsDelivered('5Alice' as AccountId, 999n);

    expect(useDmStore.getState().conversations).toBe(before);
  });

  it('advances only the targeted message within a thread', () => {
    useDmStore.getState().addOutgoing(outgoing('5Alice', 10n, 1n, 'first'));
    useDmStore.getState().addOutgoing(outgoing('5Alice', 11n, 2n, 'second'));

    useDmStore.getState().markAsDelivered('5Alice' as AccountId, 2n);

    const msgs = useDmStore.getState().conversations.get('5Alice' as AccountId)?.messages;
    expect(msgs?.find((m) => m.messageId === 1n)?.deliveryState).toBe('sent');
    expect(msgs?.find((m) => m.messageId === 2n)?.deliveryState).toBe('delivered');
  });
});

describe('dmStore — receipt opt-out setting (T075 / FR-016c)', () => {
  it('defaults to false', () => {
    expect(useDmStore.getState().receiptOptOut).toBe(false);
  });

  it('setReceiptOptOut toggles the flag', () => {
    useDmStore.getState().setReceiptOptOut(true);
    expect(useDmStore.getState().receiptOptOut).toBe(true);
    useDmStore.getState().setReceiptOptOut(false);
    expect(useDmStore.getState().receiptOptOut).toBe(false);
  });
});

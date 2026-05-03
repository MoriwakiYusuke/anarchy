/**
 * T055: dmStore.blockSender / unblockSender が ConversationList ソースを
 * フィルタすることを確認する (FR-011)。
 *
 * `<ConversationList />` 自体ではなく、UI が描画ソースとして使う「未ブロック
 * スレッドの配列」が正しく出ることを store 単体で検証する。
 */

import { useDmStore } from '../store';
import type { AccountId, ConversationState, DmMessageRecord } from '../types';

function msg(counterparty: string, blockNumber: bigint, messageId: bigint): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction: 'incoming',
    counterparty: counterparty as AccountId,
    timestampMs: Number(blockNumber) * 6000,
    body: new Uint8Array([0xab]),
    bodyState: 'plaintext',
    signatureValid: true,
  };
}

function visibleThreads(): ConversationState[] {
  const { conversations, blockList } = useDmStore.getState();
  return Array.from(conversations.values()).filter(
    (c) => !c.blocked && !blockList.has(c.counterparty),
  );
}

beforeEach(() => {
  useDmStore.setState({
    conversations: new Map(),
    blockList: new Set(),
    lastScannedBlock: 0n,
    isScanning: false,
  });
});

describe('dmStore.blockSender / unblockSender (T055 / FR-011)', () => {
  it('filters out a blocked counterparty from visible threads', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n));
    useDmStore.getState().addIncoming(msg('5Bob', 11n, 2n));

    expect(visibleThreads()).toHaveLength(2);

    useDmStore.getState().blockSender('5Bob' as AccountId);
    const threads = visibleThreads();
    expect(threads).toHaveLength(1);
    expect(threads[0].counterparty).toBe('5Alice');
  });

  it('does not delete underlying conversation data when blocking (per spec: hide-only)', () => {
    useDmStore.getState().addIncoming(msg('5Bob', 11n, 2n));
    useDmStore.getState().blockSender('5Bob' as AccountId);

    const raw = useDmStore.getState().conversations.get('5Bob' as AccountId);
    expect(raw).toBeDefined();
    expect(raw?.blocked).toBe(true);
    expect(raw?.messages).toHaveLength(1);
  });

  it('zeroes the unread badge of the blocked counterparty', () => {
    useDmStore.getState().addIncoming(msg('5Bob', 11n, 2n));
    expect(useDmStore.getState().conversations.get('5Bob' as AccountId)?.unreadCount).toBe(1);

    useDmStore.getState().blockSender('5Bob' as AccountId);
    expect(useDmStore.getState().conversations.get('5Bob' as AccountId)?.unreadCount).toBe(0);
  });

  it('does not increment unread for newly arriving messages from a blocked counterparty', () => {
    useDmStore.getState().blockSender('5Bob' as AccountId);
    useDmStore.getState().addIncoming(msg('5Bob', 12n, 3n));

    const conv = useDmStore.getState().conversations.get('5Bob' as AccountId);
    expect(conv?.blocked).toBe(true);
    expect(conv?.unreadCount).toBe(0);
    // FR-011: ブロック中は ConversationList から非表示。
    expect(visibleThreads()).toHaveLength(0);
  });

  it('restores visibility after unblockSender', () => {
    useDmStore.getState().addIncoming(msg('5Bob', 11n, 2n));
    useDmStore.getState().blockSender('5Bob' as AccountId);
    expect(visibleThreads()).toHaveLength(0);

    useDmStore.getState().unblockSender('5Bob' as AccountId);
    expect(visibleThreads()).toHaveLength(1);
    expect(useDmStore.getState().conversations.get('5Bob' as AccountId)?.blocked).toBe(false);
  });
});

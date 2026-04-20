/**
 * DM Zustand store skeleton (contracts/frontend-ui.md §1.4 / data-model.md §2.4)。
 *
 * Phase 2 では in-memory のみ。IndexedDB バック (T064) と Web Worker
 * scanner 連携 (T046) は後続フェーズで結線する。
 */

import { create } from 'zustand';
import type { AccountId, ConversationState, DmMessageRecord } from './types';

interface DmStoreState {
  conversations: Map<AccountId, ConversationState>;
  blockList: Set<AccountId>;
  lastScannedBlock: bigint;
  isScanning: boolean;

  /** FR-016c: 受信者が read receipt 送信を抑止する設定 (ローカルのみ)。 */
  receiptOptOut: boolean;

  /** 送信済み receipt を identify するキー集合 ("counterparty|messageId|kind")。
   *  T078: 同じ受信メッセージに対して receipt を重複送信しないための idempotent guard。 */
  sentReceipts: Set<string>;

  addIncoming: (message: DmMessageRecord) => void;
  addOutgoing: (message: DmMessageRecord) => void;
  markAsDelivered: (counterparty: AccountId, messageId: bigint) => void;
  markAsRead: (counterparty: AccountId, messageId: bigint) => void;
  blockSender: (account: AccountId) => void;
  unblockSender: (account: AccountId) => void;
  setLastScannedBlock: (block: bigint) => void;
  setIsScanning: (scanning: boolean) => void;
  setReceiptOptOut: (optOut: boolean) => void;
  rememberReceiptSent: (key: string) => void;
}

const cloneConversations = (
  conversations: Map<AccountId, ConversationState>,
): Map<AccountId, ConversationState> => new Map(conversations);

const upsertMessage = (
  conversations: Map<AccountId, ConversationState>,
  blockList: Set<AccountId>,
  message: DmMessageRecord,
): Map<AccountId, ConversationState> => {
  const next = cloneConversations(conversations);
  const counterparty = message.counterparty;
  const existing = next.get(counterparty);

  const messages = existing ? [...existing.messages, message] : [message];
  // 連続呼出での順序保持: blockNumber 昇順 → 同 block 内は messageId 昇順。
  messages.sort((a, b) => {
    if (a.blockNumber !== b.blockNumber) {
      return a.blockNumber < b.blockNumber ? -1 : 1;
    }
    if (a.messageId !== b.messageId) {
      return a.messageId < b.messageId ? -1 : 1;
    }
    return 0;
  });

  const blocked = existing?.blocked ?? blockList.has(counterparty);
  const incrementUnread = message.direction === 'incoming' && !blocked ? 1 : 0;

  next.set(counterparty, {
    counterparty,
    messages,
    unreadCount: (existing?.unreadCount ?? 0) + incrementUnread,
    blocked,
    lastActivityBlock:
      existing && existing.lastActivityBlock > message.blockNumber
        ? existing.lastActivityBlock
        : message.blockNumber,
  });
  return next;
};

/**
 * outgoing message の deliveryState を「前進のみ」更新する。
 *
 * FR-016a: 'sent' → 'delivered' → 'read' の一方向遷移。
 *  - 既に 'read' のものに 'delivered' を当てても退行させない。
 *  - 既に 'delivered' のものに 'delivered' を当てても no-op (exactly once)。
 */
const RANK: Record<'sent' | 'delivered' | 'read', number> = {
  sent: 0,
  delivered: 1,
  read: 2,
};

const advanceDeliveryState = (
  conversations: Map<AccountId, ConversationState>,
  counterparty: AccountId,
  messageId: bigint,
  target: 'delivered' | 'read',
): Map<AccountId, ConversationState> => {
  const conv = conversations.get(counterparty);
  if (!conv) return conversations;
  let changed = false;
  const messages = conv.messages.map((m) => {
    if (m.direction !== 'outgoing' || m.messageId !== messageId) return m;
    const current = m.deliveryState ?? 'sent';
    if (RANK[current] >= RANK[target]) return m;
    changed = true;
    return { ...m, deliveryState: target };
  });
  if (!changed) return conversations;
  const next = cloneConversations(conversations);
  next.set(counterparty, { ...conv, messages });
  return next;
};

export const useDmStore = create<DmStoreState>((set) => ({
  conversations: new Map(),
  blockList: new Set(),
  lastScannedBlock: 0n,
  isScanning: false,
  receiptOptOut: false,
  sentReceipts: new Set<string>(),

  addIncoming: (message) =>
    set((state) => ({
      conversations: upsertMessage(
        state.conversations,
        state.blockList,
        { ...message, direction: 'incoming' },
      ),
    })),

  addOutgoing: (message) =>
    set((state) => ({
      conversations: upsertMessage(
        state.conversations,
        state.blockList,
        { ...message, direction: 'outgoing' },
      ),
    })),

  markAsDelivered: (counterparty, messageId) =>
    set((state) => ({
      conversations: advanceDeliveryState(
        state.conversations,
        counterparty,
        messageId,
        'delivered',
      ),
    })),

  markAsRead: (counterparty, messageId) =>
    set((state) => {
      const advanced = advanceDeliveryState(
        state.conversations,
        counterparty,
        messageId,
        'read',
      );
      // 受信側が自分のスレッドを開いた際の既読クリアも従来通り実行。
      const conv = advanced.get(counterparty);
      if (!conv || conv.unreadCount === 0) return { conversations: advanced };
      const next = cloneConversations(advanced);
      next.set(counterparty, { ...conv, unreadCount: 0 });
      return { conversations: next };
    }),

  blockSender: (account) =>
    set((state) => {
      const blockList = new Set(state.blockList);
      blockList.add(account);
      const conversations = cloneConversations(state.conversations);
      const conv = conversations.get(account);
      if (conv) {
        conversations.set(account, { ...conv, blocked: true, unreadCount: 0 });
      }
      return { blockList, conversations };
    }),

  unblockSender: (account) =>
    set((state) => {
      const blockList = new Set(state.blockList);
      blockList.delete(account);
      const conversations = cloneConversations(state.conversations);
      const conv = conversations.get(account);
      if (conv) {
        conversations.set(account, { ...conv, blocked: false });
      }
      return { blockList, conversations };
    }),

  setLastScannedBlock: (block) => set({ lastScannedBlock: block }),
  setIsScanning: (scanning) => set({ isScanning: scanning }),
  setReceiptOptOut: (optOut) => set({ receiptOptOut: optOut }),
  rememberReceiptSent: (key) =>
    set((state) => {
      if (state.sentReceipts.has(key)) return state;
      const next = new Set(state.sentReceipts);
      next.add(key);
      return { sentReceipts: next };
    }),
}));

/**
 * T078: incoming message に対して送った receipt を identify するキー。
 *  "counterparty|messageId|kind" 形式。store.sentReceipts に蓄積して idempotent
 *  にする (ConversationView の再マウント / worker の scan リトライで再送しない)。
 */
export function receiptKey(
  counterparty: AccountId,
  messageId: bigint,
  kind: 'delivered' | 'read',
): string {
  return `${counterparty}|${messageId.toString()}|${kind}`;
}

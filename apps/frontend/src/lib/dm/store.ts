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

  addIncoming: (message: DmMessageRecord) => void;
  addOutgoing: (message: DmMessageRecord) => void;
  markAsRead: (counterparty: AccountId, messageId: bigint) => void;
  blockSender: (account: AccountId) => void;
  unblockSender: (account: AccountId) => void;
  setLastScannedBlock: (block: bigint) => void;
  setIsScanning: (scanning: boolean) => void;
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

export const useDmStore = create<DmStoreState>((set) => ({
  conversations: new Map(),
  blockList: new Set(),
  lastScannedBlock: 0n,
  isScanning: false,

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

  markAsRead: (counterparty, messageId) =>
    set((state) => {
      const conv = state.conversations.get(counterparty);
      if (!conv) return state;
      const messages = conv.messages.map((m) =>
        m.direction === 'outgoing' && m.messageId === messageId
          ? { ...m, deliveryState: 'read' as const }
          : m,
      );
      const next = cloneConversations(state.conversations);
      next.set(counterparty, {
        ...conv,
        messages,
        unreadCount: 0,
      });
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
}));

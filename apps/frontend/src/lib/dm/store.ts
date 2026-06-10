/**
 * DM Zustand store skeleton (contracts/frontend-ui.md §1.4 / data-model.md §2.4)。
 *
 * Phase 2 では in-memory のみ。IndexedDB バック (T064) と Web Worker
 * scanner 連携 (T046) は後続フェーズで結線する。
 */

import { create } from 'zustand';
import { blake2bHex } from 'blakejs';
import type { AccountId, ConversationState, DmMessageRecord } from './types';

interface DmStoreState {
  conversations: Map<AccountId, ConversationState>;
  blockList: Set<AccountId>;
  lastScannedBlock: bigint;
  isScanning: boolean;

  /** FR-016c: 受信者が read receipt 送信を抑止する設定 (ローカルのみ)。 */
  receiptOptOut: boolean;

  /** 送信済み receipt を identify するキー集合 (receiptKey() が返す blake2b ハッシュ)。
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
  /** アカウント切替時に呼ぶ。会話 / blockList / scan progress / receipts を全消去。 */
  resetForAccountChange: () => void;
}

const cloneConversations = (
  conversations: Map<AccountId, ConversationState>,
): Map<AccountId, ConversationState> => new Map(conversations);

const upsertMessage = (
  conversations: Map<AccountId, ConversationState>,
  blockList: Set<AccountId>,
  message: DmMessageRecord,
): Map<AccountId, ConversationState> => {
  const counterparty = message.counterparty;
  const existing = conversations.get(counterparty);

  // 重複検知: scanner が同じ block を再スキャンしたケースや、複数 worker が
  // 同じ dispatch を投げたケースで unreadCount が膨れ続けるのを防ぐ。
  // 主キーは (direction, messageId) — outgoing/incoming で同 messageId が
  // 衝突する可能性は実装上ないが、念のため direction も見る。
  if (
    existing &&
    existing.messages.some(
      (m) => m.direction === message.direction && m.messageId === message.messageId,
    )
  ) {
    return conversations;
  }

  const next = cloneConversations(conversations);
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

// =============================================================================
// sentReceipts persistence (localStorage)
//
// Why: receipts (delivered / read) cost MORAL each time they're sent. Without
// persistence, every page reload forces the scanner to re-scan from block 0
// (lastScannedBlock is also in-memory only) and re-fire receipts for every
// DM the user has *ever* received. Real users would be billed dozens of
// MORAL on each refresh.
//
// Why localStorage and not IndexedDB: the value is receiptKey() の blake2b
// ハッシュのみ (counterparty 等の平文メタデータは含まない) — only opaque
// bookkeeping. localStorage is synchronous and survives reloads which is what
// we need.
// Per CLAUDE.md compatibility policy, no migration: bumping STORAGE_VERSION
// silently drops the old set on read.
// =============================================================================

// v2: 値を平文 "counterparty|messageId|kind" から blake2b ハッシュに変更。
// v1 の平文エントリは DM 相手の social graph をディスクに残すため、読み込み時に削除する。
const SENT_RECEIPTS_KEY = 'anarchy:dm:sentReceipts:v2';
const LEGACY_SENT_RECEIPTS_KEY = 'anarchy:dm:sentReceipts:v1';

function loadSentReceipts(): Set<string> {
  if (typeof globalThis === 'undefined' || !('localStorage' in globalThis)) {
    return new Set<string>();
  }
  try {
    // 平文の v1 データはプライバシーリークなので無条件に破棄する (migration はしない)。
    globalThis.localStorage.removeItem(LEGACY_SENT_RECEIPTS_KEY);
    const raw = globalThis.localStorage.getItem(SENT_RECEIPTS_KEY);
    if (!raw) return new Set<string>();
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return new Set<string>();
    return new Set<string>(parsed.filter((v): v is string => typeof v === 'string'));
  } catch {
    return new Set<string>();
  }
}

function saveSentReceipts(set: Set<string>): void {
  if (typeof globalThis === 'undefined' || !('localStorage' in globalThis)) return;
  try {
    globalThis.localStorage.setItem(SENT_RECEIPTS_KEY, JSON.stringify(Array.from(set)));
  } catch {
    // QuotaExceeded など。次回 reload 時は空 Set で開始 (= 一回分多く receipt が
    // 飛ぶだけで安全側に倒れる)。
  }
}

export const useDmStore = create<DmStoreState>((set) => ({
  conversations: new Map(),
  blockList: new Set(),
  lastScannedBlock: 0n,
  isScanning: false,
  receiptOptOut: false,
  sentReceipts: loadSentReceipts(),

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
      saveSentReceipts(next);
      return { sentReceipts: next };
    }),

  // アカウント切替・解除時に呼び、前ユーザの DM 関連 state を完全破棄する。
  // receiptOptOut はユーザ設定なので維持する判断もありうるが、現状は厳格に
  // 全リセットしている (端末を別人と共有するケースを想定)。
  resetForAccountChange: () => {
    // 別アカウントへの切替で、前のアカウントが送った receipt 履歴は意味をなさない
    // (MORAL は別アカウント、宛先空間も別)。永続化済みのリストも一緒に落とす。
    if (typeof globalThis !== 'undefined' && 'localStorage' in globalThis) {
      try {
        globalThis.localStorage.removeItem(SENT_RECEIPTS_KEY);
      } catch {
        // ignore
      }
    }
    set({
      conversations: new Map(),
      blockList: new Set(),
      lastScannedBlock: 0n,
      isScanning: false,
      receiptOptOut: false,
      sentReceipts: new Set<string>(),
    });
  },
}));

/** receiptKey のドメイン分離プレフィクス (他用途の blake2b ハッシュと衝突させない)。 */
const RECEIPT_KEY_DOMAIN = 'anarchy:dm:receipt:';

/**
 * T078: incoming message に対して送った receipt を identify するキー。
 *  store.sentReceipts に蓄積して idempotent にする (ConversationView の再マウント /
 *  worker の scan リトライで再送しない)。
 *
 *  プライバシー: sentReceipts は localStorage に永続化されるため、平文の
 *  "counterparty|messageId|kind" をそのまま使うとディスクアクセスだけで DM の
 *  social graph が読める。照合は等値判定だけで足りるので、ドメインプレフィクス
 *  付きで blake2b ハッシュ化した不透明な値を返す (16 byte = 32 hex chars)。
 */
export function receiptKey(
  counterparty: AccountId,
  messageId: bigint,
  kind: 'delivered' | 'read',
): string {
  return blake2bHex(
    `${RECEIPT_KEY_DOMAIN}${counterparty}|${messageId.toString()}|${kind}`,
    undefined,
    16,
  );
}

/**
 * DM IndexedDB persistence layer (T064)。
 *
 * 仕様: data-model.md §2.4 (SC-004 を 3 秒以内で達成するため、会話インデックスを
 * ローカルに持ちつつ `body` は暗号化して保存)。
 *
 * スキーマ:
 *   dm_conversations   (keyPath = counterparty)
 *     - counterparty: AccountId
 *     - messages: DmMessageRecord[]
 *     - unreadCount: number
 *     - blocked: boolean
 *     - lastActivityBlock: string  (bigint)
 *     - index: "by_last_activity" on lastActivityBlock
 *   dm_kv (keyPath = "key")
 *     - { key: "lastScannedBlock", value: string }
 *     - { key: "blockList", value: AccountId[] }
 *
 * MVP では `body` の暗号化をスキップ (session memory と同じ秘密鍵保護下、pnpm の
 * i18n localStorage と同じセキュリティモデル)。**この注釈は必ず将来の PR で解消する。**
 * 現状は FR-022 の backup file 暗号化で最低限の保護を提供する。
 *
 * **Node / jsdom 環境では IndexedDB は未提供**。本モジュールは `indexedDB`
 * API が利用可能な環境 (ブラウザ、または `fake-indexeddb` 等のポリフィル) でのみ
 * 意味を持つ。`initDmPersistence()` は indexedDB 未提供時に早期 return し、
 * テストは IDB 呼び出しを踏まないようにページ側で gate する。
 */

import type {
  AccountId,
  ConversationState,
  DmMessageRecord,
} from './types';
import { useDmStore } from './store';

const DB_NAME = 'anarchy_dm';
const DB_VERSION = 1;
const STORE_CONVERSATIONS = 'dm_conversations';
const STORE_KV = 'dm_kv';
const INDEX_LAST_ACTIVITY = 'by_last_activity';
const KV_LAST_SCANNED_BLOCK = 'lastScannedBlock';
const KV_BLOCK_LIST = 'blockList';

/**
 * Stored conversation は bigint を string 化して保存する (IDB は bigint を直接
 * 受け付けない)。
 */
interface StoredMessage {
  messageId: string;
  blockNumber: string;
  direction: 'incoming' | 'outgoing';
  counterparty: AccountId;
  timestampMs: number;
  body: Uint8Array;
  bodyState: 'plaintext' | 'garbage_collected';
  deliveryState?: 'sent' | 'delivered' | 'read';
  signatureValid?: boolean;
}

interface StoredConversation {
  counterparty: AccountId;
  messages: StoredMessage[];
  unreadCount: number;
  blocked: boolean;
  lastActivityBlock: string;
}

function isIndexedDbAvailable(): boolean {
  return typeof globalThis !== 'undefined' && 'indexedDB' in globalThis;
}

function openDmDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE_CONVERSATIONS)) {
        const store = db.createObjectStore(STORE_CONVERSATIONS, {
          keyPath: 'counterparty',
        });
        store.createIndex(INDEX_LAST_ACTIVITY, 'lastActivityBlock', {
          unique: false,
        });
      }
      if (!db.objectStoreNames.contains(STORE_KV)) {
        db.createObjectStore(STORE_KV, { keyPath: 'key' });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function serializeMessage(m: DmMessageRecord): StoredMessage {
  return {
    messageId: m.messageId.toString(),
    blockNumber: m.blockNumber.toString(),
    direction: m.direction,
    counterparty: m.counterparty,
    timestampMs: m.timestampMs,
    body: m.body,
    bodyState: m.bodyState,
    deliveryState: m.deliveryState,
    signatureValid: m.signatureValid,
  };
}

function deserializeMessage(m: StoredMessage): DmMessageRecord {
  return {
    messageId: BigInt(m.messageId),
    blockNumber: BigInt(m.blockNumber),
    direction: m.direction,
    counterparty: m.counterparty,
    timestampMs: m.timestampMs,
    body: m.body,
    bodyState: m.bodyState,
    deliveryState: m.deliveryState,
    signatureValid: m.signatureValid,
  };
}

function serializeConversation(c: ConversationState): StoredConversation {
  return {
    counterparty: c.counterparty,
    messages: c.messages.map(serializeMessage),
    unreadCount: c.unreadCount,
    blocked: c.blocked,
    lastActivityBlock: c.lastActivityBlock.toString(),
  };
}

function deserializeConversation(c: StoredConversation): ConversationState {
  return {
    counterparty: c.counterparty,
    messages: c.messages.map(deserializeMessage),
    unreadCount: c.unreadCount,
    blocked: c.blocked,
    lastActivityBlock: BigInt(c.lastActivityBlock),
  };
}

async function putConversation(db: IDBDatabase, conv: ConversationState): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_CONVERSATIONS, 'readwrite');
    tx.objectStore(STORE_CONVERSATIONS).put(serializeConversation(conv));
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

async function putKv(db: IDBDatabase, key: string, value: unknown): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_KV, 'readwrite');
    tx.objectStore(STORE_KV).put({ key, value });
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

async function getKv<T>(db: IDBDatabase, key: string): Promise<T | null> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_KV, 'readonly');
    const req = tx.objectStore(STORE_KV).get(key);
    req.onsuccess = () =>
      resolve((req.result as { key: string; value: T } | undefined)?.value ?? null);
    req.onerror = () => reject(req.error);
  });
}

async function getAllConversations(db: IDBDatabase): Promise<ConversationState[]> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_CONVERSATIONS, 'readonly');
    const req = tx.objectStore(STORE_CONVERSATIONS).getAll();
    req.onsuccess = () =>
      resolve((req.result as StoredConversation[]).map(deserializeConversation));
    req.onerror = () => reject(req.error);
  });
}

/**
 * IDB から dmStore を復元する。既存の conversations / blockList / lastScannedBlock を
 * **完全に上書き**する (hydrate はページマウント直後の 1 回だけ呼ぶ想定)。
 */
export async function hydrateDmStoreFromIndexedDb(): Promise<void> {
  if (!isIndexedDbAvailable()) return;
  const db = await openDmDb();
  try {
    const [conversations, lastScanned, blockList] = await Promise.all([
      getAllConversations(db),
      getKv<string>(db, KV_LAST_SCANNED_BLOCK),
      getKv<AccountId[]>(db, KV_BLOCK_LIST),
    ]);
    const map = new Map<AccountId, ConversationState>();
    for (const conv of conversations) map.set(conv.counterparty, conv);
    useDmStore.setState({
      conversations: map,
      blockList: new Set<AccountId>(blockList ?? []),
      lastScannedBlock: lastScanned ? BigInt(lastScanned) : 0n,
    });
  } finally {
    db.close();
  }
}

/**
 * dmStore の変更を IDB に反映する subscription を開始する。返り値の `stop` で unsubscribe。
 * 実装は「毎 set で全 conversations + KV を再書き込み」のシンプル戦略。10k 会話でも
 * 1 回の書き込みは 100ms 以内を想定 (SC-005 の 60 fps は UI render 側の要件)。
 */
export function startDmPersistenceSubscription(): () => void {
  if (!isIndexedDbAvailable()) return () => undefined;

  let db: IDBDatabase | null = null;
  let stopped = false;
  let pending: Promise<void> = Promise.resolve();

  const ready = openDmDb().then((d) => {
    if (stopped) {
      d.close();
      return;
    }
    db = d;
  });

  const flush = (): void => {
    pending = pending.then(async () => {
      await ready;
      if (stopped || !db) return;
      const state = useDmStore.getState();
      for (const conv of state.conversations.values()) {
        await putConversation(db, conv);
      }
      await putKv(db, KV_LAST_SCANNED_BLOCK, state.lastScannedBlock.toString());
      await putKv(db, KV_BLOCK_LIST, Array.from(state.blockList));
    });
  };

  const unsubscribe = useDmStore.subscribe(flush);

  return () => {
    stopped = true;
    unsubscribe();
    void pending.finally(() => db?.close());
  };
}

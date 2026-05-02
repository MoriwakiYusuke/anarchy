/**
 * DM keyManager (T043 + T063)
 *
 * T043: Publish / revoke `DmReceptionKeys` via `pallet_messaging`.
 * T063 (US2): DM バックアップの暗号化エクスポート / 復号インポート + store マージ。
 *
 * 仕様: contracts/frontend-ui.md §1.3 / data-model.md §2.4 (FR-022)。
 *
 * バックアップ format (AES-GCM + PBKDF2-SHA256 100k):
 *   [0]                       version (u8 = 1)
 *   [1 .. 17]                 salt (16 B, PBKDF2)
 *   [17 .. 29]                iv (12 B, AES-GCM nonce)
 *   [29 ..]                   ciphertext (AES-GCM with 16 B tag)
 * Plaintext は {@link DmBackupJson} を UTF-8 JSON 化したもの。bigint は 10 進数文字列、
 * Uint8Array は hex 文字列にシリアライズ (JSON 非対応型の回避)。
 */

import type { Binary } from 'polkadot-api';
import type { PolkadotSigner } from 'polkadot-api/signer';
import { stealthKeyManager } from '../stealth/keyManager';
import { useDmStore } from './store';
import type {
  AccountId,
  ConversationState,
  DmMessageRecord,
  DmMetaAddress,
} from './types';
import { DmError } from './types';

/**
 * `pallet_messaging` の最小 PAPI 形状。型安全だが unsafeApi に被せる薄い shim。
 */
export interface MessagingApi {
  tx: {
    Messaging: {
      publish_dm_key: (params: {
        meta_address: { scan_pub: Binary; spend_pub: Binary };
      }) => {
        signAndSubmit: (signer: PolkadotSigner) => Promise<{ txHash: string }>;
      };
      revoke_dm_key: () => {
        signAndSubmit: (signer: PolkadotSigner) => Promise<{ txHash: string }>;
      };
    };
  };
}

/**
 * 既存 stealth 鍵から DmMetaAddress を取り出す。鍵がまだ生成/インポートされて
 * いない場合は `null`。
 */
export function getDmMetaAddressFromStealth(): DmMetaAddress | null {
  const scanPub = stealthKeyManager.getViewPubkey();
  const spendPub = stealthKeyManager.getSpendPubkey();
  if (!scanPub || !spendPub) {
    return null;
  }
  if (scanPub.length !== 32 || spendPub.length !== 32) {
    throw new Error(DmError.InvalidMetaAddress);
  }
  // X25519 view + Ed25519 spend を Uint8Array コピーで返す (元の鍵を保護)。
  return {
    scanPub: new Uint8Array(scanPub),
    spendPub: new Uint8Array(spendPub),
  };
}

/**
 * `pallet_messaging.publish_dm_key` を呼ぶ。既存 stealth 鍵が必要。
 *
 * 失敗モード:
 * - 鍵未生成: `Error('stealth key not loaded')`
 * - PAPI / signer の例外はそのまま再 throw (UI 側で handle)
 */
export async function publishDmKey(
  api: unknown,
  signer: PolkadotSigner,
): Promise<{ txHash: string; metaAddress: DmMetaAddress }> {
  if (!api) throw new Error('messaging api unavailable');
  if (!signer) throw new Error('signer unavailable');

  const meta = getDmMetaAddressFromStealth();
  if (!meta) throw new Error('stealth key not loaded');

  const { Binary } = await import('polkadot-api');
  const typed = api as MessagingApi;

  const tx = typed.tx.Messaging.publish_dm_key({
    meta_address: {
      scan_pub: Binary.fromBytes(meta.scanPub),
      spend_pub: Binary.fromBytes(meta.spendPub),
    },
  });
  const result = await tx.signAndSubmit(signer);
  return { txHash: result.txHash, metaAddress: meta };
}

/**
 * `pallet_messaging.revoke_dm_key` を呼ぶ。
 */
export async function revokeDmKey(
  api: unknown,
  signer: PolkadotSigner,
): Promise<{ txHash: string }> {
  if (!api) throw new Error('messaging api unavailable');
  if (!signer) throw new Error('signer unavailable');

  const typed = api as MessagingApi;
  const tx = typed.tx.Messaging.revoke_dm_key();
  const result = await tx.signAndSubmit(signer);
  return { txHash: result.txHash };
}

// =============================================================================
// T063 Backup export / import
// =============================================================================

const BACKUP_VERSION = 1;
// OWASP 2023 ASVS minimum for PBKDF2-SHA256 / 256-bit derived key.
// 旧バックアップ (100k iter) との互換性は CLAUDE.md "互換性方針" により破棄。
const PBKDF2_ITERATIONS = 600_000;
const SALT_BYTES = 16;
const IV_BYTES = 12;
const KEY_BITS = 256;
const HEADER_BYTES = 1 /* version */ + SALT_BYTES + IV_BYTES;

interface BackupMessage {
  messageId: string;
  blockNumber: string;
  direction: 'incoming' | 'outgoing';
  counterparty: string;
  timestampMs: number;
  body: string;
  bodyState: 'plaintext' | 'garbage_collected';
  deliveryState?: 'sent' | 'delivered' | 'read';
  signatureValid?: boolean;
}

interface BackupConversation {
  counterparty: string;
  messages: BackupMessage[];
  unreadCount: number;
  blocked: boolean;
  lastActivityBlock: string;
}

interface DmBackupJson {
  version: 1;
  dm_meta: {
    scan_priv: string;
    spend_priv: string;
    spend_pub: string;
    published_at_block: string;
  };
  dm_scan_index: {
    lastScannedBlock: string;
    conversations: BackupConversation[];
  };
  dm_block_list: {
    version: 1;
    entries: string[];
  };
  /** T079 / FR-016c: read-receipt opt-out 設定をデバイス間で引き継ぐ。
   *  旧バックアップ (undefined) は false 扱い。 */
  receipt_opt_out?: boolean;
  exported_at_ms: number;
  device_id: string;
}

function toHex(b: Uint8Array): string {
  let s = '';
  for (let i = 0; i < b.length; i += 1) s += b[i].toString(16).padStart(2, '0');
  return s;
}

function fromHex(s: string): Uint8Array {
  if (s.length % 2 !== 0) throw new Error('invalid hex length');
  const out = new Uint8Array(s.length / 2);
  for (let i = 0; i < out.length; i += 1) {
    out[i] = parseInt(s.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

function serializeMessage(m: DmMessageRecord): BackupMessage {
  return {
    messageId: m.messageId.toString(),
    blockNumber: m.blockNumber.toString(),
    direction: m.direction,
    counterparty: m.counterparty,
    timestampMs: m.timestampMs,
    body: toHex(m.body),
    bodyState: m.bodyState,
    deliveryState: m.deliveryState,
    signatureValid: m.signatureValid,
  };
}

function deserializeMessage(m: BackupMessage): DmMessageRecord {
  return {
    messageId: BigInt(m.messageId),
    blockNumber: BigInt(m.blockNumber),
    direction: m.direction,
    counterparty: m.counterparty as AccountId,
    timestampMs: m.timestampMs,
    body: fromHex(m.body),
    bodyState: m.bodyState,
    deliveryState: m.deliveryState,
    signatureValid: m.signatureValid,
  };
}

async function deriveAesKey(password: string, salt: Uint8Array): Promise<CryptoKey> {
  const encoder = new TextEncoder();
  const keyMaterial = await crypto.subtle.importKey(
    'raw',
    encoder.encode(password),
    { name: 'PBKDF2' },
    false,
    ['deriveKey'],
  );
  return crypto.subtle.deriveKey(
    {
      name: 'PBKDF2',
      salt: salt as BufferSource,
      iterations: PBKDF2_ITERATIONS,
      hash: 'SHA-256',
    },
    keyMaterial,
    { name: 'AES-GCM', length: KEY_BITS },
    false,
    ['encrypt', 'decrypt'],
  );
}

function generateDeviceId(): string {
  const c = crypto as unknown as { randomUUID?: () => string };
  if (typeof c.randomUUID === 'function') return c.randomUUID();
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return toHex(bytes);
}

/**
 * `stealthKeyManager.loadFromBackup` が実装されていれば DM backup の private key を
 * 流し込む。T065-T067 で StealthKeyManager 側にも実装を追加する想定。存在しない場合は
 * 黙って no-op (テスト mock / MVP ルート前提)。
 */
function loadKeysIntoStealth(scanPriv: Uint8Array, spendPriv: Uint8Array): void {
  const km = stealthKeyManager as unknown as {
    loadFromBackup?: (scanPriv: Uint8Array, spendPriv: Uint8Array) => void;
  };
  if (typeof km.loadFromBackup === 'function') {
    km.loadFromBackup(scanPriv, spendPriv);
  }
}

/**
 * 現在の dmStore 状態 + stealth 秘密鍵を AES-GCM + PBKDF2 で暗号化した backup ファイルを
 * 返す。返値はそのまま `File` / `Blob` としてユーザーにダウンロードさせる前提。
 *
 * 失敗モード:
 * - stealth 鍵が未ロード: `Error('stealth key not loaded')` (呼び出し前に gate する)。
 * - WebCrypto が使えない環境: subtle が存在しない場合 throw (実質 node test でのみ発生)。
 */
export async function exportDmBackup(password: string): Promise<Uint8Array> {
  const scanPriv = stealthKeyManager.getViewKey();
  const spendPriv = stealthKeyManager.getSpendKey();
  const spendPub = stealthKeyManager.getSpendPubkey();
  if (!scanPriv || !spendPriv || !spendPub) {
    throw new Error('stealth key not loaded');
  }

  const state = useDmStore.getState();
  const conversations: BackupConversation[] = [];
  state.conversations.forEach((conv) => {
    conversations.push({
      counterparty: conv.counterparty,
      messages: conv.messages.map(serializeMessage),
      unreadCount: conv.unreadCount,
      blocked: conv.blocked,
      lastActivityBlock: conv.lastActivityBlock.toString(),
    });
  });

  const payload: DmBackupJson = {
    version: BACKUP_VERSION,
    dm_meta: {
      scan_priv: toHex(scanPriv),
      spend_priv: toHex(spendPriv),
      spend_pub: toHex(spendPub),
      published_at_block: '0',
    },
    dm_scan_index: {
      lastScannedBlock: state.lastScannedBlock.toString(),
      conversations,
    },
    dm_block_list: {
      version: 1,
      entries: Array.from(state.blockList),
    },
    receipt_opt_out: state.receiptOptOut,
    exported_at_ms: Date.now(),
    device_id: generateDeviceId(),
  };

  const plaintext = new TextEncoder().encode(JSON.stringify(payload));
  const salt = crypto.getRandomValues(new Uint8Array(SALT_BYTES));
  const iv = crypto.getRandomValues(new Uint8Array(IV_BYTES));
  const key = await deriveAesKey(password, salt);

  // AAD: header bytes (version || salt || iv) を AES-GCM の additionalData として
  // 認証する。これがないとファイルの header だけ書き換える攻撃が GCM tag に
  // 検知されず、復号で salt/iv 不一致 → BackupImportFailed (DoS) に化ける。
  const header = new Uint8Array(HEADER_BYTES);
  header[0] = BACKUP_VERSION;
  header.set(salt, 1);
  header.set(iv, 1 + SALT_BYTES);

  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv, additionalData: header as BufferSource },
      key,
      plaintext,
    ),
  );

  const out = new Uint8Array(HEADER_BYTES + ciphertext.length);
  out.set(header, 0);
  out.set(ciphertext, HEADER_BYTES);
  return out;
}

/**
 * 暗号化 backup を復号し、dmStore にマージする。data-model.md §2.4 の merge 規則:
 *   - incoming message は (blockNumber, messageId, direction) を主キーとして集合和 (重複無視)
 *   - blocked は OR (どちらかが block → 結合後も block)
 *   - block_list は和集合
 *   - lastScannedBlock は backup のものを採用 (新端末 import ユースケース)
 *
 * 失敗モード:
 * - パスワード誤り / 改ざん: `DmError.BackupImportFailed` を throw
 * - version 不一致 / JSON decode 失敗: 同上
 */
export async function importDmBackup(
  file: Uint8Array,
  password: string,
): Promise<void> {
  if (file.length < HEADER_BYTES || file[0] !== BACKUP_VERSION) {
    throw new Error(DmError.BackupImportFailed);
  }
  const header = file.slice(0, HEADER_BYTES);
  const salt = file.slice(1, 1 + SALT_BYTES);
  const iv = file.slice(1 + SALT_BYTES, HEADER_BYTES);
  const ciphertext = file.slice(HEADER_BYTES);

  let plaintext: Uint8Array;
  try {
    const key = await deriveAesKey(password, salt);
    plaintext = new Uint8Array(
      await crypto.subtle.decrypt(
        { name: 'AES-GCM', iv, additionalData: header as BufferSource },
        key,
        ciphertext,
      ),
    );
  } catch {
    throw new Error(DmError.BackupImportFailed);
  }

  let payload: DmBackupJson;
  try {
    payload = JSON.parse(new TextDecoder().decode(plaintext)) as DmBackupJson;
  } catch {
    throw new Error(DmError.BackupImportFailed);
  }
  if (payload.version !== BACKUP_VERSION) {
    throw new Error(DmError.BackupImportFailed);
  }

  loadKeysIntoStealth(
    fromHex(payload.dm_meta.scan_priv),
    fromHex(payload.dm_meta.spend_priv),
  );

  const current = useDmStore.getState();
  const nextConversations = new Map<AccountId, ConversationState>(current.conversations);
  const nextBlockList = new Set<AccountId>(current.blockList);

  for (const entry of payload.dm_block_list.entries) {
    nextBlockList.add(entry as AccountId);
  }

  for (const backupConv of payload.dm_scan_index.conversations) {
    const counterparty = backupConv.counterparty as AccountId;
    const existing = nextConversations.get(counterparty);

    const keyOf = (m: DmMessageRecord) =>
      `${m.blockNumber.toString()}-${m.messageId.toString()}-${m.direction}`;
    const seen = new Map<string, DmMessageRecord>();
    if (existing) {
      for (const m of existing.messages) seen.set(keyOf(m), m);
    }
    for (const bm of backupConv.messages) {
      const m = deserializeMessage(bm);
      if (!seen.has(keyOf(m))) seen.set(keyOf(m), m);
    }
    const messages = Array.from(seen.values()).sort((a, b) => {
      if (a.blockNumber !== b.blockNumber) {
        return a.blockNumber < b.blockNumber ? -1 : 1;
      }
      if (a.messageId !== b.messageId) {
        return a.messageId < b.messageId ? -1 : 1;
      }
      return 0;
    });

    const blockedByList = nextBlockList.has(counterparty);
    const blocked =
      (existing?.blocked ?? false) || backupConv.blocked || blockedByList;

    const backupLast = BigInt(backupConv.lastActivityBlock);
    const existingLast = existing?.lastActivityBlock ?? 0n;
    const lastActivityBlock = backupLast > existingLast ? backupLast : existingLast;

    const unreadCount = blocked
      ? 0
      : Math.max(existing?.unreadCount ?? 0, backupConv.unreadCount);

    nextConversations.set(counterparty, {
      counterparty,
      messages,
      unreadCount,
      blocked,
      lastActivityBlock,
    });
  }

  useDmStore.setState({
    conversations: nextConversations,
    blockList: nextBlockList,
    lastScannedBlock: BigInt(payload.dm_scan_index.lastScannedBlock),
    // T079: backup に記録があれば優先。無い (旧版バックアップ) なら現値を維持。
    ...(payload.receipt_opt_out !== undefined
      ? { receiptOptOut: payload.receipt_opt_out }
      : {}),
  });
}

/**
 * Direct Message (feature 019) — frontend 型定義。
 *
 * 対応:
 * - data-model.md §2.4 (`ConversationState`, `DmMessageRecord`)
 * - contracts/frontend-ui.md §1.1–1.4 (`SendDmParams`, `SendDmResult`, `DmStore`)
 * - contracts/pallet-messaging-extrinsics.md §On-chain types (`DmMetaAddress`, `DmDispatch`)
 */

/**
 * Substrate AccountId32 の SS58 文字列表現。Polkadot-api の `SS58String` 互換。
 */
export type AccountId = string;

/**
 * DM 受信メタアドレス (データ構造)。
 *
 * `DmReceptionKeys` StorageMap の値と等価。`scan_pub` は X25519、
 * `spend_pub` は Ed25519 compressed Edwards point (いずれも 32B)。
 *
 * UI 層では Base58 (`st:anarchy:...`) 文字列で扱い、チェーン送受信時のみ
 * この構造体に変換する。`dmMetaFromString` / `dmMetaToString` を参照。
 */
export interface DmMetaAddress {
  scanPub: Uint8Array; // 32 bytes
  spendPub: Uint8Array; // 32 bytes
}

/**
 * `DmContentRef` の frontend 対応型。分散ストレージへの参照。
 */
export interface DmContentRef {
  root: Uint8Array; // 32 bytes (Blake2b-256 MerkleRoot)
  k: number;
  n: number;
  ciphertextLen: bigint; // パディング後サイズ (FR-026 のバケット値)
}

/**
 * `DmScanApi::dispatches_at` が返す単一 DM メタデータ。
 */
export interface DmDispatch {
  recipientStealth: AccountId;
  ephemeralPubkey: Uint8Array; // 32 bytes
  content: DmContentRef;
}

/**
 * 添付メディア参照 (DM 本文 envelope に格納)。
 *
 * - `root`: storage 上の Blake2b merkle root (32B hex)。`storage_getFragment` の id。
 * - `key`: AES-256-GCM 鍵 (32B hex)。E2E 担保のため DM 本文側にのみ存在する。
 * - `n` / `k`: 既存 DM 本体と同じ k-of-n (MVP では k=3 / n=5 固定)。
 * - `ciphertextLen`: `dm_media_encrypt` 出力の合計長 (`nonce(12) + ct + tag(16)`)。
 *   `fetchCiphertextFromStorage` で切り詰めるのに使う。
 *
 * `width` / `height` / `duration` / `thumbnail` は UI ヒント (post 側 MediaRef と同形式)。
 */
export interface DmMediaRef {
  root: string; // 32-byte hex (lowercase, no prefix)
  key: string; // 32-byte hex (lowercase, no prefix)
  mime: string;
  size: number;
  k: number;
  n: number;
  ciphertextLen: number;
  width?: number;
  height?: number;
  /** 動画の長さ (秒)。 */
  duration?: number;
  /** 動画サムネイル (data: URL)。 */
  thumbnail?: string;
}

/**
 * DM 本文 envelope。`lib/dm/contentCodec.ts` で encode/decode する。
 *
 * オンチェーンに乗るのは暗号化された DmEnvelope で、その中の `body` フィールドに
 * 本構造を [magic 4B] || [JSON UTF-8] で詰める。
 */
export interface DmContent {
  text: string;
  media: DmMediaRef[];
}

/**
 * 一件の DM をローカル永続化する際のレコード。data-model.md §2.4。
 *
 * GC 判別のため `bodyState` を保持 (T093/T094 対応、spec.md Edge Cases)。
 */
export interface DmMessageRecord {
  messageId: bigint;
  blockNumber: bigint;
  direction: 'incoming' | 'outgoing';
  counterparty: AccountId;
  timestampMs: number;
  body: Uint8Array;
  bodyState: 'plaintext' | 'garbage_collected';
  deliveryState?: 'sent' | 'delivered' | 'read'; // outgoing のみ
  signatureValid?: boolean; // incoming のみ。false のものは UI 非表示 (FR-004)
}

/**
 * 会話単位の状態。`DmStore.conversations` の値。
 */
export interface ConversationState {
  counterparty: AccountId;
  messages: DmMessageRecord[];
  unreadCount: number;
  blocked: boolean;
  lastActivityBlock: bigint;
}

/**
 * 送信時パラメータ。contracts/frontend-ui.md §1.1。
 */
export interface SendDmParams {
  recipientAccountId: AccountId;
  body: Uint8Array;
  k?: number; // 既定 3
  n?: number; // 既定 5
}

/**
 * 送信結果。contracts/frontend-ui.md §1.1。
 */
export interface SendDmResult {
  messageId: bigint;
  blockNumber: bigint;
  recipientStealth: AccountId;
  merkleRoot: Uint8Array;
  paddingBucket: number;
  totalCostMoral: bigint;
}

/**
 * スキャン結果。contracts/frontend-ui.md §1.2。
 *
 * `newReceipts` は US4 で追加: 受信者から届いた delivered/read receipt を
 * そのまま列挙する。store 側で `markAsDelivered` / `markAsRead` に変換する。
 */
export interface ScanDmResult {
  scannedFromBlock: bigint;
  scannedToBlock: bigint;
  newMessages: DmMessageRecord[];
  newReceipts: Array<{
    counterparty: AccountId;
    refMessageId: bigint;
    kind: 'delivered' | 'read';
  }>;
}

/**
 * ユーザー可視のエラー種別。`sender.ts` / `scanner.ts` から throw される。
 */
export enum DmError {
  RecipientKeyNotPublished = 'RecipientKeyNotPublished',
  StorageInsufficient = 'StorageInsufficient',
  MainAccountInsufficientBalance = 'MainAccountInsufficientBalance',
  TransactionDropped = 'TransactionDropped',
  BodyTooLarge = 'BodyTooLarge',
  InvalidMetaAddress = 'InvalidMetaAddress',
  DecryptFailed = 'DecryptFailed',
  SignatureInvalid = 'SignatureInvalid',
  BackupImportFailed = 'BackupImportFailed',
  MediaUploadFailed = 'MediaUploadFailed',
  MediaFetchFailed = 'MediaFetchFailed',
}

/**
 * ブロックリスト (ローカルのみ、オンチェーンには書き込まない; R9)。
 */
export interface BlockList {
  version: 1;
  entries: AccountId[];
}

/**
 * DM receipt wire format (T077 / US4).
 *
 * FR-016a の sent/delivered/read 遷移で使う「受信確認 DM」の body フォーマット。
 * receipts はプロトコル的には通常の DM (同じ pallet_messaging.send_dm を使う) だが、
 * 受信側で区別するため body の先頭に固定マジック + kind + messageId を置く。
 *
 * 設計判断 (MVP):
 *   - Rust `DmEnvelope` struct には手を入れない (SCALE 互換を崩さない)。
 *   - 代わりに envelope.body の wire format を 1 階層かぶせる: `MAGIC || kind || ref_msg_id`。
 *   - MAGIC は 4 バイト固定 `0x00 0x44 0x4D 0x52` ("\0DMR")。UTF-8 テキストが
 *     `0x00` から始まる可能性は実用的に無い (NULL 文字) ため衝突リスクは極小。
 *   - 将来 `DmEnvelope.kind: u8` を追加して wasm 側に移す際は、この body-framing
 *     を検出しない scanner パスへ切り替えれば良い (既存バックアップとの互換のため
 *     しばらく併存させる想定)。
 *
 * body layout (13 bytes):
 *   offset 0..4   MAGIC         (4 bytes, = [0x00, 0x44, 0x4D, 0x52])
 *   offset 4      kind          (1 byte, 1 = delivered, 2 = read)
 *   offset 5..13  ref_msg_id    (8 bytes, u64 little-endian)
 */

import type { AccountId, SendDmResult } from './types';
import { sendDm, type SendDmContext } from './sender';
import { useDmStore } from './store';

export const RECEIPT_MAGIC: Readonly<Uint8Array> = new Uint8Array([0x00, 0x44, 0x4d, 0x52]);
export const RECEIPT_BODY_LENGTH = RECEIPT_MAGIC.length + 1 + 8;

export type DmReceiptKind = 'delivered' | 'read';

const RECEIPT_KIND_BYTE: Record<DmReceiptKind, number> = {
  delivered: 1,
  read: 2,
};

const RECEIPT_KIND_FROM_BYTE: Record<number, DmReceiptKind> = {
  1: 'delivered',
  2: 'read',
};

export interface DmReceiptPayload {
  kind: DmReceiptKind;
  refMessageId: bigint;
}

export function encodeReceiptBody(kind: DmReceiptKind, refMessageId: bigint): Uint8Array {
  const out = new Uint8Array(RECEIPT_BODY_LENGTH);
  out.set(RECEIPT_MAGIC, 0);
  out[RECEIPT_MAGIC.length] = RECEIPT_KIND_BYTE[kind];
  // u64 little-endian。BigInt は DataView.setBigUint64(little=true) で書く。
  const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
  view.setBigUint64(RECEIPT_MAGIC.length + 1, refMessageId, true);
  return out;
}

/**
 * body が receipt フォーマットなら `{ kind, refMessageId }` を返す。
 * MAGIC / 長さ / kind byte のいずれかが不一致なら `null` (= 通常メッセージ扱い)。
 */
export function decodeReceiptBody(body: Uint8Array): DmReceiptPayload | null {
  if (body.length !== RECEIPT_BODY_LENGTH) return null;
  for (let i = 0; i < RECEIPT_MAGIC.length; i += 1) {
    if (body[i] !== RECEIPT_MAGIC[i]) return null;
  }
  const kind = RECEIPT_KIND_FROM_BYTE[body[RECEIPT_MAGIC.length]];
  if (!kind) return null;
  const view = new DataView(body.buffer, body.byteOffset, body.byteLength);
  const refMessageId = view.getBigUint64(RECEIPT_MAGIC.length + 1, true);
  return { kind, refMessageId };
}

export interface SendDmReceiptParams {
  counterparty: AccountId;
  refMessageId: bigint;
  kind: DmReceiptKind;
}

/**
 * 受信確認 DM を送る。
 *
 * - FR-016b: `receiptOptOut` が true のときは **`delivered` / `read` 両方とも**
 *   サプレスする (戻り値 `null`)。`delivered` だけでも "受信側がオンラインで
 *   メッセージを処理した時刻 T" がリークするため、UI ラベル
 *   "Do not send read receipts" の利用者期待 = "受信メタデータを送らない" を満たす。
 * - 成功時は通常の `SendDmResult` を返す (呼出側は badge 更新不要; 送信側スキャナが
 *   この receipt を拾って `markAsDelivered` / `markAsRead` に反映する)。
 */
export async function sendDmReceipt(
  params: SendDmReceiptParams,
  ctx: SendDmContext,
): Promise<SendDmResult | null> {
  if (useDmStore.getState().receiptOptOut) {
    return null;
  }
  const body = encodeReceiptBody(params.kind, params.refMessageId);
  return sendDm(
    {
      recipientAccountId: params.counterparty,
      body,
    },
    ctx,
  );
}

/**
 * DM 本文 (envelope.body) の codec。
 *
 * Wire format:
 *   byte 0..3  MAGIC = `0x44 0x4D 0x43 0x01` ("DMC" + version=1)
 *   byte 4..   JSON UTF-8 ({"text": string, "media": DmMediaRef[]})
 *
 * 衝突回避:
 *   receipt magic は `[0x00, 0x44, 0x4D, 0x52]` で先頭 byte が 0x00。本 codec は
 *   先頭 0x44 のため、`decodeReceiptBody` → `decodeDmContent` の順に試せば誤判定なし。
 *
 * 互換性: 旧 plain UTF-8 body はサポートしない (CLAUDE.md "Compatibility Policy")。
 */

import type { DmContent, DmMediaRef } from './types';

export const DM_CONTENT_MAGIC: Readonly<Uint8Array> = new Uint8Array([0x44, 0x4d, 0x43, 0x01]);

export function isDmContentEnvelope(body: Uint8Array): boolean {
  if (body.length < DM_CONTENT_MAGIC.length) return false;
  for (let i = 0; i < DM_CONTENT_MAGIC.length; i += 1) {
    if (body[i] !== DM_CONTENT_MAGIC[i]) return false;
  }
  return true;
}

export function encodeDmContent(content: DmContent): Uint8Array {
  // text 単体だが、空 media[] 配列も含めて常に envelope を出す (codec 単一化)。
  const json = JSON.stringify({
    text: content.text ?? '',
    media: content.media ?? [],
  });
  const jsonBytes = new TextEncoder().encode(json);
  const out = new Uint8Array(DM_CONTENT_MAGIC.length + jsonBytes.length);
  out.set(DM_CONTENT_MAGIC, 0);
  out.set(jsonBytes, DM_CONTENT_MAGIC.length);
  return out;
}

/**
 * 復号。`body` が envelope でない場合は `null` を返す。呼び出し側は null の場合
 * UI に「不明な本文」プレースホルダ等を出すか、receipt 判定済みのケースなら無視する。
 */
export function decodeDmContent(body: Uint8Array): DmContent | null {
  if (!isDmContentEnvelope(body)) return null;
  const jsonSlice = body.subarray(DM_CONTENT_MAGIC.length);
  let parsed: unknown;
  try {
    const json = new TextDecoder('utf-8', { fatal: true }).decode(jsonSlice);
    parsed = JSON.parse(json);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== 'object') return null;
  const obj = parsed as { text?: unknown; media?: unknown };
  const text = typeof obj.text === 'string' ? obj.text : '';
  const mediaRaw = Array.isArray(obj.media) ? obj.media : [];
  const media: DmMediaRef[] = [];
  for (const m of mediaRaw) {
    const ref = sanitizeMediaRef(m);
    if (ref) media.push(ref);
  }
  return { text, media };
}

function sanitizeMediaRef(raw: unknown): DmMediaRef | null {
  if (!raw || typeof raw !== 'object') return null;
  const r = raw as Record<string, unknown>;
  if (typeof r.root !== 'string' || !/^[0-9a-f]{64}$/.test(r.root)) return null;
  if (typeof r.key !== 'string' || !/^[0-9a-f]{64}$/.test(r.key)) return null;
  if (typeof r.mime !== 'string' || r.mime.length > 128) return null;
  if (typeof r.size !== 'number' || !Number.isFinite(r.size) || r.size < 0) return null;
  if (typeof r.k !== 'number' || typeof r.n !== 'number') return null;
  if (typeof r.ciphertextLen !== 'number' || r.ciphertextLen < 0) return null;
  const ref: DmMediaRef = {
    root: r.root,
    key: r.key,
    mime: r.mime,
    size: r.size,
    k: r.k,
    n: r.n,
    ciphertextLen: r.ciphertextLen,
  };
  if (typeof r.width === 'number') ref.width = r.width;
  if (typeof r.height === 'number') ref.height = r.height;
  if (typeof r.duration === 'number') ref.duration = r.duration;
  if (typeof r.thumbnail === 'string' && r.thumbnail.length < 32 * 1024) {
    // data: URL を想定。極端な巨大値はサニタイズで弾く。
    ref.thumbnail = r.thumbnail;
  }
  return ref;
}

/**
 * `text` 部分だけを取り出すヘルパ。envelope じゃない / 解析失敗時は null。
 */
export function decodeDmText(body: Uint8Array): string | null {
  const decoded = decodeDmContent(body);
  return decoded ? decoded.text : null;
}

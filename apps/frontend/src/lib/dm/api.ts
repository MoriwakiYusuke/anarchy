/**
 * DM 用データ変換ユーティリティ。
 *
 * UI 層と on-chain 層の間で `DmMetaAddress` の表現が 2 つ存在する:
 * - UI / backup:      `st:anarchy:<Base58(spend_pub || scan_pub)>` 文字列
 * - pallet / PAPI:    `{ scanPub: Uint8Array(32), spendPub: Uint8Array(32) }`
 *
 * 既存 stealth ライブラリの `parse_meta_address` / `format_meta_address_wasm`
 * を wasm-engine 経由でそのまま流用する (data-model.md §1.1 の「フィールド名の
 * 不一致を layer 境界で吸収」指針どおり)。
 */

import { DmError, type DmMetaAddress } from './types';

const META_ADDRESS_PREFIX = 'st:anarchy:';

/**
 * Base58 メタアドレス文字列 → `DmMetaAddress`。
 *
 * 受け取る文字列の定義は `packages/wasm-engine/src/stealth/address.rs` と
 * 完全一致 (spend || view の 64 バイトを Base58 エンコード)。wasm 呼出しを
 * 跨ぐ必要がないため本関数は同期 (TypeScript 単体) で完結させる。
 */
export function dmMetaFromString(s: string): DmMetaAddress {
  if (!s.startsWith(META_ADDRESS_PREFIX)) {
    throw new Error(DmError.InvalidMetaAddress);
  }
  const encoded = s.slice(META_ADDRESS_PREFIX.length);
  const decoded = base58Decode(encoded);
  if (decoded.length !== 64) {
    throw new Error(DmError.InvalidMetaAddress);
  }
  // stealth 仕様: `format_meta_address(spend, view)` → `Base58(spend || view)`。
  // DM 側の命名は `scan_pub` = view, `spend_pub` = spend。
  const spendPub = decoded.slice(0, 32);
  const scanPub = decoded.slice(32, 64);

  if (isAllZero(spendPub) || isAllZero(scanPub)) {
    throw new Error(DmError.InvalidMetaAddress);
  }
  return { scanPub, spendPub };
}

/**
 * `DmMetaAddress` → Base58 メタアドレス文字列。
 */
export function dmMetaToString(m: DmMetaAddress): string {
  if (m.scanPub.length !== 32 || m.spendPub.length !== 32) {
    throw new Error(DmError.InvalidMetaAddress);
  }
  const combined = new Uint8Array(64);
  combined.set(m.spendPub, 0);
  combined.set(m.scanPub, 32);
  return `${META_ADDRESS_PREFIX}${base58Encode(combined)}`;
}

// --- Base58 (stealth/address.rs と同じ bs58 デフォルトアルファベット) ---

const B58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

function base58Encode(bytes: Uint8Array): string {
  if (bytes.length === 0) return '';

  let zeros = 0;
  while (zeros < bytes.length && bytes[zeros] === 0) zeros += 1;

  const digits: number[] = [];
  for (let i = zeros; i < bytes.length; i += 1) {
    let carry = bytes[i];
    for (let j = 0; j < digits.length; j += 1) {
      carry += digits[j] << 8;
      digits[j] = carry % 58;
      carry = Math.floor(carry / 58);
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = Math.floor(carry / 58);
    }
  }

  let out = '';
  for (let i = 0; i < zeros; i += 1) out += B58_ALPHABET[0];
  for (let i = digits.length - 1; i >= 0; i -= 1) out += B58_ALPHABET[digits[i]];
  return out;
}

function base58Decode(s: string): Uint8Array {
  if (s.length === 0) return new Uint8Array(0);

  let zeros = 0;
  while (zeros < s.length && s[zeros] === B58_ALPHABET[0]) zeros += 1;

  const bytes: number[] = [];
  for (let i = zeros; i < s.length; i += 1) {
    const digit = B58_ALPHABET.indexOf(s[i]);
    if (digit < 0) throw new Error(DmError.InvalidMetaAddress);
    let carry = digit;
    for (let j = 0; j < bytes.length; j += 1) {
      carry += bytes[j] * 58;
      bytes[j] = carry & 0xff;
      carry >>= 8;
    }
    while (carry > 0) {
      bytes.push(carry & 0xff);
      carry >>= 8;
    }
  }

  const out = new Uint8Array(zeros + bytes.length);
  for (let i = bytes.length - 1, k = zeros; i >= 0; i -= 1, k += 1) out[k] = bytes[i];
  return out;
}

function isAllZero(buf: Uint8Array): boolean {
  for (let i = 0; i < buf.length; i += 1) if (buf[i] !== 0) return false;
  return true;
}

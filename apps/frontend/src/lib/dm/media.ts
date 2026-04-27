/**
 * DM 添付メディアの upload / fetch ヘルパ。
 *
 * 各添付ファイルは:
 *   1. K_media (32B random) を生成
 *   2. (image なら) EXIF 除去
 *   3. `dm_media_encrypt(K_media, plaintext)` → AES-GCM ciphertext (nonce||ct||tag)
 *   4. `dm_fragment_ciphertext(ct, k=3, n=5)` → fragments + merkle_root + proofs
 *   5. chain-node `storage_uploadFragment` で k-of-n upload (sender.ts の uploadFragments を再利用)
 *
 * 受信側は merkle_root から fragments を集めて concat → `dm_media_decrypt(K_media, ct)` で復元。
 *
 * E2E 担保: K_media は `DmMediaRef.key` として DM 本文 envelope に格納。本文自体は
 *   既存 DM パイプライン (X25519 + HKDF + AES-256-GCM) で受信者の stealth 鍵にだけ
 *   復号可能。よって K_media が他者に漏れることはない。
 */

import type { DmMediaRef } from './types';
import { DmError } from './types';
import { resolveChainRpcEndpoint, uploadFragments, type StorageSigner } from './sender';
import { fetchCiphertextFromStorage } from './storageFetch';
import { processMediaFile } from '@/lib/mediaProcessor';

type WasmModule = typeof import('anarchy-wasm-engine');
let wasmModule: WasmModule | null = null;

async function getWasm(): Promise<WasmModule> {
  if (wasmModule) return wasmModule;
  const module = await import('anarchy-wasm-engine');
  if (typeof window !== 'undefined' && typeof module.initSync === 'function') {
    const wasmUrl = new URL('/wasm/anarchy_wasm_engine_bg.wasm', window.location.origin);
    const response = await fetch(wasmUrl);
    if (!response.ok) {
      throw new Error(`Failed to fetch WASM: ${response.status} ${response.statusText}`);
    }
    const wasmBytes = await response.arrayBuffer();
    module.initSync({ module: new WebAssembly.Module(wasmBytes) });
  }
  wasmModule = module;
  return wasmModule;
}

function toHex(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 1) s += bytes[i].toString(16).padStart(2, '0');
  return s;
}

function fromHex(hex: string): Uint8Array {
  if (hex.length % 2 !== 0) throw new Error('invalid hex');
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i += 1) {
    out[i] = parseInt(hex.substr(i * 2, 2), 16);
  }
  return out;
}

export interface UploadDmMediaContext {
  /** Chain-node JSON-RPC HTTP エンドポイント (sender と同じ規則で解決)。 */
  chainRpcEndpoint?: string;
  /** Storage upload 認証用 raw signer (main account)。auth disabled の dev 環境では省略可。 */
  signer?: StorageSigner;
  /** ファイル単位の進捗 0–100。 */
  onProgress?: (uploadedFragments: number, totalFragments: number) => void;
}

export interface UploadDmMediaOptions {
  /** EXIF 除去 (画像のみ)。既定 true。 */
  stripExif?: boolean;
  /** k-of-n 閾値 (既定 3)。 */
  k?: number;
  /** total fragments (既定 5)。 */
  n?: number;
}

/**
 * 1 ファイルを暗号化 → 分割 → アップロードし、`DmMediaRef` を返す。
 * `key` は呼出後ゼロクリアされるが、ref に hex として残るため呼出側で速やかに本文へ詰める。
 */
export async function uploadDmMedia(
  file: File,
  ctx: UploadDmMediaContext,
  options: UploadDmMediaOptions = {},
): Promise<DmMediaRef> {
  const k = options.k ?? 3;
  const n = options.n ?? 5;
  const stripExif = options.stripExif ?? true;

  let processedFile = file;
  let width: number | undefined;
  let height: number | undefined;

  if (stripExif && file.type.startsWith('image/')) {
    try {
      const r = await processMediaFile(file, { stripExif: true });
      processedFile = r.file;
      width = r.width;
      height = r.height;
    } catch {
      // EXIF 除去失敗は致命的ではない。元ファイルで続行。
    }
  }

  const buffer = await processedFile.arrayBuffer();
  const plaintext = new Uint8Array(buffer);
  if (plaintext.length === 0) {
    throw new Error(`${DmError.MediaUploadFailed}: empty file`);
  }

  const wasm = await getWasm();

  // ---- Generate per-file key ----
  const key = new Uint8Array(32);
  crypto.getRandomValues(key);

  let ciphertext: Uint8Array;
  try {
    ciphertext = new Uint8Array(wasm.dm_media_encrypt(key, plaintext));
  } catch (e) {
    key.fill(0);
    throw new Error(`${DmError.MediaUploadFailed}: encrypt: ${(e as Error).message}`);
  }

  // ---- Fragment + Merkle ----
  let fragmented;
  try {
    fragmented = wasm.dm_fragment_ciphertext(ciphertext, k, n);
  } catch (e) {
    key.fill(0);
    throw new Error(`${DmError.MediaUploadFailed}: fragment: ${(e as Error).message}`);
  }

  const merkleRoot = new Uint8Array(fragmented.merkle_root);
  const fragments: Array<{ index: number; data: Uint8Array; proof: Uint8Array }> = [];
  for (let i = 0; i < fragmented.fragment_count; i += 1) {
    const frag = fragmented.fragment(i);
    const proof = fragmented.proof(i);
    if (!frag || !proof) continue;
    fragments.push({
      index: i,
      data: new Uint8Array(frag),
      proof: new Uint8Array(proof),
    });
  }

  // ---- Upload via chain-node ----
  const rpcEndpoint = resolveChainRpcEndpoint(ctx.chainRpcEndpoint);
  ctx.onProgress?.(0, fragments.length);
  try {
    await uploadFragments(rpcEndpoint, merkleRoot, fragments, k, fragments.length, {
      signer: ctx.signer,
    });
  } catch (e) {
    key.fill(0);
    throw new Error(`${DmError.MediaUploadFailed}: upload: ${(e as Error).message}`);
  }
  ctx.onProgress?.(fragments.length, fragments.length);

  const ref: DmMediaRef = {
    root: toHex(merkleRoot),
    key: toHex(key),
    mime: file.type || 'application/octet-stream',
    size: file.size,
    k,
    n,
    ciphertextLen: ciphertext.length,
    width,
    height,
  };

  // hex copy は ref に残ったので、生バッファは破棄。
  key.fill(0);
  return ref;
}

export interface FetchDmMediaContext {
  chainRpcEndpoint?: string;
  /** テスト用 fetch 差し替え。 */
  fetchImpl?: typeof fetch;
}

/**
 * blob / blob URL のグローバルキャッシュ。同一 root+key の重複 fetch を防ぎ、
 * 再 render 時に URL を都度作り直さない (createObjectURL を再呼び出しすると
 * `<img src=blob:>` で読み込み済みのバイトが無効化される副作用がある)。
 *
 * ライフサイクル: ページが unload されるまで保持。DM media は padding-bucket
 * 上限 256KB に収まる小さなバイナリで、上限内で高々 数百件 = 数十 MB なので
 * メモリリスク低。明示的な evict が必要になったら counterparty switch / DM
 * settings の "clear cache" 等で `clearDmMediaCache()` を呼ぶ運用にする。
 */
const blobCache = new Map<string, Promise<Blob>>();
const urlCache = new Map<string, string>();

function cacheKey(ref: DmMediaRef): string {
  return `${ref.root}|${ref.key}`;
}

/** 全件 evict (テスト / 鍵切替時に呼ぶ)。 */
export function clearDmMediaCache(): void {
  for (const url of urlCache.values()) URL.revokeObjectURL(url);
  urlCache.clear();
  blobCache.clear();
}

/**
 * キャッシュ済みなら同じ blob URL を返す。未取得なら fetch + decrypt + URL 生成
 * を実行してキャッシュ登録する。失敗したエントリは throw された Error を残さず
 * cache から消す (次回再試行できるように)。
 */
export async function getCachedDmMediaUrl(
  ref: DmMediaRef,
  ctx: FetchDmMediaContext = {},
): Promise<string> {
  const k = cacheKey(ref);
  const existingUrl = urlCache.get(k);
  if (existingUrl) return existingUrl;

  let p = blobCache.get(k);
  if (!p) {
    p = fetchDmMedia(ref, ctx);
    blobCache.set(k, p);
    // 失敗時は cache を捨てて次回再試行可能にする。
    p.catch(() => {
      blobCache.delete(k);
    });
  }
  const blob = await p;

  // 競合: 別の caller が同じ key で先に URL 生成済みかも知れない。
  const racedUrl = urlCache.get(k);
  if (racedUrl) return racedUrl;
  const url = URL.createObjectURL(blob);
  urlCache.set(k, url);
  return url;
}

/**
 * `DmMediaRef` を解決して `Blob` を返す。
 *
 * 失敗 (storage 取得不能 / 復号失敗) は `DmError.MediaFetchFailed` を投げる。
 * 呼出側は UI 上 placeholder を出すなりリトライするなり判断する。
 */
export async function fetchDmMedia(
  ref: DmMediaRef,
  ctx: FetchDmMediaContext = {},
): Promise<Blob> {
  const wasm = await getWasm();
  const rpcEndpoint = resolveChainRpcEndpoint(ctx.chainRpcEndpoint);

  const root = fromHex(ref.root);
  if (root.length !== 32) {
    throw new Error(`${DmError.MediaFetchFailed}: invalid root`);
  }

  const ciphertext = await fetchCiphertextFromStorage(
    rpcEndpoint,
    root,
    ref.n,
    ref.ciphertextLen,
    { fetchImpl: ctx.fetchImpl },
  );
  if (!ciphertext) {
    throw new Error(`${DmError.MediaFetchFailed}: fragment unavailable`);
  }

  const key = fromHex(ref.key);
  if (key.length !== 32) {
    throw new Error(`${DmError.MediaFetchFailed}: invalid key`);
  }
  let plaintext: Uint8Array;
  try {
    plaintext = new Uint8Array(wasm.dm_media_decrypt(key, ciphertext));
  } catch (e) {
    key.fill(0);
    throw new Error(`${DmError.MediaFetchFailed}: decrypt: ${(e as Error).message}`);
  }
  key.fill(0);

  // 明示的に ArrayBuffer (≠ SharedArrayBuffer) にコピー。strict TS の BlobPart 型推論が
  // `Uint8Array<ArrayBufferLike>` を許容しないため。
  const arrayBuffer = new ArrayBuffer(plaintext.byteLength);
  new Uint8Array(arrayBuffer).set(plaintext);
  return new Blob([arrayBuffer], { type: ref.mime });
}

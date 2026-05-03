/**
 * DM sender (T044) — `sendDm` 9-step orchestrator.
 *
 * Contract: contracts/frontend-ui.md §1.1.
 *
 * 9-step flow:
 *  1. Resolve recipient `DmReceptionKeys` via `DmScanApi::reception_key`. Missing
 *     → throw `DmError.RecipientKeyNotPublished` (no MORAL spent).
 *  2. Wallet signer signs the inner_signed_hash (W6) of the envelope.
 *  3. wasm `dm_encrypt_and_pad` → ciphertext + ephemeral_pub + recipient_stealth.
 *  4. wasm `dm_fragment_ciphertext` → fragments + merkle_root + per-index proofs.
 *  5. Upload fragments via **chain-node RPC** `storage_uploadFragment` (CLAUDE.md
 *     Security Principle #5: no direct storage-node access from frontend); chain
 *     node verifies merkle proof + forwards to storage-node. Require k ACKs
 *     within 30s or throw `DmError.StorageInsufficient`.
 *  6. wasm `dm_generate_sender_stealth` → fresh sr25519 keypair (CT-1).
 *  7. PAPI: `pallet_stealth.send_to_stealth(sender_stealth, random_eph, pre_fund)`
 *     signed by main account → finalize.
 *  8. PAPI: `pallet_messaging.send_dm(...)` signed by sender_stealth seed →
 *     finalize. Failures here surface as `DmError.TransactionDropped` with the
 *     pre-fund still resident on the stealth account (T091 retry path).
 *  9. Zeroize sender_stealth secret_seed, return `SendDmResult`.
 */

import type { Binary } from 'polkadot-api';
import type { PolkadotSigner } from 'polkadot-api/signer';
import type { AccountId, DmMediaRef, DmMetaAddress, SendDmParams, SendDmResult } from './types';
import { DmError } from './types';
import { encodeDmContent } from './contentCodec';
import { debugError, debugWarn } from '@/lib/debugLog';

type WasmModule = typeof import('anarchy-wasm-engine');
let wasmModule: WasmModule | null = null;

async function getWasm(): Promise<WasmModule> {
  if (wasmModule) return wasmModule;
  const module = await import('anarchy-wasm-engine');
  // ブラウザ実行時のみ initSync を行う。Jest 環境では mock 化されるため通らない。
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

/**
 * パラメータ - PAPI api / wallet signer / chain RPC エンドポイントなど IO を
 * UI 層から注入するための束。テスト容易性のため `sendDm` から分離。
 */
export interface SendDmContext {
  /** PAPI unsafeApi。`tx.Messaging.send_dm`, `tx.Stealth.send_to_stealth`,
   *  `apis.DmScanApi.reception_key` を持つ。 */
  api: unknown;
  /** 送信者メインアカウントの polkadot-api signer。 */
  mainSigner: PolkadotSigner;
  /** 送信者メインアカウントの Sr25519 公開鍵 (= AccountId32 raw 32B)。 */
  mainAccountPublicKey: Uint8Array;
  /** 送信者メインアカウントの raw sr25519 signer。`inner_signed_hash` (W6) の
   *  署名に使う。`PolkadotSigner.signBytes` は `<Bytes>...</Bytes>` wrap をする
   *  ため受信側 `dm_decrypt_scan` の `signature_valid` が false になる。
   *  `@polkadot/keyring` の `pair.sign(msg)` をそのまま渡す想定。 */
  mainRawSigner?: StorageSigner;
  /** Chain-node JSON-RPC HTTP エンドポイント。`storage_uploadFragment` を叩く
   *  ため必要 (CLAUDE.md Security Principle #5)。省略時は環境変数
   *  `NEXT_PUBLIC_WS_ENDPOINT` を http:// に変換したものか
   *  `http://127.0.0.1:9944` を使用。 */
  chainRpcEndpoint?: string;
  /** UI 進捗報告コールバック (FR-025)。 */
  onProgress?: (step: SendDmProgress) => void;
}

/**
 * `inner_signed_hash` 署名用の raw sr25519 signer。`@polkadot/keyring` の
 * `pair.sign(msg)` が substrate signing context を適用するため、それをそのまま
 * `sign` として渡す想定。
 */
export interface StorageSigner {
  publicKey: Uint8Array;
  sign: (message: Uint8Array) => Uint8Array | Promise<Uint8Array>;
}

export type SendDmProgress =
  | { kind: 'encrypting' }
  | { kind: 'uploading'; uploaded: number; total: number }
  | { kind: 'prefunding' }
  | { kind: 'dispatching' }
  | { kind: 'done' };

/**
 * `pallet_messaging` / `pallet_stealth` / `DmScanApi` の最小 PAPI 形状。
 */
interface MessagingPapi {
  tx: {
    Messaging: {
      send_dm: (params: {
        recipient_stealth: string;
        ephemeral_pubkey: Binary;
        merkle_root: Binary;
        k: number;
        n: number;
        ciphertext_len: bigint;
      }) => {
        signAndSubmit: (signer: PolkadotSigner) => Promise<{
          txHash: string;
          ok: boolean;
          block: { number: number; hash: string };
        }>;
      };
    };
    Stealth: {
      send_to_stealth: (params: {
        stealth_address: string;
        ephemeral_pubkey: Binary;
        amount: bigint;
      }) => {
        signAndSubmit: (signer: PolkadotSigner) => Promise<{
          txHash: string;
          ok: boolean;
          block: { number: number; hash: string };
        }>;
      };
    };
  };
  apis: {
    DmScanApi: {
      reception_key: (account: string) => Promise<DmMetaAddress | null>;
    };
  };
}

const MORAL = 1_000_000_000_000n; // 12 decimals
const DM_BASE_COST = 1n * MORAL;
const DM_BYTE_COST = 50_000_000_000n; // 0.05 MORAL / byte
const PRE_FUND_MARGIN = 1n * MORAL;   // 1 MORAL margin for tx fee on stealth side

/**
 * `pallet_messaging.send_dm` のコスト見積もり。pre-fund 量を求めるのに使う。
 */
export function estimateDmCost(ciphertextLen: number): bigint {
  return DM_BASE_COST + DM_BYTE_COST * BigInt(ciphertextLen) + PRE_FUND_MARGIN;
}

/** ISO 7816-4 padding bucket (`packages/wasm-engine/src/dm/padding.rs` と一致)。 */
const PADDING_BUCKETS = [1024, 4096, 16384, 65536, 262144] as const;

/** AES-GCM tag length (バケット内訳の控除用)。 */
const AES_GCM_TAG_LEN = 16;

/** `DmEnvelope` の SCALE encoded 固定長フィールド合計 (version + sender + ts + sig)。
 *  body は別途 SCALE compact prefix が付くので encode 関数で別計算。 */
const ENVELOPE_FIXED_BYTES = 1 + 32 + 8 + 64;

/**
 * SCALE compact mode の長さ prefix のバイト数。
 *   < 64           → 1 byte
 *   < 16384        → 2 bytes
 *   < 1_073_741_824 → 4 bytes
 *   それ以上        → 5+ bytes (DM body では事実上発生しない)
 */
function scaleCompactPrefixLen(n: number): number {
  if (n < 64) return 1;
  if (n < 16384) return 2;
  if (n < 1073741824) return 4;
  return 5;
}

/** Estimate 用に必要な PendingFile のサブセット。 */
export interface DmCostFile {
  mime: string;
  size: number;
  width?: number;
  height?: number;
  duration?: number;
  /** 動画サムネイル (data URL)。video の場合 data URL の長さがそのまま envelope
   *  に乗るので、ここを無視すると数十 KB の見積もり誤差が出る。 */
  thumbnail?: string;
}

/**
 * 送信コスト UI 表示用: `text` と `files` から実際の `encodeDmContent` 結果と
 * `DmEnvelope` の SCALE 長を計算し、最小バケットを選定して `estimateDmCost`
 * を返す。`dm_encrypt_and_pad` と同じバケット選択を再現するので、選ばれる
 * バケット = ciphertext_len は実値と一致する。
 *
 * 引数:
 *   - `text`: 本文 (codec の `text` フィールド)
 *   - `files`: 添付の mime/size/dimensions/thumbnail。`mediaCount` のみの旧
 *     API (number) も後方互換で受け付ける。
 *
 * 戻り値:
 *   - bigint: 12-decimal MORAL 単位のコスト (= main account 残高引き落とし額)
 *   - null:   最大バケット (262144) に収まらない (= `BodyTooLarge` 相当)
 */
export function estimateDmCostFromInputs(
  text: string,
  files: number | readonly DmCostFile[],
): bigint | null {
  const fileList: readonly DmCostFile[] = typeof files === 'number'
    ? new Array<DmCostFile>(files).fill({
        // 旧 API 互換: 数値だけ渡されたら平均的な image-like ref を仮定する。
        mime: 'application/octet-stream',
        size: 0,
      })
    : files;

  // `lib/dm/contentCodec.ts` と同じ wire format を生成して body byte 数を出す。
  // root/key はまだ未生成 (encrypt 前) なので 32B 0 埋めの hex プレースホルダで
  // **実際と完全一致する長さ** にする (root/key は固定 64-hex で必ず同じ長さ)。
  const dummyRefs: DmMediaRef[] = fileList.map((f) => {
    const ref: DmMediaRef = {
      root: '0'.repeat(64),
      key: '0'.repeat(64),
      mime: f.mime || 'application/octet-stream',
      size: f.size,
      k: 3,
      n: 5,
      // ciphertextLen は実値だと最終的に dm_media_encrypt の出力長 = file.size
      // + nonce(12) + tag(16) = file.size + 28。仮値として近似する。
      ciphertextLen: f.size + 28,
    };
    if (typeof f.width === 'number') ref.width = f.width;
    if (typeof f.height === 'number') ref.height = f.height;
    if (typeof f.duration === 'number') ref.duration = f.duration;
    if (typeof f.thumbnail === 'string') ref.thumbnail = f.thumbnail;
    return ref;
  });

  // body = 4B magic + JSON UTF-8 (実値と完全一致する長さ)。
  const body = encodeDmContent({ text, media: dummyRefs });
  const bodyLen = body.length;

  // envelope_len = 1 (version) + 32 (sender) + 8 (ts) + scale(body.len) + body.len + 64 (sig)
  const envelopeLen =
    ENVELOPE_FIXED_BYTES + scaleCompactPrefixLen(bodyLen) + bodyLen;

  // padded plaintext = envelope + ISO 7816-4 terminator (1B); ciphertext = padded + tag (16B).
  // bucket は ciphertext (= padded + tag) を収める最小値。
  const needed = envelopeLen + 1 + AES_GCM_TAG_LEN;

  for (const b of PADDING_BUCKETS) {
    if (b >= needed) return estimateDmCost(b);
  }
  return null;
}

/** 12-decimal plancks → "X.YY MORAL" 文字列 (2 桁少数)。 */
export function formatMoral(plancks: bigint): string {
  const negative = plancks < 0n;
  const v = negative ? -plancks : plancks;
  const whole = v / MORAL;
  const frac = v % MORAL;
  // 0.05 単位精度を担保するため少数 2 桁表示。MORAL は 12 桁なので
  // frac * 100 / 1e12 で 2 桁を取り、四捨五入はせず切り捨て。
  const fracHundredths = (frac * 100n) / MORAL;
  const fracStr = fracHundredths.toString().padStart(2, '0');
  return `${negative ? '-' : ''}${whole}.${fracStr}`;
}

/**
 * Uint8Array → base64 (storage-node `b64_decode` と互換)。
 */
function toBase64(bytes: Uint8Array): string {
  if (typeof btoa === 'function') {
    let bin = '';
    for (let i = 0; i < bytes.length; i += 1) bin += String.fromCharCode(bytes[i]);
    return btoa(bin);
  }
  // Node.js 環境 (Jest) フォールバック
  return Buffer.from(bytes).toString('base64');
}

function toHex(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 1) s += bytes[i].toString(16).padStart(2, '0');
  return s;
}

interface SignedAuth {
  account_id: string;
  timestamp: number;
  nonce: string;
  payload_hash: string;
  signature: string;
}

/**
 * Storage-node middleware が `storage_storeFragment` のような書き込み API で
 * 要求する `X-Anarchy-Auth` を作る。chain-node 経由で forward される時、
 * chain-node は `params.auth` を取り出して `X-Anarchy-Auth` ヘッダに移し替える
 * (apps/blockchain/node/src/rpc/storage.rs の StorageNodeClient.upload_fragment)。
 *
 * Principle #5 (no direct storage access) は **IP correlation** を防ぐ規約であり、
 * **user 署名による rate limiting / abuse 抑止**は依然必要。post 側の useUpload.ts
 * の `generateAuth` と同等。
 *
 * 署名対象: account_id(32) || timestamp_le_u64(8) || nonce(16) || payloadHash(32)
 * payloadHash = blake2b256(JSON.stringify(sortedParamsWithoutAuth))
 */
async function generateUploadAuth(
  signer: StorageSigner,
  params: Record<string, unknown>,
): Promise<SignedAuth> {
  const { blake2b } = await import('blakejs');

  const timestamp = Math.floor(Date.now() / 1000);
  const nonceBytes = new Uint8Array(16);
  crypto.getRandomValues(nonceBytes);

  // storage-node の `extract_and_hash_params` は serde_json default
  // (キー alphabetical 順) → Blake2b-256。フロントも明示的に sort 必須。
  const sortedParams = Object.keys(params)
    .sort()
    .reduce<Record<string, unknown>>((acc, key) => {
      acc[key] = params[key];
      return acc;
    }, {});
  const payloadHash = blake2b(
    new TextEncoder().encode(JSON.stringify(sortedParams)),
    undefined,
    32,
  );

  const message = new Uint8Array(32 + 8 + 16 + 32);
  message.set(signer.publicKey, 0);
  new DataView(message.buffer).setBigUint64(32, BigInt(timestamp), true);
  message.set(nonceBytes, 40);
  message.set(payloadHash, 56);

  const signature = await signer.sign(message);
  return {
    account_id: toHex(signer.publicKey),
    timestamp,
    nonce: toHex(nonceBytes),
    payload_hash: toHex(payloadHash),
    signature: toHex(signature),
  };
}

/**
 * Chain-node JSON-RPC HTTP エンドポイントを解決する。
 *
 * - `override` (SendDmContext.chainRpcEndpoint) が最優先。
 * - 無ければ `NEXT_PUBLIC_WS_ENDPOINT` を `ws://`→`http://` に書き換えて使用。
 * - それも無ければ dev fallback の `http://127.0.0.1:9944`。
 *
 * post / reaction の useUpload.ts / useFragments.ts と同じ規則で揃えている。
 */
export function resolveChainRpcEndpoint(override?: string): string {
  if (override) return override;
  const fromEnv = process.env.NEXT_PUBLIC_WS_ENDPOINT;
  if (fromEnv) {
    return fromEnv.replace(/^ws:\/\//, 'http://').replace(/^wss:\/\//, 'https://');
  }
  return 'http://127.0.0.1:9944';
}

/**
 * tx1 (`send_to_stealth`) は常に main account で署名する。レシート自動送信などで
 * 並行して走った場合、PAPI が同じ nonce で 2 つ署名し → 後続が `InvalidTransaction::Stale`
 * で reject される。グローバル mutex で順序化する (DM 送信は非ホットパスなので OK)。
 */
let tx1Lock: Promise<unknown> = Promise.resolve();
async function runSerialTx1<T>(fn: () => Promise<T>): Promise<T> {
  const prev = tx1Lock;
  let resolveNext: () => void = () => {};
  tx1Lock = new Promise<void>((r) => {
    resolveNext = r;
  });
  try {
    await prev.catch(() => {
      /* 前のホルダーが reject してもそのまま続行 */
    });
    return await fn();
  } finally {
    resolveNext();
  }
}

/**
 * Chain-node の `storage_uploadFragment` RPC で fragments を並列アップロードする。
 * `k` 個 ACK で成功。30 秒以内に揃わなければ `DmError.StorageInsufficient`。
 *
 * **CLAUDE.md Security Principle #5**: フロントは storage-node に直接アクセスしない。
 * チェーンノード経由で fan-out する。チェーンノード側で merkle proof を検証してから
 * 内部で storage-node に転送するので、proof は必須。
 */
export async function uploadFragments(
  chainRpcEndpoint: string,
  merkleRoot: Uint8Array,
  fragments: Array<{ index: number; data: Uint8Array; proof: Uint8Array }>,
  k: number,
  totalLeaves: number,
  options: {
    retryWindowMs?: number;
    fetchImpl?: typeof fetch;
    signer?: StorageSigner;
  } = {},
): Promise<number> {
  const start = Date.now();
  const window = options.retryWindowMs ?? 30_000;
  const fetchFn = options.fetchImpl ?? fetch;
  const signer = options.signer;
  const ok = new Set<number>();
  let attempt = 0;

  while (ok.size < k && Date.now() - start < window) {
    attempt += 1;
    const pending = fragments.filter((f) => !ok.has(f.index));
    const results = await Promise.allSettled(
      pending.map(async (f) => {
        // chain-node `UploadFragmentRequest`: merkle_root [u8; 32], index u32,
        // data b64, proof b64, total_leaves u32, auth optional。
        // signer があれば auth を付ける (chain-node が X-Anarchy-Auth ヘッダに展開)。
        const params: Record<string, unknown> = {
          merkle_root: Array.from(merkleRoot),
          index: f.index,
          data: toBase64(f.data),
          proof: toBase64(f.proof),
          total_leaves: totalLeaves,
        };
        if (signer) {
          params.auth = await generateUploadAuth(signer, params);
        }

        const res = await fetchFn(chainRpcEndpoint, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            jsonrpc: '2.0',
            id: f.index,
            method: 'storage_uploadFragment',
            params: [params],
          }),
        });
        if (!res.ok) throw new Error(`http ${res.status}`);
        const body = (await res.json()) as { error?: unknown; result?: unknown };
        if (body.error) throw new Error(`rpc error: ${JSON.stringify(body.error)}`);
        return f.index;
      }),
    );
    for (const r of results) if (r.status === 'fulfilled') ok.add(r.value);

    if (ok.size < k && Date.now() - start < window) {
      const backoff = Math.min(1000 * attempt, 5000);
      await new Promise((res) => setTimeout(res, backoff));
    }
  }
  if (ok.size < k) throw new Error(DmError.StorageInsufficient);
  return ok.size;
}

/**
 * 送信者 stealth の sr25519 seed から polkadot-api 互換 signer を作る。
 * 鍵を destroy する責務は呼出側 (sendDm が finalize 後にゼロクリア)。
 */
async function makeStealthSigner(seed: Uint8Array): Promise<PolkadotSigner> {
  if (seed.length !== 32) throw new Error('stealth seed must be 32 bytes');
  const { Keyring } = await import('@polkadot/keyring');
  const { getPolkadotSigner } = await import('polkadot-api/signer');
  const keyring = new Keyring({ type: 'sr25519' });
  const pair = keyring.addFromSeed(seed);
  return getPolkadotSigner(
    pair.publicKey,
    'Sr25519',
    (input: Uint8Array) => Promise.resolve(pair.sign(input)),
  );
}

/**
 * SS58 (prefix 42 = generic Substrate) エンコード。stealth/api.ts と同じ実装方針
 * (substrate-bindings の `fromBufferToBase58`)。
 */
async function encodeSs58(pubkey: Uint8Array): Promise<string> {
  const { fromBufferToBase58 } = await import('@polkadot-api/substrate-bindings');
  return fromBufferToBase58(42)(pubkey);
}

/**
 * `sendDm` — DM 送信オーケストレータ。9 ステップ。
 */
export async function sendDm(
  params: SendDmParams,
  ctx: SendDmContext,
): Promise<SendDmResult> {
  const {
    api,
    mainSigner,
    mainAccountPublicKey,
    mainRawSigner,
    chainRpcEndpoint,
    onProgress,
  } = ctx;
  if (!api) throw new Error('api unavailable');
  if (mainAccountPublicKey.length !== 32) {
    throw new Error('mainAccountPublicKey must be 32 bytes');
  }
  if (!params.body || params.body.length === 0) {
    throw new Error(DmError.BodyTooLarge); // 仕様: 空本文は呼出前にバリデート
  }

  const k = params.k ?? 3;
  const n = params.n ?? 5;
  const typed = api as MessagingPapi;
  const wasm = await getWasm();

  // ---- Step 1: recipient reception key ----
  const rawRecipient = (await typed.apis.DmScanApi.reception_key(
    params.recipientAccountId,
  )) as unknown as
    | null
    | undefined
    | {
        scan_pub?: { asBytes?: () => Uint8Array } | Uint8Array;
        spend_pub?: { asBytes?: () => Uint8Array } | Uint8Array;
        scanPub?: Uint8Array;
        spendPub?: Uint8Array;
      };
  if (!rawRecipient) throw new Error(DmError.RecipientKeyNotPublished);
  const toBytes = (v: unknown): Uint8Array => {
    if (v instanceof Uint8Array) return v;
    const withAsBytes = v as { asBytes?: () => Uint8Array };
    if (withAsBytes?.asBytes) return withAsBytes.asBytes();
    throw new Error('recipient meta_address field has unexpected shape');
  };
  const recipient: { scanPub: Uint8Array; spendPub: Uint8Array } = {
    scanPub: toBytes(rawRecipient.scan_pub ?? rawRecipient.scanPub),
    spendPub: toBytes(rawRecipient.spend_pub ?? rawRecipient.spendPub),
  };

  // ---- Step 2 prep: ephemeral X25519 priv + stealth pre-derive (W5) ----
  const ephPriv = randomEphemeralPriv();
  const derived = wasm.dm_derive_recipient_stealth(
    recipient.scanPub,
    recipient.spendPub,
    ephPriv,
  );
  const ephPubBytes = new Uint8Array(derived.ephemeral_pubkey);
  const stealthBytes = new Uint8Array(derived.stealth_pubkey);
  // shared_secret は HKDF を Rust 側で行うため JS 側からは触らない。
  // wasm-bindgen の `free()` で Rust 線形メモリ上の Zeroizing<[u8;32]> を消す。
  try {
    (derived as unknown as { free?: () => void }).free?.();
  } catch {
    // free が無いビルド (テスト mock 等) では no-op
  }

  const timestampMs = BigInt(Date.now());
  const body = params.body;

  onProgress?.({ kind: 'encrypting' });

  // ---- Step 2: wallet signs inner_signed_hash (W6) ----
  const sigHash = wasm.dm_compute_inner_signed_hash(
    mainAccountPublicKey,
    stealthBytes,
    ephPubBytes,
    timestampMs,
    body,
  );
  // inner_signed_hash は raw sr25519 で署名する必要がある。PolkadotSigner.signBytes
  // は `<Bytes>...</Bytes>` wrap をしてしまい dm_decrypt_scan の signature_valid が
  // false になるため不可。mainRawSigner (keyring pair.sign) を優先し、無ければ
  // main signer にフォールバック (後者は dev 時以外互換性なし)。
  const senderSignature = mainRawSigner
    ? await Promise.resolve(mainRawSigner.sign(new Uint8Array(sigHash)))
    : await mainSigner.signBytes(new Uint8Array(sigHash));
  if (senderSignature.length !== 64) {
    throw new Error(`expected 64-byte signature, got ${senderSignature.length}`);
  }

  // ---- Step 3: encrypt + pad (W1) ----
  const encrypted = wasm.dm_encrypt_and_pad(
    recipient.scanPub,
    recipient.spendPub,
    mainAccountPublicKey,
    senderSignature,
    body,
    timestampMs,
    ephPriv,
  );
  // ephemeral priv は WASM 側で Zeroizing コピーされるが JS 側でも即破棄。
  ephPriv.fill(0);

  const ciphertext = new Uint8Array(encrypted.ciphertext);
  const paddingBucket = encrypted.padding_bucket;

  // ---- Step 4: fragment + merkle (W4) ----
  const fragmented = wasm.dm_fragment_ciphertext(ciphertext, k, n);
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

  // ---- Step 5: storage upload (k-of-n ACK) via chain-node RPC ----
  // mainRawSigner があれば user-signed `auth` を付加する。chain-node は
  // それを X-Anarchy-Auth ヘッダに展開して storage-node に forward する。
  // ない場合は無認証で送信 (auth disabled な dev storage-node でのみ動作)。
  onProgress?.({ kind: 'uploading', uploaded: 0, total: fragments.length });
  const rpcEndpoint = resolveChainRpcEndpoint(chainRpcEndpoint);
  await uploadFragments(rpcEndpoint, merkleRoot, fragments, k, fragments.length, {
    signer: mainRawSigner,
  });
  onProgress?.({ kind: 'uploading', uploaded: fragments.length, total: fragments.length });

  // ---- Step 6: fresh sender stealth (W3) ----
  const stealth = wasm.dm_generate_sender_stealth();
  const senderStealthAccount = new Uint8Array(stealth.account_id);
  // CT-1 / FR-021: secret_seed は wasm getter が返す Uint8Array をそのまま保持し、
  // tx2 完了後 (finally) に当該バッファを直接 fill(0) する。コピーは作らない。
  const senderStealthSeed: Uint8Array = stealth.secret_seed as unknown as Uint8Array;

  let stealthSigner: PolkadotSigner;
  try {
    stealthSigner = await makeStealthSigner(senderStealthSeed);

    // ---- Step 7: tx1 = pallet_stealth.send_to_stealth (pre-fund) ----
    onProgress?.({ kind: 'prefunding' });
    const preFundAmount = estimateDmCost(ciphertext.length);
    const senderStealthSs58 = await encodeSs58(senderStealthAccount);

    const { Binary } = await import('polkadot-api');
    // R8: tx1 の ephemeral_pubkey はランダム値 (DM 側 ephemeral とは独立)。
    const tx1Eph = new Uint8Array(32);
    crypto.getRandomValues(tx1Eph);

    const tx1 = typed.tx.Stealth.send_to_stealth({
      stealth_address: senderStealthSs58,
      ephemeral_pubkey: Binary.fromBytes(tx1Eph),
      amount: preFundAmount,
    });
    let tx1Result;
    try {
      // tx1 は main account 署名で nonce を消費する。並行 sendDm (例: 受信レシート
      // と user 起点 DM) が同 nonce で signAndSubmit すると後者が `Stale` で reject
      // されるので、グローバル mutex で 1 件ずつ流す。さらに `Stale` を 1 回だけ
      // リトライ — PAPI は signAndSubmit 毎に nonce 再取得するので、mutex 内で
      // 再呼び出しすれば最新値が拾える。
      tx1Result = await runSerialTx1(async () => {
        try {
          return await tx1.signAndSubmit(mainSigner);
        } catch (err) {
          const m = err instanceof Error ? err.message : String(err);
          if (/Stale/i.test(m)) {
            debugWarn('[dm-sender] tx1 stale, retrying once');
            await new Promise((r) => setTimeout(r, 200));
            return await tx1.signAndSubmit(mainSigner);
          }
          throw err;
        }
      });
    } catch (e) {
      // 失敗理由をできるだけ具体的に出す。残高絡みなら専用エラー、
      // それ以外は元 error を残して TransactionDropped に丸める。
      const msg = e instanceof Error ? e.message : String(e);
      if (/InsufficientBalance|FundsUnavailable|Token::FundsUnavailable|Payment/i.test(msg)) {
        throw new Error(DmError.MainAccountInsufficientBalance);
      }
      debugError('[dm-sender] tx1 (send_to_stealth) failed:', e);
      throw new Error(`${DmError.TransactionDropped}: tx1 ${msg}`);
    }
    if (!tx1Result.ok) {
      debugError('[dm-sender] tx1 dispatch error:', tx1Result);
      throw new Error(DmError.TransactionDropped);
    }

    // ---- Step 8: tx2 = pallet_messaging.send_dm ----
    onProgress?.({ kind: 'dispatching' });
    const recipientStealthSs58 = await encodeSs58(stealthBytes);
    const tx2 = typed.tx.Messaging.send_dm({
      recipient_stealth: recipientStealthSs58,
      ephemeral_pubkey: Binary.fromBytes(ephPubBytes),
      merkle_root: Binary.fromBytes(merkleRoot),
      k,
      n,
      ciphertext_len: BigInt(ciphertext.length),
    });
    let tx2Result;
    try {
      tx2Result = await tx2.signAndSubmit(stealthSigner);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      debugError('[dm-sender] tx2 (send_dm) failed:', e);
      throw new Error(`${DmError.TransactionDropped}: tx2 ${msg}`);
    }
    if (!tx2Result.ok) {
      debugError('[dm-sender] tx2 dispatch error:', tx2Result);
      throw new Error(DmError.TransactionDropped);
    }

    onProgress?.({ kind: 'done' });

    const blockNumber = BigInt(tx2Result.block.number);
    // scanner.ts の deriveMessageId と同じ u64 messageId を使う (merkleRoot[0..8])。
    // receipt wire format が u64 で運ぶため u64 範囲内に収める必要がある。
    let messageId = 0n;
    for (let i = 0; i < 8 && i < merkleRoot.length; i += 1) {
      messageId = (messageId << 8n) | BigInt(merkleRoot[i]);
    }
    return {
      messageId,
      blockNumber,
      recipientStealth: recipientStealthSs58 as AccountId,
      merkleRoot,
      paddingBucket,
      totalCostMoral: preFundAmount,
    };
  } finally {
    // ---- Step 9: zeroize sender stealth seed (CT-1, FR-021) ----
    // JS 側のコピー (`secret_seed` getter が wasm-bindgen 越しに作った Uint8Array)
    // をゼロ化。
    senderStealthSeed.fill(0);
    // Rust 線形メモリ上の `Zeroizing<[u8; 32]>` を drop で memset(0) する。
    // wasm-bindgen 自動生成の `free()` が Rust の Drop を呼ぶ。
    try {
      (stealth as unknown as { free?: () => void }).free?.();
    } catch {
      // free が無いビルド (テスト mock 等) では no-op
    }
  }
}


/**
 * 32B のランダム X25519 priv。`crypto.getRandomValues` が無い環境 (古い Node.js)
 * では Node の `crypto` を fallback に使う。
 */
function randomEphemeralPriv(): Uint8Array {
  const out = new Uint8Array(32);
  if (typeof crypto !== 'undefined' && crypto.getRandomValues) {
    crypto.getRandomValues(out);
  } else {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const nodeCrypto = require('crypto') as typeof import('crypto');
    nodeCrypto.randomFillSync(out);
  }
  return out;
}

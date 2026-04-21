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
 *  4. wasm `dm_fragment_ciphertext` → fragments + merkle_root.
 *  5. Upload fragments to storage-node(s); require k ACKs within 30s window or
 *     throw `DmError.StorageInsufficient`.
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
import type { AccountId, DmMetaAddress, SendDmParams, SendDmResult } from './types';
import { DmError } from './types';

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
 * パラメータ - PAPI api / wallet signer / storage-node エンドポイントなど IO を
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
  /** Storage-node JSON-RPC エンドポイント。例: `http://127.0.0.1:3030`。 */
  storageEndpoint: string;
  /** Storage-node 認証用 raw Sr25519 signer。`X-Anarchy-Auth` / `X-Chain-Auth`
   *  ヘッダー署名に使う (substrate context)。PolkadotSigner.signBytes は
   *  `<Bytes>` wrap をするため不適合。省略時は認証ヘッダ無しで送信 (auth 無効な
   *  dev storage-node でのみ動作)。 */
  storageSigner?: StorageSigner;
  /** UI 進捗報告コールバック (FR-025)。 */
  onProgress?: (step: SendDmProgress) => void;
}

/**
 * Storage-node 認証用の raw sr25519 signer。`@polkadot/keyring` の
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

/**
 * storage-node 認証ヘッダー (`X-Anarchy-Auth` / `X-Chain-Auth`) を作成する。
 *
 * X-Anarchy-Auth:
 *   message = publicKey(32B) || timestamp_le(8B) || nonce(16B) || payloadHash(32B)
 *   payloadHash = blake2b256(JSON.stringify(sortedParamsWithoutAuth))
 *
 * X-Chain-Auth:
 *   message = utf8("chain-auth:{timestamp}:{method}")
 *   (impersonation 許容 = どの sr25519 鍵でも可)
 *
 * どちらも sr25519 raw 署名 (schnorrkel + "substrate" signing context)。
 * `@polkadot/keyring` の `pair.sign` がこの形式。
 */
async function buildStorageAuthHeaders(
  signer: StorageSigner,
  params: Record<string, unknown>,
  method: string,
): Promise<Record<string, string>> {
  const { blake2b } = await import('blakejs');

  const timestamp = Math.floor(Date.now() / 1000);
  const nonce = new Uint8Array(16);
  crypto.getRandomValues(nonce);

  // NOTE: storage-node の `extract_and_hash_params` は serde_json のデフォルト
  //  (= キーを alphabetical 順に) でシリアライズ後に Blake2b-256 を計算する。
  //  フロント側で明示的に sort してから JSON.stringify する必要がある。
  const sortedParams = Object.keys(params)
    .sort()
    .reduce<Record<string, unknown>>((acc, key) => {
      acc[key] = params[key];
      return acc;
    }, {});
  const paramsJson = JSON.stringify(sortedParams);
  const payloadHash = blake2b(new TextEncoder().encode(paramsJson), undefined, 32);

  // X-Anarchy-Auth: account_id(32) || timestamp_le_u64(8) || nonce(16) || payload_hash(32)
  const authMsg = new Uint8Array(32 + 8 + 16 + 32);
  authMsg.set(signer.publicKey, 0);
  new DataView(authMsg.buffer).setBigUint64(32, BigInt(timestamp), true);
  authMsg.set(nonce, 40);
  authMsg.set(payloadHash, 56);
  const anarchySig = await signer.sign(authMsg);

  const xAnarchyAuth = JSON.stringify({
    account_id: toHex(signer.publicKey),
    timestamp,
    nonce: toHex(nonce),
    payload_hash: toHex(payloadHash),
    signature: toHex(anarchySig),
  });

  // X-Chain-Auth: "chain-auth:{timestamp}:{method}" の utf8 署名
  const chainMsg = new TextEncoder().encode(`chain-auth:${timestamp}:${method}`);
  const chainSig = await signer.sign(chainMsg);
  const xChainAuth = JSON.stringify({
    public_key: toHex(signer.publicKey),
    timestamp,
    method,
    signature: toHex(chainSig),
  });

  return {
    'X-Anarchy-Auth': xAnarchyAuth,
    'X-Chain-Auth': xChainAuth,
  };
}

/**
 * Storage-node に fragments を並列アップロードする。`k` 個 ACK 達成で OK。
 * 30 秒の retry 窓内に届かなければ `DmError.StorageInsufficient` を throw。
 *
 * **MVP 版**: 単一 storage-node エンドポイントに n フラグメントを送る (k=n=1
 * 等の構成でも動作)。複数エンドポイントへの fan-out は後続タスクで拡張する。
 *
 * `signer` を渡すと `storage_storeFragment` に必要な `X-Anarchy-Auth` /
 * `X-Chain-Auth` ヘッダを付加する (FR-201)。省略すると認証無しで送信する。
 */
export async function uploadFragments(
  endpoint: string,
  merkleRoot: Uint8Array,
  fragments: Array<{ index: number; data: Uint8Array }>,
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
        // StoreFragmentParams (storage-node): merkle_root = [u8; 32] なので
        // JSON 上は number 配列で送る。proof / total_leaves は必須フィールド
        // (実装側では現状検証していないが、serde デシリアライズでは required)。
        const params: Record<string, unknown> = {
          merkle_root: Array.from(merkleRoot),
          index: f.index,
          data: toBase64(f.data),
          proof: toBase64(new Uint8Array()),
          total_leaves: totalLeaves,
        };

        const headers: Record<string, string> = {
          'Content-Type': 'application/json',
        };
        if (signer) {
          const authHeaders = await buildStorageAuthHeaders(
            signer,
            params,
            'storage_storeFragment',
          );
          Object.assign(headers, authHeaders);
        }

        const res = await fetchFn(`${endpoint}/`, {
          method: 'POST',
          headers,
          body: JSON.stringify({
            jsonrpc: '2.0',
            id: f.index,
            method: 'storage_storeFragment',
            params,
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
    storageEndpoint,
    storageSigner,
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
  const fragments: Array<{ index: number; data: Uint8Array }> = [];
  for (let i = 0; i < fragmented.fragment_count; i += 1) {
    const frag = fragmented.fragment(i);
    if (!frag) continue;
    fragments.push({ index: i, data: new Uint8Array(frag) });
  }

  // ---- Step 5: storage upload (k-of-n ACK) ----
  onProgress?.({ kind: 'uploading', uploaded: 0, total: fragments.length });
  await uploadFragments(storageEndpoint, merkleRoot, fragments, k, n, {
    signer: storageSigner,
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
      tx1Result = await tx1.signAndSubmit(mainSigner);
    } catch (e) {
      // 残高不足等は MainAccountInsufficientBalance に丸める。
      throw new Error(DmError.MainAccountInsufficientBalance);
    }
    if (!tx1Result.ok) {
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
      throw new Error(DmError.TransactionDropped);
    }
    if (!tx2Result.ok) {
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
    senderStealthSeed.fill(0);
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

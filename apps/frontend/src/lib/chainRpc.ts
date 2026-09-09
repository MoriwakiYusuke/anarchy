/**
 * Chain-node JSON-RPC 共有プラミング。
 *
 * 旧実装では以下が 3〜4 箇所にコピペされていた:
 *   - endpoint 解決 (`NEXT_PUBLIC_WS_ENDPOINT` の ws→http 変換):
 *     useUpload.ts / useFragments.ts / useMediaUpload.ts / lib/dm/sender.ts
 *   - 認証付き upload auth 生成 (byte-identical なメッセージレイアウト):
 *     useUpload.ts generateAuth / lib/dm/sender.ts generateUploadAuth
 *   - base64 codec: postCodec.ts / lib/dm/sender.ts / lib/dm/storageFetch.ts
 *
 * 本モジュールに一本化する。**auth メッセージのレイアウトは chain-node /
 * storage-node が検証するため byte-identical を維持すること**:
 *   署名対象: account_id(32) || timestamp_le_u64(8) || nonce(16) || payload_hash(32)
 *   payload_hash = blake2b256(JSON.stringify(キー alphabetical 順の params))
 *
 * **CLAUDE.md Security Principle #5**: フロントは storage-node (:3030) に直接
 * アクセスしない。すべて chain-node の `storage_*` RPC 経由。
 */

import { blake2b } from 'blakejs';

// ---------------------------------------------------------------------------
// Endpoint 解決
// ---------------------------------------------------------------------------

/**
 * Chain-node JSON-RPC HTTP エンドポイントを解決する。
 *
 * - `override` が最優先。
 * - 無ければ `NEXT_PUBLIC_WS_ENDPOINT` を `ws://`→`http://` に書き換えて使用。
 * - それも無ければ dev fallback の `http://127.0.0.1:9944`。
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
 * フェイルオーバー用の chain-node エンドポイント一覧。
 * ノード追加時はここに追記する (旧 useUpload / useFragments の TODO を引き継ぎ)。
 */
export function getChainRpcEndpoints(): string[] {
  return [
    resolveChainRpcEndpoint(),
    // TODO: Add more full nodes for redundancy
    // 'http://node2.anarchy.network:9944',
    // 'http://node3.anarchy.network:9944',
  ];
}

// ---------------------------------------------------------------------------
// base64 / hex codec
// ---------------------------------------------------------------------------

/**
 * Uint8Array → base64。大きいデータでもスタックオーバーフローしないよう
 * チャンク処理する (storage-node の `b64_decode` と互換)。
 * Node.js (Jest) 環境では Buffer にフォールバック。
 */
export function uint8ArrayToBase64(data: Uint8Array): string {
  if (typeof btoa !== 'function') {
    return Buffer.from(data).toString('base64');
  }
  // 小さいデータは単純変換
  if (data.length < 0x8000) {
    let binary = '';
    for (let i = 0; i < data.length; i++) {
      binary += String.fromCharCode(data[i]);
    }
    return btoa(binary);
  }
  // 大きいデータは 3 の倍数チャンク (base64 は 3 byte → 4 文字) で分割
  const CHUNK_SIZE = 0x6000; // 24KB
  const chunks: string[] = [];
  for (let i = 0; i < data.length; i += CHUNK_SIZE) {
    const end = Math.min(i + CHUNK_SIZE, data.length);
    let binary = '';
    for (let j = i; j < end; j++) {
      binary += String.fromCharCode(data[j]);
    }
    chunks.push(btoa(binary));
  }
  return chunks.join('');
}

/** base64 → Uint8Array。Node.js (Jest) 環境では Buffer にフォールバック。 */
export function base64ToUint8Array(b64: string): Uint8Array {
  if (typeof atob !== 'function') {
    return new Uint8Array(Buffer.from(b64, 'base64'));
  }
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) out[i] = bin.charCodeAt(i);
  return out;
}

/** Uint8Array → lowercase hex 文字列。 */
export function toHex(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 1) s += bytes[i].toString(16).padStart(2, '0');
  return s;
}

// ---------------------------------------------------------------------------
// 認証付き JSON-RPC
// ---------------------------------------------------------------------------

export interface SignedAuth {
  account_id: string;
  timestamp: number;
  nonce: string;
  payload_hash: string;
  signature: string;
}

/**
 * upload auth 署名用の raw sr25519 signer。`@polkadot/keyring` の
 * `pair.sign(msg)` をそのまま渡す想定。
 */
export interface StorageSigner {
  publicKey: Uint8Array;
  sign: (message: Uint8Array) => Uint8Array | Promise<Uint8Array>;
}

/**
 * Storage-node middleware が書き込み API で要求する `X-Anarchy-Auth` を作る。
 * chain-node 経由で forward される際、chain-node が `params.auth` を取り出して
 * ヘッダに移し替える (apps/blockchain/node/src/rpc/storage.rs)。
 *
 * **レイアウト変更厳禁** (chain-node / storage-node が byte 単位で検証する):
 *   message = account_id(32) || timestamp_le_u64(8) || nonce(16) || payload_hash(32)
 *   payload_hash = blake2b256(JSON.stringify(sorted params))
 *   (storage-node の `extract_and_hash_params` は serde_json default =
 *    キー alphabetical 順なので、フロントも明示的に sort 必須)
 */
export async function generateUploadAuth(
  signer: StorageSigner,
  params: Record<string, unknown>,
): Promise<SignedAuth> {
  const timestamp = Math.floor(Date.now() / 1000);
  const nonceBytes = new Uint8Array(16);
  crypto.getRandomValues(nonceBytes);

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

export interface ChainRpcOptions {
  /** 接続先 endpoint 一覧 (順にフェイルオーバー)。省略時は `getChainRpcEndpoints()`。 */
  endpoints?: string[];
  /** fetch 差し替え (テスト用)。 */
  fetchImpl?: typeof fetch;
}

/**
 * Chain-node JSON-RPC 呼び出し (フェイルオーバー付き)。
 * HTTP エラーだけでなく **JSON-RPC error (HTTP 200) も必ず検査**して throw する。
 */
export async function chainRpcCall<T>(
  method: string,
  params: unknown[],
  options: ChainRpcOptions = {},
): Promise<T> {
  const endpoints = options.endpoints ?? getChainRpcEndpoints();
  // ReferenceError 回避: fetch 未定義環境 (古い jsdom 等) では明示エラーにする。
  const fetchFn =
    options.fetchImpl ?? (globalThis as { fetch?: typeof fetch }).fetch;
  if (!fetchFn) throw new Error('fetch is not available in this environment');
  let lastError: Error | null = null;

  for (const endpoint of endpoints) {
    try {
      const response = await fetchFn(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
      });
      if (!response.ok) throw new Error(`http ${response.status}`);
      const json = (await response.json()) as {
        result?: T;
        error?: { message?: string };
      };
      if (json.error) {
        throw new Error(json.error.message || `rpc error: ${JSON.stringify(json.error)}`);
      }
      return json.result as T;
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(String(err));
      // 次の endpoint を試す
      continue;
    }
  }

  throw lastError || new Error('All RPC endpoints unreachable');
}

/**
 * 認証付き JSON-RPC 呼び出し。`signer` があれば `baseParams` への署名を
 * `params.auth` として付加する (chain-node が X-Anarchy-Auth に展開)。
 * 無ければ無認証で送信 (auth disabled な dev storage-node でのみ動作)。
 */
export async function authenticatedRpcCall<T>(
  method: string,
  baseParams: Record<string, unknown>,
  options: ChainRpcOptions & { signer?: StorageSigner } = {},
): Promise<T> {
  const params: Record<string, unknown> = { ...baseParams };
  if (options.signer) {
    params.auth = await generateUploadAuth(options.signer, baseParams);
  }
  return chainRpcCall<T>(method, [params], options);
}

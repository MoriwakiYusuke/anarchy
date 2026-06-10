/**
 * Chain-node `storage_getFragment` 経由で n 個のフラグメントを並列取得し、
 * `dm_fragment_ciphertext` の等分割規則 (chunk = ceil(len/n)) で連結 → 切り詰めて
 * 元の ciphertext を復元するヘルパ。
 *
 * 受信時の DM 本体 (scanner) と添付メディア (media.ts) で共有する。
 *
 * **CLAUDE.md Security Principle #5**: storage-node 直叩きは禁止。chain-node 経由
 * のみ。chain-node 側で fan-out + 認証は完結するので、フロントから X-Chain-Auth
 * を送る必要はない。
 */

import { base64ToUint8Array } from '@/lib/chainRpc';

export interface FetchCiphertextOptions {
  /** Optional fetch override (テスト時の差し替え用)。 */
  fetchImpl?: typeof fetch;
}

/**
 * `merkleRoot` を id とした n 個のフラグメントを取得し、ciphertext を再構成する。
 * 1 つでも取得失敗 (404 / RPC error / GC) すると `null`。
 *
 * @param chainRpcEndpoint chain-node JSON-RPC HTTP エンドポイント
 * @param merkleRoot 32B Blake2b root (DmContentRef.root と同じ値)
 * @param n total fragment count
 * @param ciphertextLen padded ciphertext の長さ (この値で切り詰める)
 */
export async function fetchCiphertextFromStorage(
  chainRpcEndpoint: string,
  merkleRoot: Uint8Array,
  n: number,
  ciphertextLen: number,
  options: FetchCiphertextOptions = {},
): Promise<Uint8Array | null> {
  // ReferenceError 回避: テスト環境 (古い jsdom) では `fetch` が未定義のため、
  // override 未指定時は `globalThis.fetch` を取り、欠けていれば catch 側で null を返す。
  const fetchFn = options.fetchImpl ?? (globalThis as { fetch?: typeof fetch }).fetch;

  const tasks: Promise<Uint8Array | null>[] = [];
  for (let i = 0; i < n; i += 1) {
    tasks.push(
      (async (): Promise<Uint8Array | null> => {
        try {
          if (!fetchFn) return null;
          const res = await fetchFn(chainRpcEndpoint, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              jsonrpc: '2.0',
              id: i,
              method: 'storage_getFragment',
              params: [{
                merkle_root: Array.from(merkleRoot),
                index: i,
              }],
            }),
          });
          if (!res.ok) return null;
          const body = (await res.json()) as {
            result?: { data?: string };
            error?: unknown;
          };
          if (body.error || !body.result?.data) return null;
          return base64ToUint8Array(body.result.data);
        } catch {
          return null;
        }
      })(),
    );
  }
  const parts = await Promise.all(tasks);
  for (const p of parts) if (!p) return null;

  const total = parts.reduce((acc, p) => acc + (p ? p.length : 0), 0);
  const ct = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    if (!p) return null;
    ct.set(p, off);
    off += p.length;
  }
  return ct.length >= ciphertextLen ? ct.slice(0, ciphertextLen) : null;
}

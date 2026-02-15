/**
 * Crypto Web Worker
 *
 * Wasm暗号エンジン(anarchy-wasm-engine)をWeb Worker内で実行し、
 * メインスレッドをブロックせずにKZG-VSS Hybrid分割/復元、MerkleTree構築/検証を行う。
 */

// Worker内でのWasmモジュール
let wasmModule: typeof import("anarchy-wasm-engine") | null = null;

// MerkleResultキャッシュ（Proof生成用）
// key: merkle_root (hex), value: MerkleResult
const merkleCache = new Map<string, import("anarchy-wasm-engine").MerkleResult>();

/**
 * Uint8Arrayをhex文字列に変換
 */
function toHex(arr: Uint8Array): string {
  return Array.from(arr).map(b => b.toString(16).padStart(2, '0')).join('');
}

/**
 * Wasmモジュールを初期化
 */
async function initWasm(): Promise<void> {
  if (wasmModule) return;

  try {
    // Dynamic import (wasm-packでビルドされたモジュール)
    const module = await import("anarchy-wasm-engine");
    // default exportを呼び出してWasmバイナリをロード
    // これにより内部でwasm.__wbindgen_start()が呼ばれ、Rust側のinit()も実行される
    await module.default();
    wasmModule = module;
    console.log("[CryptoWorker] Wasm module initialized");
  } catch (error) {
    console.error("[CryptoWorker] Failed to initialize Wasm:", error);
    throw error;
  }
}

/**
 * メッセージタイプ定義
 */
interface WorkerRequest {
  id: string;
  type: "hybrid_split" | "hybrid_recover" | "merkle_build" | "merkle_generate_proof" | "merkle_verify" | "blake2b_hash";
  payload: unknown;
}

interface WorkerResponse {
  id: string;
  success: boolean;
  result?: unknown;
  error?: string;
}

/**
 * Hybrid分割リクエスト
 */
interface HybridSplitPayload {
  data: Uint8Array;
  k: number;
  n: number;
}

/**
 * Hybrid復元リクエスト
 */
interface HybridRecoverPayload {
  shardBytes: Uint8Array[];
  k: number;
  n: number;
  originalLen: number;
  ciphertextLen: number;
  shardSize: number;
  compressed: boolean;
}

/**
 * MerkleTree構築リクエスト
 */
interface MerkleBuildPayload {
  fragments: Uint8Array[];
}

/**
 * MerkleProof生成リクエスト
 */
interface MerkleGenerateProofPayload {
  merkleRootHex: string;
  index: number;
}

/**
 * MerkleProof検証リクエスト
 */
interface MerkleVerifyPayload {
  root: Uint8Array;
  proof: Uint8Array;
  leafData: Uint8Array;
  leafIndex: number;
  totalLeaves: number;
}

/**
 * Blake2bハッシュリクエスト
 */
interface Blake2bHashPayload {
  data: Uint8Array;
}

/**
 * メッセージハンドラ
 */
self.onmessage = async (event: MessageEvent<WorkerRequest>) => {
  const { id, type, payload } = event.data;

  try {
    await initWasm();

    if (!wasmModule) {
      throw new Error("Wasm module not initialized");
    }

    let result: unknown;

    switch (type) {
      case "hybrid_split": {
        const { data, k, n } = payload as HybridSplitPayload;
        const splitResult = wasmModule.hybrid_split(data, k, n);
        // シャードをシリアライズしてメタデータと共に返す
        const shards: Uint8Array[] = [];
        for (let i = 0; i < splitResult.shard_count; i++) {
          const shard = splitResult.get_shard(i);
          if (shard) {
            shards.push(new Uint8Array(shard.to_bytes()));
          }
        }
        result = {
          shards,
          shardHashes: shards.map((_, i) => {
            const shard = splitResult.get_shard(i);
            return shard ? new Uint8Array(shard.chunk_hash) : new Uint8Array(32);
          }),
          originalLen: splitResult.original_len,
          ciphertextLen: splitResult.ciphertext_len,
          shardSize: splitResult.shard_size,
          compressed: splitResult.compressed,
          threshold: splitResult.threshold,
          totalShards: splitResult.total_shards,
        };
        break;
      }

      case "hybrid_recover": {
        const { shardBytes, k, n, originalLen, ciphertextLen, shardSize, compressed } = payload as HybridRecoverPayload;
        result = new Uint8Array(wasmModule.hybrid_recover(shardBytes, k, n, originalLen, ciphertextLen, shardSize, compressed));
        break;
      }

      case "merkle_build": {
        const { fragments } = payload as MerkleBuildPayload;
        const merkleResult = wasmModule.merkle_build(fragments);
        const root = new Uint8Array(merkleResult.root);
        const rootHex = toHex(root);
        // キャッシュに保存（後でProof生成に使用）
        merkleCache.set(rootHex, merkleResult);
        result = {
          root,
          rootHex,
          leafCount: merkleResult.leaf_count,
        };
        break;
      }

      case "merkle_generate_proof": {
        const { merkleRootHex, index } = payload as MerkleGenerateProofPayload;
        const cachedResult = merkleCache.get(merkleRootHex);
        if (!cachedResult) {
          throw new Error(`MerkleResult not found for root: ${merkleRootHex}`);
        }
        result = new Uint8Array(cachedResult.generate_proof(index));
        break;
      }

      case "merkle_verify": {
        const { root, proof, leafData, leafIndex, totalLeaves } = payload as MerkleVerifyPayload;
        result = wasmModule.merkle_verify(root, proof, leafData, leafIndex, totalLeaves);
        break;
      }

      case "blake2b_hash": {
        const { data } = payload as Blake2bHashPayload;
        result = new Uint8Array(wasmModule.blake2b_hash(data));
        break;
      }

      default:
        throw new Error(`Unknown message type: ${type}`);
    }

    const response: WorkerResponse = { id, success: true, result };
    self.postMessage(response);
  } catch (error) {
    const response: WorkerResponse = {
      id,
      success: false,
      error: error instanceof Error ? error.message : String(error),
    };
    self.postMessage(response);
  }
};

// Workerの初期化完了を通知
self.postMessage({ type: "ready" });

export {};

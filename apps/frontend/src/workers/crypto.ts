/**
 * Crypto Web Worker
 *
 * Wasm暗号エンジン(anarchy-wasm-engine)をWeb Worker内で実行し、
 * メインスレッドをブロックせずにKZG-VSS Hybrid分割/復元、MerkleTree構築/検証を行う。
 * 
 * Also handles PoW mining for reaction mining (017-reaction-mining).
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
 * Count leading zero bits in a byte array (for PoW verification)
 * @param hash - The hash to check
 * @returns Number of leading zero bits
 */
function countLeadingZeroBits(hash: Uint8Array): number {
  let zeroBits = 0;
  for (const byte of hash) {
    if (byte === 0) {
      zeroBits += 8;
    } else {
      // Count leading zeros in this byte
      let b = byte;
      while ((b & 0x80) === 0) {
        zeroBits++;
        b <<= 1;
      }
      break;
    }
  }
  return zeroBits;
}

/**
 * Wasmモジュールを初期化
 * Worker内ではimport.meta.urlが正しく解決されないため、
 * public/wasmフォルダにコピーされたWASMファイルを使用
 */
async function initWasm(): Promise<void> {
  if (wasmModule) return;

  try {
    const module = await import("anarchy-wasm-engine");
    
    // public/wasm/に配置されたWASMファイルを使用
    const wasmUrl = new URL('/wasm/anarchy_wasm_engine_bg.wasm', self.location.origin);
    const response = await fetch(wasmUrl);
    
    if (!response.ok) {
      throw new Error(`Failed to fetch WASM: ${response.status} ${response.statusText}`);
    }
    
    const wasmBytes = await response.arrayBuffer();
    module.initSync({ module: new WebAssembly.Module(wasmBytes) });
    
    wasmModule = module;
    console.log("[CryptoWorker] Wasm module initialized from /wasm/");
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
  type: "hybrid_split" | "hybrid_recover" | "merkle_build" | "merkle_generate_proof" | "merkle_verify" | "blake2b_hash" | "mine_reaction";
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
 * Reaction Mining リクエスト (017-reaction-mining)
 * Client-side PoW mining for reaction submission
 */
interface MineReactionPayload {
  /** Challenge = blake2b(post_id ++ user_address ++ block_number) from chain */
  challenge: Uint8Array;
  /** Required leading zero bits (difficulty) */
  difficulty: number;
  /** Maximum iterations before giving up (0 = unlimited) */
  maxIterations?: number;
}

/**
 * Reaction Mining 結果
 */
interface MineReactionResult {
  /** Found nonce that satisfies difficulty */
  nonce: bigint;
  /** Iterations tried */
  iterations: number;
  /** Hash rate (hashes/second) */
  hashRate: number;
  /** Elapsed time in milliseconds */
  elapsedMs: number;
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

      case "mine_reaction": {
        const { challenge, difficulty, maxIterations = 0 } = payload as MineReactionPayload;
        const startTime = performance.now();
        let nonce = BigInt(0);
        let iterations = 0;
        
        // Pre-allocate buffer for challenge + nonce (challenge + 8 bytes for u64 nonce)
        const inputBuffer = new Uint8Array(challenge.length + 8);
        inputBuffer.set(challenge, 0);
        
        while (true) {
          // Encode nonce as little-endian u64
          const nonceBytes = new Uint8Array(8);
          let n = nonce;
          for (let i = 0; i < 8; i++) {
            nonceBytes[i] = Number(n & BigInt(0xff));
            n >>= BigInt(8);
          }
          inputBuffer.set(nonceBytes, challenge.length);
          
          // Compute Blake2b hash
          const hash = wasmModule.blake2b_hash(inputBuffer);
          iterations++;
          
          // Check leading zero bits
          if (countLeadingZeroBits(new Uint8Array(hash)) >= difficulty) {
            // Found valid nonce!
            const elapsedMs = performance.now() - startTime;
            const hashRate = elapsedMs > 0 ? (iterations / elapsedMs) * 1000 : 0;
            result = {
              nonce,
              iterations,
              hashRate: Math.round(hashRate),
              elapsedMs: Math.round(elapsedMs),
            } as MineReactionResult;
            break;
          }
          
          // Check iteration limit
          if (maxIterations > 0 && iterations >= maxIterations) {
            throw new Error(`Mining failed: reached max iterations (${maxIterations})`);
          }
          
          nonce++;
        }
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

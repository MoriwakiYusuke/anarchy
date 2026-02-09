/**
 * Crypto Web Worker
 *
 * Wasm暗号エンジン(anarchy-wasm-engine)をWeb Worker内で実行し、
 * メインスレッドをブロックせずにSSS分割/復元、MerkleTree構築/検証を行う。
 */

// Worker内でのWasmモジュール
let wasmModule: typeof import("anarchy-wasm-engine") | null = null;

/**
 * Wasmモジュールを初期化
 */
async function initWasm(): Promise<void> {
  if (wasmModule) return;

  try {
    // Dynamic import (wasm-packでビルドされたモジュール)
    wasmModule = await import("anarchy-wasm-engine");
    // Wasmの初期化(panic hookの設定など)
    wasmModule.init();
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
  type: "sss_split" | "sss_recover" | "merkle_build" | "merkle_verify" | "blake2b_hash";
  payload: unknown;
}

interface WorkerResponse {
  id: string;
  success: boolean;
  result?: unknown;
  error?: string;
}

/**
 * SSS分割リクエスト
 */
interface SssSplitPayload {
  data: Uint8Array;
  k: number;
  n: number;
}

/**
 * SSS復元リクエスト
 */
interface SssRecoverPayload {
  shares: Uint8Array[];
  k: number;
}

/**
 * MerkleTree構築リクエスト
 */
interface MerkleBuildPayload {
  fragments: Uint8Array[];
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
      case "sss_split": {
        const { data, k, n } = payload as SssSplitPayload;
        const splitResult = wasmModule.sss_split(data, k, n);
        // SplitResultからUint8Array[]を取得
        result = splitResult.get_all_fragments();
        break;
      }

      case "sss_recover": {
        const { shares, k } = payload as SssRecoverPayload;
        result = wasmModule.sss_recover(shares, k);
        break;
      }

      case "merkle_build": {
        const { fragments } = payload as MerkleBuildPayload;
        const merkleResult = wasmModule.merkle_build(fragments);
        result = {
          root: new Uint8Array(merkleResult.root),
          leafCount: merkleResult.leaf_count,
          // Proofを生成するメソッドを保持（複数回呼ぶ可能性があるため）
          _internal: merkleResult,
        };
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

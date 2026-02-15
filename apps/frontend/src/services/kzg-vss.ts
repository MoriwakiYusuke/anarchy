/**
 * KZG-VSS Service
 *
 * KZG-VSS暗号操作のフロントエンドラッパー。
 * Wasm Engineを使用してクライアントサイドでシェア生成・復元・検証を実行。
 *
 * @module services/kzg-vss
 */

// Wasm Engine imports
import initWasm, {
  kzg_vss_split,
  kzg_vss_recover,
  kzg_verify_proof,
  kzg_init_srs,
  kzg_is_srs_initialized,
  WasmVssSplitResult,
} from 'anarchy-wasm-engine';

/**
 * VSSシェア
 */
export interface VssShare {
  /** シェアインデックス (1..n) */
  index: number;
  /** シェア値 (32 bytes) */
  value: Uint8Array;
}

/**
 * VSS分割結果
 */
export interface VssSplitResult {
  /** KZGコミットメント (48 bytes) */
  commitment: Uint8Array;
  /** 生成されたシェア */
  shares: VssShare[];
  /** 各シェアのKZG proof */
  proofs: Uint8Array[];
  /** 圧縮が適用されたか */
  compressed: boolean;
  /** 元データ長 */
  originalLen: number;
  /** 処理済みデータ長 (圧縮後) */
  processedLen: number;
  /** 複数セグメントか */
  multiSegment: boolean;
  /** セグメント数 */
  segmentCount: number;
}

/**
 * Wasm結果をTypeScript型に変換
 */
function convertWasmResult(wasmResult: WasmVssSplitResult): VssSplitResult {
  const shareCount = wasmResult.share_count;
  const shares: VssShare[] = [];
  const proofs: Uint8Array[] = [];

  for (let i = 0; i < shareCount; i++) {
    const share = wasmResult.get_share(i);
    if (share) {
      shares.push({
        index: share.index,
        value: new Uint8Array(share.value),
      });
      share.free();
    }

    const proof = wasmResult.get_proof(i);
    if (proof) {
      proofs.push(new Uint8Array(proof));
    }
  }

  return {
    commitment: new Uint8Array(wasmResult.commitment),
    shares,
    proofs,
    compressed: wasmResult.compressed,
    originalLen: wasmResult.original_len,
    processedLen: wasmResult.processed_len,
    multiSegment: false, // TODO: Update when multi-segment is implemented
    segmentCount: 1,
  };
}

/**
 * KZG-VSS Service
 */
export class KzgVssService {
  private initialized = false;
  private srsLoaded = false;

  /**
   * Wasm Engineを初期化
   */
  async initialize(): Promise<void> {
    if (this.initialized) return;

    // Initialize Wasm module
    await initWasm();
    this.initialized = true;

    // Load SRS if not already loaded
    if (!this.srsLoaded && !kzg_is_srs_initialized()) {
      await this.loadSrs();
    }
  }

  /**
   * SRS (Trusted Setup) をロード
   *
   * Note: SRSファイルはpublic/srs/に配置する必要がある
   */
  private async loadSrs(): Promise<void> {
    try {
      // Fetch SRS from static assets
      const response = await fetch('/srs/mainnet.bin');
      if (!response.ok) {
        console.warn('SRS file not found at /srs/mainnet.bin, KZG operations will fail');
        return;
      }

      const srsBytes = new Uint8Array(await response.arrayBuffer());
      kzg_init_srs(srsBytes);
      this.srsLoaded = true;
    } catch (error) {
      console.error('Failed to load SRS:', error);
      throw new Error('SRS loading failed');
    }
  }

  /**
   * データをKZG-VSSでシェアに分割
   *
   * @param data - 分割するデータ
   * @param threshold - 復元に必要な最小シェア数 (k)
   * @param shareCount - 生成するシェア数 (n)
   * @returns VssSplitResult
   */
  async split(
    data: Uint8Array,
    threshold: number,
    shareCount: number
  ): Promise<VssSplitResult> {
    await this.initialize();

    const wasmResult = kzg_vss_split(data, threshold, shareCount);
    const result = convertWasmResult(wasmResult);
    wasmResult.free();

    return result;
  }

  /**
   * シェアから元データを復元
   *
   * @param shares - 復元に使用するシェア (k個以上)
   * @param threshold - 復元閾値 (k)
   * @param compressed - 圧縮フラグ
   * @param originalLen - 元データ長
   * @param processedLen - 処理済みデータ長 (圧縮後)
   * @returns 復元されたデータ
   */
  async recover(
    shares: VssShare[],
    threshold: number,
    compressed: boolean,
    originalLen: number,
    processedLen: number
  ): Promise<Uint8Array> {
    await this.initialize();

    // Convert shares to flat format for Wasm
    const indices = new Uint8Array(shares.map((s) => s.index));
    const valuesFlat = new Uint8Array(shares.length * 32);
    shares.forEach((share, i) => {
      valuesFlat.set(share.value, i * 32);
    });

    return kzg_vss_recover(
      indices,
      valuesFlat,
      threshold,
      compressed,
      originalLen,
      processedLen
    );
  }

  /**
   * KZG proofを検証
   *
   * @param commitment - KZGコミットメント
   * @param index - シェアインデックス
   * @param value - シェア値
   * @param proof - KZG proof
   * @returns 検証成功ならtrue
   */
  async verifyProof(
    commitment: Uint8Array,
    index: number,
    value: Uint8Array,
    proof: Uint8Array
  ): Promise<boolean> {
    await this.initialize();

    return kzg_verify_proof(commitment, index, value, proof);
  }
}

// Export singleton instance
export const kzgVssService = new KzgVssService();

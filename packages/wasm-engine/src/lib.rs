//! Anarchy Wasm Engine
//!
//! ブラウザで実行可能なWasm暗号エンジン。
//! - KZG-VSS: BLS12-381曲線上の検証可能秘密分散
//! - Hybrid: AES-256-GCM暗号 + KZG-VSS鍵分散

pub mod kzg;
mod merkle;

use wasm_bindgen::prelude::*;

// パニック時のコンソール出力を有効化
#[cfg(feature = "console_error_panic_hook")]
pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Wasm初期化
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    set_panic_hook();
}

// Re-export Merkle tree APIs (legacy)
pub use merkle::*;

// Re-export KZG-VSS APIs
pub use kzg::{
    compress, decompress, init_srs, is_srs_initialized, regenerate_share, verify_kzg_proof,
    vss_prove, vss_recover, vss_split, KzgCommitment, KzgError, KzgProof, VssShare,
    VssSplitResult, BYTES_PER_SCALAR,
};

// Re-export Wasm bindings
pub use kzg::wasm::{
    hybrid_recover, hybrid_split, kzg_compress, kzg_decompress, kzg_generate_proof,
    kzg_init_srs, kzg_is_srs_initialized, kzg_verify_proof, kzg_vss_recover, kzg_vss_split,
    WasmHybridShard, WasmHybridSplitResult, WasmVssShare, WasmVssSplitResult,
};

//! KZG-VSS Module
//!
//! BLS12-381曲線上でKZG多項式コミットメントを使用した検証可能秘密分散（VSS）を実装。
//!
//! ## モジュール構成
//! - `srs`: Trusted Setup (SRS) ローディング
//! - `encoding`: データ⇔BLS12-381スカラー変換
//! - `compression`: gzip圧縮/解凍
//! - `vss`: シェア生成・復元
//! - `proof`: KZG proof生成・検証
//! - `wasm`: wasm-bindgen バインディング

pub mod compression;
pub mod encoding;
pub mod proof;
pub mod srs;
pub mod vss;
pub mod wasm;

// Re-export main types and functions
pub use compression::{compress, decompress};
pub use encoding::BYTES_PER_SCALAR;
pub use proof::{verify_kzg_proof, vss_prove};
pub use srs::{init_srs, is_srs_initialized};
pub use vss::{vss_recover, vss_split, KzgCommitment, KzgProof, VssShare, VssSplitResult};

// Test utility exports (for both unit and integration tests)
#[cfg(any(test, feature = "test-utils"))]
pub use srs::init_test_srs;

/// KZG-VSS関連のエラー型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KzgError {
    /// データが大きすぎる (>32MB)
    DataTooLarge,
    /// 無効な閾値 (k > n or k < 1)
    InvalidThreshold,
    /// SRSが未初期化
    SrsNotLoaded,
    /// シェア数不足 (< k)
    InsufficientShares,
    /// 無効なシェアインデックス
    InvalidShareIndex,
    /// 解凍失敗
    DecompressionFailed,
    /// 無効なコミットメント
    InvalidCommitment,
    /// 無効なproof
    InvalidProof,
    /// エンコーディングエラー
    EncodingError(String),
}

impl core::fmt::Display for KzgError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            KzgError::DataTooLarge => write!(f, "Data exceeds 32MB limit"),
            KzgError::InvalidThreshold => write!(f, "Invalid threshold: k must be <= n and >= 1"),
            KzgError::SrsNotLoaded => write!(f, "SRS not initialized"),
            KzgError::InsufficientShares => write!(f, "Insufficient shares for recovery"),
            KzgError::InvalidShareIndex => write!(f, "Invalid or duplicate share index"),
            KzgError::DecompressionFailed => write!(f, "Decompression failed"),
            KzgError::InvalidCommitment => write!(f, "Invalid KZG commitment"),
            KzgError::InvalidProof => write!(f, "Invalid KZG proof"),
            KzgError::EncodingError(msg) => write!(f, "Encoding error: {}", msg),
        }
    }
}

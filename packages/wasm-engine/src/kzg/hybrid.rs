//! Hybrid Split Module
//!
//! ハイブリッドアプローチの統合API。
//!
//! ## アーキテクチャ
//! ```text
//! [原文データ]
//!      ↓
//! [AES-256-GCM暗号化] ← K_post (32バイトランダム鍵)
//!      ↓
//! [暗号文]
//!      ↓
//! [Reed-Solomon k-of-n erasure coding]
//!      ↓
//! [n個のチャンク] + [各チャンクのBlake2bハッシュ]
//!
//! [K_post]
//!      ↓
//! [SSS k-of-n 分割]
//!      ↓
//! [n個の鍵シェア]
//! ```

use super::{
    compression::{compress, decompress},
    encryption::{decrypt, encrypt, generate_key, EncryptionError},
    key_sss::{key_recover, key_split, KeyShare, KeySssError},
    reed_solomon::{rs_decode_from_shards, rs_encode, RsError, RsShard},
};
use blake2::{Blake2b, Digest};

/// ハイブリッドエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HybridError {
    /// データが空
    EmptyData,
    /// データが大きすぎる
    DataTooLarge,
    /// 無効な閾値
    InvalidThreshold,
    /// 暗号化エラー
    Encryption(String),
    /// Reed-Solomonエラー
    ReedSolomon(String),
    /// 鍵SSS エラー
    KeySss(String),
    /// 復元エラー
    RecoveryFailed(String),
    /// 解凍失敗
    DecompressionFailed,
}

impl core::fmt::Display for HybridError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HybridError::EmptyData => write!(f, "Data cannot be empty"),
            HybridError::DataTooLarge => write!(f, "Data exceeds maximum size"),
            HybridError::InvalidThreshold => write!(f, "Invalid threshold: k must be >= 2 and <= n"),
            HybridError::Encryption(e) => write!(f, "Encryption error: {}", e),
            HybridError::ReedSolomon(e) => write!(f, "Reed-Solomon error: {}", e),
            HybridError::KeySss(e) => write!(f, "Key SSS error: {}", e),
            HybridError::RecoveryFailed(e) => write!(f, "Recovery failed: {}", e),
            HybridError::DecompressionFailed => write!(f, "Decompression failed"),
        }
    }
}

impl From<EncryptionError> for HybridError {
    fn from(e: EncryptionError) -> Self {
        HybridError::Encryption(e.to_string())
    }
}

impl From<RsError> for HybridError {
    fn from(e: RsError) -> Self {
        HybridError::ReedSolomon(e.to_string())
    }
}

impl From<KeySssError> for HybridError {
    fn from(e: KeySssError) -> Self {
        HybridError::KeySss(e.to_string())
    }
}

/// チャンクのハッシュ（検証用）
pub type ChunkHash = [u8; 32];

/// ハイブリッドシェア（チャンク + 鍵シェア）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridShard {
    /// シェアのインデックス (0..n)
    pub index: usize,
    /// Reed-Solomonチャンク
    pub chunk: Vec<u8>,
    /// チャンクのBlake2bハッシュ（検証用）
    pub chunk_hash: ChunkHash,
    /// 鍵シェア
    pub key_share: KeyShare,
}

/// ハイブリッド分割結果
#[derive(Debug, Clone)]
pub struct HybridSplitResult {
    /// n個のシェア
    pub shards: Vec<HybridShard>,
    /// 元データの長さ
    pub original_len: usize,
    /// 圧縮されたか
    pub compressed: bool,
    /// 暗号文の長さ
    pub ciphertext_len: usize,
    /// Reed-Solomonシェアサイズ
    pub shard_size: usize,
    /// 閾値 k
    pub threshold: u8,
    /// 総シェア数 n
    pub total_shards: u8,
}

/// 最大データサイズ（32MB）
pub const MAX_DATA_SIZE: usize = 32 * 1024 * 1024;

/// Blake2b-256ハッシュを計算
fn compute_hash(data: &[u8]) -> ChunkHash {
    let mut hasher = Blake2b::<blake2::digest::consts::U32>::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// データをk-of-nハイブリッドシェアに分割
///
/// # Arguments
/// * `data` - 分割するデータ
/// * `k` - 復元に必要な最小シェア数
/// * `n` - 総シェア数
///
/// # Returns
/// n個のハイブリッドシェア。任意のk個があれば元データを復元可能。
pub fn hybrid_split(data: &[u8], k: u8, n: u8) -> Result<HybridSplitResult, HybridError> {
    // バリデーション
    if data.is_empty() {
        return Err(HybridError::EmptyData);
    }
    if data.len() > MAX_DATA_SIZE {
        return Err(HybridError::DataTooLarge);
    }
    if k < 2 || k > n {
        return Err(HybridError::InvalidThreshold);
    }

    let original_len = data.len();

    // Step 1: 圧縮
    let (processed_data, compressed) = compress(data);

    // Step 2: ランダム鍵を生成してAES-256-GCM暗号化
    let key = generate_key()?;
    let ciphertext = encrypt(&processed_data, &key)?;
    let ciphertext_len = ciphertext.len();

    // Step 3: Reed-Solomon k-of-n エンコード
    let rs_result = rs_encode(&ciphertext, k as usize, n as usize)?;

    // Step 4: 鍵をSSS k-of-n 分割
    let key_result = key_split(&key, k, n)?;

    // Step 5: 各シェアのハッシュを計算してシェアを構築
    let shards: Vec<HybridShard> = rs_result
        .shards
        .into_iter()
        .zip(key_result.shares.into_iter())
        .map(|(rs_shard, key_share)| {
            let chunk_hash = compute_hash(&rs_shard.data);
            HybridShard {
                index: rs_shard.index,
                chunk: rs_shard.data,
                chunk_hash,
                key_share,
            }
        })
        .collect();

    Ok(HybridSplitResult {
        shards,
        original_len,
        compressed,
        ciphertext_len,
        shard_size: rs_result.shard_size,
        threshold: k,
        total_shards: n,
    })
}

/// k個以上のハイブリッドシェアから元データを復元
///
/// # Arguments
/// * `shards` - 利用可能なシェア
/// * `k` - 閾値
/// * `n` - 総シェア数
/// * `original_len` - 元データの長さ
/// * `ciphertext_len` - 暗号文の長さ
/// * `shard_size` - シェアサイズ
/// * `compressed` - 圧縮されていたか
///
/// # Returns
/// 復元されたデータ
pub fn hybrid_recover(
    shards: &[HybridShard],
    k: u8,
    n: u8,
    original_len: usize,
    ciphertext_len: usize,
    shard_size: usize,
    compressed: bool,
) -> Result<Vec<u8>, HybridError> {
    // バリデーション
    if shards.len() < k as usize {
        return Err(HybridError::RecoveryFailed(format!(
            "Insufficient shards: got {}, need {}",
            shards.len(),
            k
        )));
    }

    // Step 1: チャンクのハッシュを検証
    for shard in shards {
        let computed_hash = compute_hash(&shard.chunk);
        if computed_hash != shard.chunk_hash {
            return Err(HybridError::RecoveryFailed(format!(
                "Chunk {} hash mismatch",
                shard.index
            )));
        }
    }

    // Step 2: 鍵を復元
    let key_shares: Vec<KeyShare> = shards.iter().map(|s| s.key_share.clone()).collect();
    let key = key_recover(&key_shares, k)?;

    // Step 3: Reed-Solomonチャンクを復元
    let rs_shards: Vec<RsShard> = shards
        .iter()
        .map(|s| RsShard {
            index: s.index,
            data: s.chunk.clone(),
        })
        .collect();
    let ciphertext = rs_decode_from_shards(&rs_shards, k as usize, n as usize, ciphertext_len, shard_size)?;

    // Step 4: 復号
    let decrypted = decrypt(&ciphertext, &key)?;

    // Step 5: 解凍（必要な場合）
    let result = if compressed {
        decompress(&decrypted).map_err(|_| HybridError::DecompressionFailed)?
    } else {
        decrypted
    };

    // 長さ検証
    if result.len() != original_len {
        return Err(HybridError::RecoveryFailed(format!(
            "Length mismatch: expected {}, got {}",
            original_len,
            result.len()
        )));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_split_recover_roundtrip() {
        let data = b"Hello, Anarchy! This is a test message for hybrid split.";
        let k = 3;
        let n = 5;

        let result = hybrid_split(data, k, n).unwrap();
        assert_eq!(result.shards.len(), n as usize);
        assert_eq!(result.threshold, k);
        assert_eq!(result.total_shards, n);

        // 全シェアで復元
        let recovered = hybrid_recover(
            &result.shards,
            k,
            n,
            result.original_len,
            result.ciphertext_len,
            result.shard_size,
            result.compressed,
        )
        .unwrap();

        assert_eq!(recovered, data);
    }

    #[test]
    fn test_hybrid_recover_with_minimum_shards() {
        let data = b"Secret data for minimum shard recovery test";
        let k = 3;
        let n = 5;

        let result = hybrid_split(data, k, n).unwrap();

        // k個のシェアのみで復元
        let subset: Vec<HybridShard> = result.shards[0..k as usize].to_vec();
        let recovered = hybrid_recover(
            &subset,
            k,
            n,
            result.original_len,
            result.ciphertext_len,
            result.shard_size,
            result.compressed,
        )
        .unwrap();

        assert_eq!(recovered, data);
    }

    #[test]
    fn test_hybrid_recover_with_any_k_shards() {
        let data = b"Any k shards should work for recovery";
        let k = 2;
        let n = 4;

        let result = hybrid_split(data, k, n).unwrap();

        // シェア0と3のみで復元
        let subset = vec![
            result.shards[0].clone(),
            result.shards[3].clone(),
        ];
        let recovered = hybrid_recover(
            &subset,
            k,
            n,
            result.original_len,
            result.ciphertext_len,
            result.shard_size,
            result.compressed,
        )
        .unwrap();

        assert_eq!(recovered, data);
    }

    #[test]
    fn test_hybrid_large_data() {
        // 100KBのデータ
        let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let k = 5;
        let n = 10;

        let result = hybrid_split(&data, k, n).unwrap();

        // シェア 0, 2, 4, 6, 8 のみで復元
        let subset: Vec<HybridShard> = result
            .shards
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, s)| s.clone())
            .collect();

        let recovered = hybrid_recover(
            &subset,
            k,
            n,
            result.original_len,
            result.ciphertext_len,
            result.shard_size,
            result.compressed,
        )
        .unwrap();

        assert_eq!(recovered, data);
    }

    #[test]
    fn test_hybrid_insufficient_shards() {
        let data = b"Not enough shards";
        let k = 3;
        let n = 5;

        let result = hybrid_split(data, k, n).unwrap();

        // k-1個のシェアでは復元不可
        let subset: Vec<HybridShard> = result.shards[0..(k - 1) as usize].to_vec();
        let err = hybrid_recover(
            &subset,
            k,
            n,
            result.original_len,
            result.ciphertext_len,
            result.shard_size,
            result.compressed,
        );

        assert!(err.is_err());
    }

    #[test]
    fn test_hybrid_chunk_hash_verification() {
        let data = b"Test chunk hash verification";
        let k = 2;
        let n = 3;

        let result = hybrid_split(data, k, n).unwrap();

        // チャンクを改ざん
        let mut tampered_shards = result.shards.clone();
        if let Some(last) = tampered_shards[0].chunk.last_mut() {
            *last ^= 0xFF;
        }

        let err = hybrid_recover(
            &tampered_shards,
            k,
            n,
            result.original_len,
            result.ciphertext_len,
            result.shard_size,
            result.compressed,
        );

        assert!(matches!(err, Err(HybridError::RecoveryFailed(_))));
    }

    #[test]
    fn test_hybrid_invalid_threshold() {
        let data = b"test";

        // k = 1 (不可)
        assert!(matches!(
            hybrid_split(data, 1, 3),
            Err(HybridError::InvalidThreshold)
        ));

        // k > n
        assert!(matches!(
            hybrid_split(data, 5, 3),
            Err(HybridError::InvalidThreshold)
        ));
    }

    #[test]
    fn test_hybrid_empty_data() {
        assert!(matches!(
            hybrid_split(b"", 2, 3),
            Err(HybridError::EmptyData)
        ));
    }

    #[test]
    fn test_hybrid_2_of_3() {
        let data = b"Simple 2-of-3 hybrid test";
        let k = 2;
        let n = 3;

        let result = hybrid_split(data, k, n).unwrap();

        // 各ペアで復元可能か確認
        let pairs = [(0, 1), (0, 2), (1, 2)];
        for (a, b) in pairs {
            let subset = vec![
                result.shards[a].clone(),
                result.shards[b].clone(),
            ];
            let recovered = hybrid_recover(
                &subset,
                k,
                n,
                result.original_len,
                result.ciphertext_len,
                result.shard_size,
                result.compressed,
            )
            .unwrap();

            assert_eq!(recovered, data.to_vec(), "Failed with shards {} and {}", a, b);
        }
    }

    #[test]
    fn test_hybrid_compression() {
        // 圧縮可能なデータ（繰り返しパターン）
        let data: Vec<u8> = "ABABABABABABABABABABABABABABAB"
            .repeat(100)
            .as_bytes()
            .to_vec();
        let k = 2;
        let n = 3;

        let result = hybrid_split(&data, k, n).unwrap();
        // 圧縮が適用されるはず
        assert!(result.compressed);

        let recovered = hybrid_recover(
            &result.shards,
            k,
            n,
            result.original_len,
            result.ciphertext_len,
            result.shard_size,
            result.compressed,
        )
        .unwrap();

        assert_eq!(recovered, data);
    }

    #[test]
    fn test_small_data_shard_size_calculation() {
        // Test with small data (< 256 bytes, no compression)
        let data = b"Test";
        let k = 2u8;
        let n = 3u8;
        
        let result = hybrid_split(data, k, n).unwrap();
        
        println!("original_len: {}", result.original_len);
        println!("ciphertext_len: {}", result.ciphertext_len);
        println!("shard_size: {}", result.shard_size);
        println!("compressed: {}", result.compressed);
        
        // Verify fallback calculation:
        // For uncompressed data, ciphertext_len = original_len + 28 (AES-GCM overhead)
        let expected_ciphertext_len = data.len() + 28; // 12 nonce + 16 tag
        assert_eq!(result.compressed, false, "Small data should not be compressed");
        assert_eq!(result.ciphertext_len, expected_ciphertext_len, 
            "Ciphertext length mismatch: {} != {}", result.ciphertext_len, expected_ciphertext_len);
        
        // shard_size = ceil(ciphertext_len / k)
        let expected_shard_size = (expected_ciphertext_len + k as usize - 1) / k as usize;
        assert_eq!(result.shard_size, expected_shard_size,
            "Shard size mismatch: {} != {}", result.shard_size, expected_shard_size);
    }
}

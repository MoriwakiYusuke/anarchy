//! Compression Module
//!
//! gzip圧縮/解凍。256バイト未満はスキップ。

extern crate alloc;

use alloc::vec::Vec;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::{Read, Write};

use super::KzgError;

/// Minimum size for compression (skip if smaller)
pub const MIN_COMPRESS_SIZE: usize = 256;

/// 解凍後データの上限 (32MB = vss/hybrid の MAX_DATA_SIZE と同値)。
/// 解凍爆弾 (decompression bomb) による worker の OOM を防ぐ。
pub const MAX_DECOMPRESSED_SIZE: usize = 32 * 1024 * 1024;

/// Compress data using gzip.
///
/// Returns original data if size < MIN_COMPRESS_SIZE.
///
/// # Arguments
/// * `data` - Raw data to compress
///
/// # Returns
/// * Compressed data (or original if too small)
/// * Boolean indicating whether compression was applied
pub fn compress(data: &[u8]) -> (Vec<u8>, bool) {
    if data.len() < MIN_COMPRESS_SIZE {
        return (data.to_vec(), false);
    }

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    
    if encoder.write_all(data).is_ok() {
        if let Ok(compressed) = encoder.finish() {
            if compressed.len() < data.len() {
                return (compressed, true);
            }
        }
    }
    
    // Compression didn't help or failed, return original
    (data.to_vec(), false)
}

/// Decompress gzip-compressed data.
///
/// 出力は `MAX_DECOMPRESSED_SIZE` (32MB) で打ち切り、超過時はエラーを返す
/// (解凍爆弾対策)。
///
/// # Arguments
/// * `data` - Compressed data
///
/// # Returns
/// * Decompressed data
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, KzgError> {
    // MAX + 1 バイトまでしか読まない limited reader。MAX を超えて 1 バイトでも
    // 出力されたら DecompressionFailed として拒否する。
    let decoder = GzDecoder::new(data);
    let mut limited = decoder.take(MAX_DECOMPRESSED_SIZE as u64 + 1);

    let mut out = Vec::new();
    limited
        .read_to_end(&mut out)
        .map_err(|_| KzgError::DecompressionFailed)?;

    if out.len() > MAX_DECOMPRESSED_SIZE {
        return Err(KzgError::DecompressionFailed);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_small_data_skipped() {
        let small_data = b"Hello";
        let (result, was_compressed) = compress(small_data);
        assert!(!was_compressed);
        assert_eq!(&result, small_data);
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        // Create data large enough to compress
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let (compressed, was_compressed) = compress(&data);
        
        if was_compressed {
            let decompressed = decompress(&compressed).unwrap();
            assert_eq!(decompressed, data);
        }
    }

    #[test]
    fn test_decompress_invalid_fails() {
        let invalid = b"not gzip data";
        assert!(decompress(invalid).is_err());
    }

    #[test]
    fn test_decompress_bomb_rejected() {
        // 32MB + 1 バイトのゼロ列は gzip で数十 KB に縮む (解凍爆弾の最小例)。
        // decompress は上限超過を検出してエラーにしなければならない。
        let bomb_plain = vec![0u8; MAX_DECOMPRESSED_SIZE + 1];
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&bomb_plain).unwrap();
        let bomb = encoder.finish().unwrap();
        assert!(
            bomb.len() < 1024 * 1024,
            "bomb fixture should be small, got {} bytes",
            bomb.len()
        );

        assert_eq!(decompress(&bomb), Err(KzgError::DecompressionFailed));
    }

    #[test]
    fn test_decompress_at_limit_succeeds() {
        // ちょうど 32MB は許容される。
        let plain = vec![0u8; MAX_DECOMPRESSED_SIZE];
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&plain).unwrap();
        let compressed = encoder.finish().unwrap();

        let out = decompress(&compressed).unwrap();
        assert_eq!(out.len(), MAX_DECOMPRESSED_SIZE);
    }
}

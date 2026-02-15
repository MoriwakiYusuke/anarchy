//! Compression Module
//!
//! gzip圧縮/解凍。256バイト未満はスキップ。

extern crate alloc;

use alloc::vec::Vec;
use flate2::write::{GzDecoder, GzEncoder};
use flate2::Compression;
use std::io::Write;

use super::KzgError;

/// Minimum size for compression (skip if smaller)
pub const MIN_COMPRESS_SIZE: usize = 256;

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
/// # Arguments
/// * `data` - Compressed data
///
/// # Returns
/// * Decompressed data
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, KzgError> {
    let mut decoder = GzDecoder::new(Vec::new());
    
    decoder
        .write_all(data)
        .map_err(|_| KzgError::DecompressionFailed)?;
    
    decoder
        .finish()
        .map_err(|_| KzgError::DecompressionFailed)
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
}

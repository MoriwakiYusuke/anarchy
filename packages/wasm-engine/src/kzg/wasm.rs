//! KZG-VSS Wasm Bindings
//!
//! wasm-bindgenによるJavaScript向けバインディング。

use super::{
    compression, srs,
    vss::{self, KzgCommitment, KzgProof, VssShare, VssSplitResult as InternalSplitResult},
    KzgError,
};
use wasm_bindgen::prelude::*;

/// Wasm-friendly VSS Share
#[wasm_bindgen]
pub struct WasmVssShare {
    index: u8,
    value: Vec<u8>,
}

#[wasm_bindgen]
impl WasmVssShare {
    /// Share index (1..=n)
    #[wasm_bindgen(getter)]
    pub fn index(&self) -> u8 {
        self.index
    }

    /// Share value (32 bytes)
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> Vec<u8> {
        self.value.clone()
    }
}

/// Wasm-friendly KZG-VSS split result
#[wasm_bindgen]
pub struct WasmVssSplitResult {
    commitment: Vec<u8>,
    shares: Vec<WasmVssShare>,
    proofs: Vec<Vec<u8>>,
    compressed: bool,
    original_len: usize,
    processed_len: usize,
}

#[wasm_bindgen]
impl WasmVssSplitResult {
    /// KZG commitment (48 bytes, compressed G1 point)
    #[wasm_bindgen(getter)]
    pub fn commitment(&self) -> Vec<u8> {
        self.commitment.clone()
    }

    /// Number of shares
    #[wasm_bindgen(getter)]
    pub fn share_count(&self) -> usize {
        self.shares.len()
    }

    /// Get share by index
    pub fn get_share(&self, idx: usize) -> Option<WasmVssShare> {
        self.shares.get(idx).map(|s| WasmVssShare {
            index: s.index,
            value: s.value.clone(),
        })
    }

    /// Get all share indices
    pub fn get_share_indices(&self) -> Vec<u8> {
        self.shares.iter().map(|s| s.index).collect()
    }

    /// Get all share values as flat bytes
    pub fn get_share_values_flat(&self) -> Vec<u8> {
        self.shares.iter().flat_map(|s| s.value.clone()).collect()
    }

    /// Get proof by index (48 bytes each)
    pub fn get_proof(&self, idx: usize) -> Option<Vec<u8>> {
        self.proofs.get(idx).cloned()
    }

    /// Whether compression was applied
    #[wasm_bindgen(getter)]
    pub fn compressed(&self) -> bool {
        self.compressed
    }

    /// Original data length
    #[wasm_bindgen(getter)]
    pub fn original_len(&self) -> usize {
        self.original_len
    }

    /// Processed data length (after compression)
    #[wasm_bindgen(getter)]
    pub fn processed_len(&self) -> usize {
        self.processed_len
    }

    /// Serialize entire result to bytes for storage/transmission
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();

        // Header: flags (1 byte) + original_len (4 bytes) + processed_len (4 bytes) + share_count (1 byte)
        let flags = if self.compressed { 1u8 } else { 0u8 };
        result.push(flags);
        result.extend(&(self.original_len as u32).to_le_bytes());
        result.extend(&(self.processed_len as u32).to_le_bytes());
        result.push(self.shares.len() as u8);

        // Commitment (48 bytes)
        result.extend(&self.commitment);

        // Shares: index (1 byte) + value (32 bytes) each
        for share in &self.shares {
            result.push(share.index);
            result.extend(&share.value);
        }

        // Proofs: 48 bytes each
        for proof in &self.proofs {
            result.extend(proof);
        }

        result
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<WasmVssSplitResult, JsError> {
        if data.len() < 10 {
            return Err(JsError::new("Data too short"));
        }

        let flags = data[0];
        let compressed = (flags & 1) != 0;
        let original_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
        let processed_len = u32::from_le_bytes([data[5], data[6], data[7], data[8]]) as usize;
        let share_count = data[9] as usize;

        let expected_len = 10 + 48 + share_count * 33 + share_count * 48;
        if data.len() < expected_len {
            return Err(JsError::new("Data too short for declared share count"));
        }

        let commitment = data[10..58].to_vec();

        let mut shares = Vec::with_capacity(share_count);
        let mut offset = 58;
        for _ in 0..share_count {
            let index = data[offset];
            let value = data[offset + 1..offset + 33].to_vec();
            shares.push(WasmVssShare { index, value });
            offset += 33;
        }

        let mut proofs = Vec::with_capacity(share_count);
        for _ in 0..share_count {
            proofs.push(data[offset..offset + 48].to_vec());
            offset += 48;
        }

        Ok(WasmVssSplitResult {
            commitment,
            shares,
            proofs,
            compressed,
            original_len,
            processed_len,
        })
    }
}

impl From<InternalSplitResult> for WasmVssSplitResult {
    fn from(internal: InternalSplitResult) -> Self {
        WasmVssSplitResult {
            commitment: internal.commitment.bytes.to_vec(),
            shares: internal
                .shares
                .into_iter()
                .map(|s| WasmVssShare {
                    index: s.index,
                    value: s.value.to_vec(),
                })
                .collect(),
            proofs: internal.proofs.into_iter().map(|p| p.bytes.to_vec()).collect(),
            compressed: internal.compressed,
            original_len: internal.original_len,
            processed_len: internal.processed_len,
        }
    }
}

fn kzg_error_to_js(e: KzgError) -> JsError {
    JsError::new(&e.to_string())
}

/// Initialize SRS from bytes (must be called before using KZG functions)
#[wasm_bindgen]
pub fn kzg_init_srs(srs_bytes: &[u8]) -> Result<(), JsError> {
    srs::init_srs(srs_bytes).map_err(kzg_error_to_js)
}

/// Check if SRS is initialized
#[wasm_bindgen]
pub fn kzg_is_srs_initialized() -> bool {
    srs::is_srs_initialized()
}

/// Split data into k-of-n VSS shares with KZG commitments
///
/// # Arguments
/// * `data` - Data to split (圧縮後 threshold × 31 バイト以下。超過は DataTooLarge)
/// * `threshold` - Minimum shares needed for recovery (k, must be >= 2)
/// * `share_count` - Total shares to generate (n)
///
/// # Returns
/// * WasmVssSplitResult containing commitment, shares, and proofs
#[wasm_bindgen]
pub fn kzg_vss_split(data: &[u8], threshold: u8, share_count: u8) -> Result<WasmVssSplitResult, JsError> {
    let result = vss::vss_split(data, threshold, share_count).map_err(kzg_error_to_js)?;
    Ok(result.into())
}

/// Recover data from k-of-n VSS shares
///
/// # Arguments
/// * `indices` - Share indices (1-based)
/// * `values_flat` - Share values concatenated (32 bytes each)
/// * `threshold` - Minimum shares needed (k)
/// * `compressed` - Whether data was compressed during split
/// * `original_len` - Original data length (from split result)
/// * `processed_len` - Length of processed data (from split result)
///
/// # Returns
/// * Original data
#[wasm_bindgen]
pub fn kzg_vss_recover(
    indices: &[u8],
    values_flat: &[u8],
    threshold: u8,
    compressed: bool,
    original_len: usize,
    processed_len: usize,
) -> Result<Vec<u8>, JsError> {
    // Parse shares from flat format
    if values_flat.len() % 32 != 0 {
        return Err(JsError::new("Invalid values_flat length: must be multiple of 32"));
    }
    if indices.len() != values_flat.len() / 32 {
        return Err(JsError::new("Mismatched indices and values count"));
    }

    let shares: Vec<VssShare> = indices
        .iter()
        .zip(values_flat.chunks(32))
        .map(|(&index, value)| VssShare {
            index,
            value: value.try_into().unwrap(),
        })
        .collect();

    vss::vss_recover(&shares, threshold, compressed, original_len, processed_len).map_err(kzg_error_to_js)
}

/// Generate a KZG proof for a share (requires polynomial coefficients)
///
/// Note: This is primarily for storage nodes that have access to polynomial data.
/// Frontend typically uses proofs generated during vss_split.
///
/// # Arguments
/// * `commitment_bytes` - KZG commitment (48 bytes)
/// * `share_index` - The share index (1-based)
/// * `share_value` - The share value (32 bytes)
/// * `polynomial_coeffs` - Polynomial coefficients (serialized)
///
/// # Returns
/// * KZG proof (48 bytes)
#[wasm_bindgen]
pub fn kzg_generate_proof(
    commitment_bytes: &[u8],
    share_index: u8,
    share_value: &[u8],
    polynomial_coeffs: &[u8],
) -> Result<Vec<u8>, JsError> {
    if commitment_bytes.len() != 48 {
        return Err(JsError::new("Invalid commitment length: expected 48 bytes"));
    }
    if share_value.len() != 32 {
        return Err(JsError::new("Invalid share value length: expected 32 bytes"));
    }

    let commitment = KzgCommitment {
        bytes: commitment_bytes.try_into().unwrap(),
    };
    let share = VssShare {
        index: share_index,
        value: share_value.try_into().unwrap(),
    };

    super::proof::vss_prove(&commitment, &share, polynomial_coeffs)
        .map(|proof| proof.bytes.to_vec())
        .map_err(kzg_error_to_js)
}

/// Verify a KZG proof
///
/// # Arguments
/// * `commitment_bytes` - KZG commitment (48 bytes)
/// * `share_index` - The share index (1-based)
/// * `share_value` - The share value (32 bytes)
/// * `proof_bytes` - KZG proof (48 bytes)
///
/// # Returns
/// * true if proof is valid
#[wasm_bindgen]
pub fn kzg_verify_proof(
    commitment_bytes: &[u8],
    share_index: u8,
    share_value: &[u8],
    proof_bytes: &[u8],
) -> Result<bool, JsError> {
    if commitment_bytes.len() != 48 {
        return Err(JsError::new("Invalid commitment length: expected 48 bytes"));
    }
    if share_value.len() != 32 {
        return Err(JsError::new("Invalid share value length: expected 32 bytes"));
    }
    if proof_bytes.len() != 48 {
        return Err(JsError::new("Invalid proof length: expected 48 bytes"));
    }

    let commitment = KzgCommitment {
        bytes: commitment_bytes.try_into().unwrap(),
    };
    let value: [u8; 32] = share_value.try_into().unwrap();
    let proof = KzgProof {
        bytes: proof_bytes.try_into().unwrap(),
    };

    super::proof::verify_kzg_proof(&commitment, share_index, &value, &proof).map_err(kzg_error_to_js)
}

/// Compress data using gzip
#[wasm_bindgen]
pub fn kzg_compress(data: &[u8]) -> Vec<u8> {
    compression::compress(data).0
}

/// Decompress gzip data
#[wasm_bindgen]
pub fn kzg_decompress(data: &[u8]) -> Result<Vec<u8>, JsError> {
    compression::decompress(data).map_err(kzg_error_to_js)
}

// ============================================================================
// Hybrid API (AES + Reed-Solomon + Key SSS)
// ============================================================================

use super::hybrid::{self, HybridError, HybridShard, HybridSplitResult as InternalHybridResult};

fn hybrid_error_to_js(e: HybridError) -> JsError {
    JsError::new(&e.to_string())
}

/// Wasm-friendly Hybrid Shard
#[wasm_bindgen]
pub struct WasmHybridShard {
    index: usize,
    chunk: Vec<u8>,
    chunk_hash: Vec<u8>,
    key_share_index: u8,
    key_share_data: Vec<u8>,
}

#[wasm_bindgen]
impl WasmHybridShard {
    /// Shard index (0..n)
    #[wasm_bindgen(getter)]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Chunk data
    #[wasm_bindgen(getter)]
    pub fn chunk(&self) -> Vec<u8> {
        self.chunk.clone()
    }

    /// Chunk hash (Blake2b-256, 32 bytes)
    #[wasm_bindgen(getter)]
    pub fn chunk_hash(&self) -> Vec<u8> {
        self.chunk_hash.clone()
    }

    /// Key share index
    #[wasm_bindgen(getter)]
    pub fn key_share_index(&self) -> u8 {
        self.key_share_index
    }

    /// Key share data
    #[wasm_bindgen(getter)]
    pub fn key_share_data(&self) -> Vec<u8> {
        self.key_share_data.clone()
    }

    /// Serialize shard to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        // index (4 bytes) + chunk_len (4 bytes) + key_share_index (1 byte)
        result.extend(&(self.index as u32).to_le_bytes());
        result.extend(&(self.chunk.len() as u32).to_le_bytes());
        result.push(self.key_share_index);
        // chunk_hash (32 bytes)
        result.extend(&self.chunk_hash);
        // chunk data
        result.extend(&self.chunk);
        // key_share_data_len (4 bytes) + key_share_data
        result.extend(&(self.key_share_data.len() as u32).to_le_bytes());
        result.extend(&self.key_share_data);
        result
    }

    /// Deserialize shard from bytes
    pub fn from_bytes(data: &[u8]) -> Result<WasmHybridShard, JsError> {
        if data.len() < 45 {
            return Err(JsError::new("Data too short for shard"));
        }

        let index = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let chunk_len = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let key_share_index = data[8];
        let chunk_hash = data[9..41].to_vec();

        if data.len() < 45 + chunk_len {
            return Err(JsError::new("Data too short for chunk"));
        }

        let chunk = data[41..41 + chunk_len].to_vec();
        let key_data_offset = 41 + chunk_len;

        if data.len() < key_data_offset + 4 {
            return Err(JsError::new("Data too short for key share length"));
        }

        let key_share_len = u32::from_le_bytes([
            data[key_data_offset],
            data[key_data_offset + 1],
            data[key_data_offset + 2],
            data[key_data_offset + 3],
        ]) as usize;

        if data.len() < key_data_offset + 4 + key_share_len {
            return Err(JsError::new("Data too short for key share data"));
        }

        let key_share_data = data[key_data_offset + 4..key_data_offset + 4 + key_share_len].to_vec();

        Ok(WasmHybridShard {
            index,
            chunk,
            chunk_hash,
            key_share_index,
            key_share_data,
        })
    }
}

/// Wasm-friendly Hybrid Split Result
#[wasm_bindgen]
pub struct WasmHybridSplitResult {
    shards: Vec<WasmHybridShard>,
    original_len: usize,
    compressed: bool,
    ciphertext_len: usize,
    shard_size: usize,
    threshold: u8,
    total_shards: u8,
}

#[wasm_bindgen]
impl WasmHybridSplitResult {
    /// Number of shards
    #[wasm_bindgen(getter)]
    pub fn shard_count(&self) -> usize {
        self.shards.len()
    }

    /// Get shard by index
    pub fn get_shard(&self, idx: usize) -> Option<WasmHybridShard> {
        self.shards.get(idx).map(|s| WasmHybridShard {
            index: s.index,
            chunk: s.chunk.clone(),
            chunk_hash: s.chunk_hash.clone(),
            key_share_index: s.key_share_index,
            key_share_data: s.key_share_data.clone(),
        })
    }

    /// Original data length
    #[wasm_bindgen(getter)]
    pub fn original_len(&self) -> usize {
        self.original_len
    }

    /// Whether compression was applied
    #[wasm_bindgen(getter)]
    pub fn compressed(&self) -> bool {
        self.compressed
    }

    /// Ciphertext length (for recovery)
    #[wasm_bindgen(getter)]
    pub fn ciphertext_len(&self) -> usize {
        self.ciphertext_len
    }

    /// Shard size (for recovery)
    #[wasm_bindgen(getter)]
    pub fn shard_size(&self) -> usize {
        self.shard_size
    }

    /// Threshold k
    #[wasm_bindgen(getter)]
    pub fn threshold(&self) -> u8 {
        self.threshold
    }

    /// Total shards n
    #[wasm_bindgen(getter)]
    pub fn total_shards(&self) -> u8 {
        self.total_shards
    }

    /// Serialize metadata for storage
    pub fn metadata_to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(18);
        result.extend(&(self.original_len as u32).to_le_bytes());
        result.push(if self.compressed { 1 } else { 0 });
        result.extend(&(self.ciphertext_len as u32).to_le_bytes());
        result.extend(&(self.shard_size as u32).to_le_bytes());
        result.push(self.threshold);
        result.push(self.total_shards);
        result
    }
}

impl From<InternalHybridResult> for WasmHybridSplitResult {
    fn from(internal: InternalHybridResult) -> Self {
        WasmHybridSplitResult {
            shards: internal
                .shards
                .into_iter()
                .map(|s| WasmHybridShard {
                    index: s.index,
                    chunk: s.chunk,
                    chunk_hash: s.chunk_hash.to_vec(),
                    key_share_index: s.key_share.index,
                    key_share_data: s.key_share.data,
                })
                .collect(),
            original_len: internal.original_len,
            compressed: internal.compressed,
            ciphertext_len: internal.ciphertext_len,
            shard_size: internal.shard_size,
            threshold: internal.threshold,
            total_shards: internal.total_shards,
        }
    }
}

/// Split data into k-of-n hybrid shards (AES + Reed-Solomon + Key SSS)
///
/// This is the recommended API for large data. It supports arbitrary data sizes
/// and provides efficient k-of-n recovery.
///
/// # Arguments
/// * `data` - Data to split (max 32MB)
/// * `threshold` - Minimum shards needed for recovery (k, must be >= 2)
/// * `shard_count` - Total shards to generate (n)
///
/// # Returns
/// * WasmHybridSplitResult containing n shards
#[wasm_bindgen]
pub fn hybrid_split(data: &[u8], threshold: u8, shard_count: u8) -> Result<WasmHybridSplitResult, JsError> {
    let result = hybrid::hybrid_split(data, threshold, shard_count).map_err(hybrid_error_to_js)?;
    Ok(result.into())
}

/// Recover data from k-of-n hybrid shards
///
/// # Arguments
/// * `shard_bytes` - Array of serialized shards (from WasmHybridShard.to_bytes())
/// * `threshold` - Minimum shards needed (k)
/// * `total_shards` - Total shard count (n)
/// * `original_len` - Original data length (from split result)
/// * `ciphertext_len` - Ciphertext length (from split result)
/// * `shard_size` - Shard size (from split result)
/// * `compressed` - Whether data was compressed (from split result)
///
/// # Returns
/// * Original data
#[wasm_bindgen]
pub fn hybrid_recover(
    shard_bytes: Vec<js_sys::Uint8Array>,
    threshold: u8,
    total_shards: u8,
    original_len: usize,
    ciphertext_len: usize,
    shard_size: usize,
    compressed: bool,
) -> Result<Vec<u8>, JsError> {
    use super::key_sss::KeyShare;

    // Parse shards from bytes
    let shards: Result<Vec<HybridShard>, JsError> = shard_bytes
        .iter()
        .map(|arr| {
            let bytes = arr.to_vec();
            let wasm_shard = WasmHybridShard::from_bytes(&bytes)?;
            Ok(HybridShard {
                index: wasm_shard.index,
                chunk: wasm_shard.chunk,
                chunk_hash: wasm_shard.chunk_hash.try_into().map_err(|_| {
                    JsError::new("Invalid chunk hash length")
                })?,
                key_share: KeyShare {
                    index: wasm_shard.key_share_index,
                    data: wasm_shard.key_share_data,
                },
            })
        })
        .collect();

    let shards = shards?;

    hybrid::hybrid_recover(
        &shards,
        threshold,
        total_shards,
        original_len,
        ciphertext_len,
        shard_size,
        compressed,
    )
    .map_err(hybrid_error_to_js)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_split_result_serialization_roundtrip() {
        // Create a mock split result
        let result = WasmVssSplitResult {
            commitment: vec![0u8; 48],
            shares: vec![
                WasmVssShare {
                    index: 1,
                    value: vec![1u8; 32],
                },
                WasmVssShare {
                    index: 2,
                    value: vec![2u8; 32],
                },
            ],
            proofs: vec![vec![10u8; 48], vec![20u8; 48]],
            compressed: true,
            original_len: 100,
            processed_len: 80,
        };

        let bytes = result.to_bytes();
        let recovered = WasmVssSplitResult::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.commitment, result.commitment);
        assert_eq!(recovered.share_count(), 2);
        assert_eq!(recovered.compressed, true);
        assert_eq!(recovered.original_len, 100);
        assert_eq!(recovered.processed_len, 80);
    }

    #[test]
    fn test_wasm_hybrid_shard_serialization_roundtrip() {
        let shard = WasmHybridShard {
            index: 2,
            chunk: vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            chunk_hash: vec![0xabu8; 32],
            key_share_index: 3,
            key_share_data: vec![0xcd; 33],
        };

        let bytes = shard.to_bytes();
        let recovered = WasmHybridShard::from_bytes(&bytes).unwrap();

        assert_eq!(recovered.index, shard.index);
        assert_eq!(recovered.chunk, shard.chunk);
        assert_eq!(recovered.chunk_hash, shard.chunk_hash);
        assert_eq!(recovered.key_share_index, shard.key_share_index);
        assert_eq!(recovered.key_share_data, shard.key_share_data);
    }

    #[test]
    fn test_hybrid_split_recover_with_serialization_roundtrip() {
        // This simulates the frontend flow:
        // 1. hybrid_split() -> WasmHybridSplitResult
        // 2. get_shard(i).to_bytes() -> store
        // 3. WasmHybridShard::from_bytes() -> hybrid_recover()

        let data = b"Test data for serialization roundtrip";
        let k = 2u8;
        let n = 3u8;

        // Step 1: Split
        let split_result = hybrid::hybrid_split(data, k, n).unwrap();
        let wasm_result = WasmHybridSplitResult::from(split_result.clone());

        // Step 2: Serialize each shard (simulating storage)
        let serialized_shards: Vec<Vec<u8>> = (0..n as usize)
            .map(|i| {
                let shard = wasm_result.get_shard(i).unwrap();
                shard.to_bytes()
            })
            .collect();

        // Step 3: Deserialize and recover
        let recovered_shards: Vec<HybridShard> = serialized_shards
            .iter()
            .map(|bytes| {
                let wasm_shard = WasmHybridShard::from_bytes(bytes).unwrap();
                HybridShard {
                    index: wasm_shard.index,
                    chunk: wasm_shard.chunk,
                    chunk_hash: wasm_shard.chunk_hash.try_into().unwrap(),
                    key_share: super::super::key_sss::KeyShare {
                        index: wasm_shard.key_share_index,
                        data: wasm_shard.key_share_data,
                    },
                }
            })
            .collect();

        // Verify chunk sizes match shard_size
        for shard in &recovered_shards {
            assert_eq!(
                shard.chunk.len(),
                split_result.shard_size,
                "Chunk size {} != expected shard_size {}",
                shard.chunk.len(),
                split_result.shard_size
            );
        }

        // Step 4: Recover using k shards
        let recovered = hybrid::hybrid_recover(
            &recovered_shards[0..k as usize],
            k,
            n,
            split_result.original_len,
            split_result.ciphertext_len,
            split_result.shard_size,
            split_result.compressed,
        )
        .unwrap();

        assert_eq!(recovered, data);
    }
}

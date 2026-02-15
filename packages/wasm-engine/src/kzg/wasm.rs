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
/// * `data` - Data to split (max 32MB, recommended <32KB for single segment)
/// * `threshold` - Minimum shares needed for recovery (k)
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
}

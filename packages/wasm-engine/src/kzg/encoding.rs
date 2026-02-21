//! Data Encoding for BLS12-381 Scalars
//!
//! データを32バイトチャンクに分割し、BLS12-381スカラー体の元として解釈。

use ark_bls12_381::Fr;
use ark_ff::PrimeField;
use ark_std::vec::Vec;

use super::KzgError;

/// Maximum bytes per scalar (BLS12-381 Fr field)
/// Field modulus is ~253 bits, so we use 31 bytes to be safe
pub const BYTES_PER_SCALAR: usize = 31;

/// Encode raw bytes into BLS12-381 scalar field elements.
///
/// Pads data to BYTES_PER_SCALAR boundary and interprets each chunk as a scalar.
pub fn encode_to_scalars(data: &[u8]) -> Result<Vec<Fr>, KzgError> {
    if data.is_empty() {
        return Err(KzgError::EncodingError("Empty data".into()));
    }

    // Calculate number of chunks needed
    let num_chunks = (data.len() + BYTES_PER_SCALAR - 1) / BYTES_PER_SCALAR;
    let mut scalars = Vec::with_capacity(num_chunks);

    for i in 0..num_chunks {
        let start = i * BYTES_PER_SCALAR;
        let end = core::cmp::min(start + BYTES_PER_SCALAR, data.len());
        
        // Create a 32-byte buffer (padded with zeros)
        let mut chunk = [0u8; 32];
        chunk[..end - start].copy_from_slice(&data[start..end]);
        
        // Interpret as little-endian scalar
        let scalar = Fr::from_le_bytes_mod_order(&chunk);
        scalars.push(scalar);
    }

    Ok(scalars)
}

/// Decode scalar field elements back to raw bytes.
///
/// Reconstructs the original data, removing padding.
pub fn decode_from_scalars(scalars: &[Fr], original_len: usize) -> Result<Vec<u8>, KzgError> {
    if scalars.is_empty() {
        return Err(KzgError::EncodingError("No scalars to decode".into()));
    }

    let mut data = Vec::with_capacity(scalars.len() * BYTES_PER_SCALAR);

    for scalar in scalars {
        // Convert scalar to bytes (little-endian)
        let bytes = scalar_to_bytes(scalar);
        data.extend_from_slice(&bytes[..BYTES_PER_SCALAR]);
    }

    // Truncate to original length
    data.truncate(original_len);

    Ok(data)
}

/// Convert a scalar to 32 bytes (little-endian).
pub fn scalar_to_bytes(scalar: &Fr) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    // BigInt is in little-endian limbs
    let bigint = scalar.into_bigint();
    for (i, limb) in bigint.0.iter().enumerate() {
        let limb_bytes = limb.to_le_bytes();
        let offset = i * 8;
        if offset + 8 <= 32 {
            bytes[offset..offset + 8].copy_from_slice(&limb_bytes);
        }
    }
    bytes
}

/// Convert 32 bytes to a scalar (little-endian, mod order).
pub fn bytes_to_scalar(bytes: &[u8; 32]) -> Fr {
    Fr::from_le_bytes_mod_order(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = b"Hello, KZG-VSS! This is a test message.";
        let scalars = encode_to_scalars(original).unwrap();
        let decoded = decode_from_scalars(&scalars, original.len()).unwrap();
        assert_eq!(&decoded, original);
    }

    #[test]
    fn test_encode_empty_fails() {
        assert!(encode_to_scalars(&[]).is_err());
    }

    #[test]
    fn test_scalar_bytes_roundtrip() {
        let original = Fr::from(12345u64);
        let bytes = scalar_to_bytes(&original);
        let recovered = bytes_to_scalar(&bytes);
        assert_eq!(original, recovered);
    }
}

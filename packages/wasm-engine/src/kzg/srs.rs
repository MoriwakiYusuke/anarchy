//! SRS (Structured Reference String) Loading
//!
//! Ethereum KZG Ceremony (EIP-4844) の Powers of Tau 成果物を使用。

use ark_bls12_381::{G1Affine, G2Affine};
use ark_serialize::CanonicalDeserialize;
use ark_std::vec::Vec;

use super::KzgError;

/// SRS (Structured Reference String) for KZG commitments
pub struct Srs {
    /// Powers of tau in G1: [τ^0]₁, [τ^1]₁, ..., [τ^{n-1}]₁
    pub powers_of_g1: Vec<G1Affine>,
    /// [τ]₂ for pairing verification
    pub tau_g2: G2Affine,
}

/// Global SRS instance (initialized once via init_srs)
static mut GLOBAL_SRS: Option<Srs> = None;

/// Initialize the SRS from raw bytes.
///
/// # Arguments
/// * `srs_bytes` - SRS file bytes (Ethereum KZG Ceremony format)
///
/// # Safety
/// This function sets a global mutable state. Should be called once at startup.
pub fn init_srs(srs_bytes: &[u8]) -> Result<(), KzgError> {
    let srs = load_srs_from_bytes(srs_bytes)?;
    unsafe {
        GLOBAL_SRS = Some(srs);
    }
    Ok(())
}

/// Get reference to the global SRS.
pub fn get_srs() -> Result<&'static Srs, KzgError> {
    unsafe {
        GLOBAL_SRS.as_ref().ok_or(KzgError::SrsNotLoaded)
    }
}

/// Check if SRS is initialized.
pub fn is_srs_initialized() -> bool {
    unsafe { GLOBAL_SRS.is_some() }
}

/// Initialize a test SRS for testing purposes.
///
/// WARNING: This SRS is NOT cryptographically secure. It uses a known tau value.
/// Only use this for unit tests, never for production!
#[cfg(any(test, feature = "test-utils"))]
pub fn init_test_srs(max_degree: usize) -> Result<(), KzgError> {
    use ark_bls12_381::G1Affine;
    use ark_ec::AffineRepr;
    use ark_ff::Field;
    use ark_bls12_381::Fr;

    // Use a deterministic (INSECURE) tau for testing
    // tau = 12345 (just for testing)
    let tau = Fr::from(12345u64);

    // Generate powers of tau in G1: [1]₁, [τ]₁, [τ²]₁, ...
    let g1_generator = G1Affine::generator();
    let mut powers_of_g1 = Vec::with_capacity(max_degree + 1);
    let mut tau_power = Fr::from(1u64);

    for _ in 0..=max_degree {
        let point = (g1_generator * tau_power).into();
        powers_of_g1.push(point);
        tau_power *= tau;
    }

    // Generate [τ]₂
    use ark_bls12_381::G2Affine;
    let g2_generator = G2Affine::generator();
    let tau_g2 = (g2_generator * tau).into();

    let srs = Srs {
        powers_of_g1,
        tau_g2,
    };

    unsafe {
        GLOBAL_SRS = Some(srs);
    }

    Ok(())
}

/// Load SRS from bytes (Ethereum KZG format).
fn load_srs_from_bytes(bytes: &[u8]) -> Result<Srs, KzgError> {
    // Ethereum KZG Ceremony format:
    // - First 48 bytes: number of G1 points (big-endian u64, padded)
    // - G1 points: 48 bytes each (compressed)
    // - G2 point: 96 bytes (compressed)
    
    if bytes.len() < 48 + 96 {
        return Err(KzgError::EncodingError("SRS file too small".into()));
    }

    // For now, we use a simplified format:
    // [4 bytes: num_g1 (u32 LE)] [G1 points] [G2 point]
    
    let num_g1 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let g1_size = 48; // Compressed G1
    let g2_size = 96; // Compressed G2
    let expected_size = 4 + num_g1 * g1_size + g2_size;
    
    if bytes.len() < expected_size {
        return Err(KzgError::EncodingError(format!(
            "SRS file too small: expected {} bytes, got {}",
            expected_size,
            bytes.len()
        )));
    }

    let mut powers_of_g1 = Vec::with_capacity(num_g1);
    let mut offset = 4;

    for _ in 0..num_g1 {
        let point = G1Affine::deserialize_compressed(&bytes[offset..offset + g1_size])
            .map_err(|e| KzgError::EncodingError(format!("Failed to deserialize G1 point: {}", e)))?;
        powers_of_g1.push(point);
        offset += g1_size;
    }

    let tau_g2 = G2Affine::deserialize_compressed(&bytes[offset..offset + g2_size])
        .map_err(|e| KzgError::EncodingError(format!("Failed to deserialize G2 point: {}", e)))?;

    Ok(Srs {
        powers_of_g1,
        tau_g2,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srs_not_initialized() {
        // Reset global state for test
        unsafe {
            GLOBAL_SRS = None;
        }
        assert!(!is_srs_initialized());
        assert!(get_srs().is_err());
    }
}

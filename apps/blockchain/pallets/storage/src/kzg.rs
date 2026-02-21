//! KZG Verification Logic for On-chain Proof Verification
//!
//! This module implements KZG opening proof verification using BLS12-381 curve.
//! Designed for Substrate runtime (no_std compatible).

use ark_bls12_381::{Bls12_381, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup};
use ark_ff::PrimeField;
use ark_serialize::CanonicalDeserialize;

// Import TAU_G2_BYTES from the shared kzg-constants crate
use kzg_constants::TAU_G2_BYTES;

/// BLS12-381 G1 generator (compressed form, 48 bytes)
/// Use this for benchmark test vectors instead of all-zero bytes.
#[cfg(feature = "runtime-benchmarks")]
pub const G1_GENERATOR_COMPRESSED: [u8; 48] = [
    151, 241, 211, 167, 49, 151, 215, 148, 38, 149, 99, 140, 79, 169, 172, 15,
    195, 104, 140, 79, 151, 116, 185, 5, 161, 78, 58, 63, 23, 27, 172, 88,
    108, 85, 232, 63, 249, 122, 26, 239, 251, 58, 240, 10, 219, 34, 198, 187,
];

/// KZG verification error types
#[derive(Debug, Clone, PartialEq)]
pub enum KzgVerifyError {
    /// Invalid commitment format
    InvalidCommitment,
    /// Invalid proof format
    InvalidProof,
    /// Invalid share value format
    InvalidShareValue,
    /// Invalid tau_g2 format
    InvalidTauG2,
    /// Proof verification failed
    VerificationFailed,
}

/// Verify a KZG opening proof on-chain.
///
/// Verifies that proof π is valid for commitment C at point (index, value).
///
/// Verification equation: e(C - [y]₁, [1]₂) = e(π, [τ]₂ - [x]₂)
///
/// # Arguments
/// * `commitment` - KZG commitment (compressed G1, 48 bytes)
/// * `index` - Evaluation point (share index, 1-based)
/// * `share_value` - Claimed evaluation value (32 bytes, BLS12-381 scalar)
/// * `proof` - KZG opening proof (compressed G1, 48 bytes)
///
/// # Returns
/// * `Ok(true)` if proof is valid
/// * `Ok(false)` if proof verification fails
/// * `Err(KzgVerifyError)` if inputs are malformed
pub fn verify_kzg_proof(
    commitment: &[u8; 48],
    index: u8,
    share_value: &[u8; 32],
    proof: &[u8; 48],
) -> Result<bool, KzgVerifyError> {
    // Deserialize commitment
    let c = G1Affine::deserialize_compressed(&commitment[..])
        .map_err(|_| KzgVerifyError::InvalidCommitment)?;

    // Deserialize proof
    let pi = G1Affine::deserialize_compressed(&proof[..])
        .map_err(|_| KzgVerifyError::InvalidProof)?;

    // Deserialize tau_g2 from embedded constant.
    // Note: This deserialization happens on every call. In a no_std runtime environment,
    // static caching via OnceLock/OnceCell requires additional dependencies (lazy_static
    // with spin feature, or once_cell with alloc). The deserialization cost (~1μs) is
    // negligible compared to the pairing operations (~2ms), so we accept this trade-off
    // to keep the runtime dependency-minimal.
    let tau_g2 = G2Affine::deserialize_compressed(&TAU_G2_BYTES[..])
        .map_err(|_| KzgVerifyError::InvalidTauG2)?;

    // Convert share value to scalar (little-endian)
    let y = bytes_to_scalar(share_value);

    // Evaluation point
    let x = Fr::from(index as u64);

    // G1 and G2 generators
    let g1_gen = G1Affine::generator();
    let g2_gen = G2Affine::generator();

    // Compute C - [y]₁ (commitment minus y times generator)
    let y_g1 = (G1Projective::from(g1_gen) * y).into_affine();
    let c_minus_y = (G1Projective::from(c) - G1Projective::from(y_g1)).into_affine();

    // Compute [τ]₂ - [x]₂ (tau_g2 minus x times g2_gen)
    let x_g2 = (G2Projective::from(g2_gen) * x).into_affine();
    let tau_minus_x_g2 = (G2Projective::from(tau_g2) - G2Projective::from(x_g2)).into_affine();

    // Pairing check: e(C - [y]₁, [1]₂) = e(π, [τ - x]₂)
    let lhs = Bls12_381::pairing(c_minus_y, g2_gen);
    let rhs = Bls12_381::pairing(pi, tau_minus_x_g2);

    Ok(lhs == rhs)
}

/// Convert 32-byte array to BLS12-381 scalar (Fr).
/// Uses little-endian byte order.
#[inline]
fn bytes_to_scalar(bytes: &[u8; 32]) -> Fr {
    Fr::from_le_bytes_mod_order(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_scalar_zero() {
        let zero_bytes = [0u8; 32];
        let scalar = bytes_to_scalar(&zero_bytes);
        assert_eq!(scalar, Fr::from(0u64));
    }

    #[test]
    fn test_bytes_to_scalar_one() {
        let mut one_bytes = [0u8; 32];
        one_bytes[0] = 1; // Little-endian
        let scalar = bytes_to_scalar(&one_bytes);
        assert_eq!(scalar, Fr::from(1u64));
    }

    #[test]
    fn test_verify_rejects_invalid_commitment() {
        let invalid_commitment = [0u8; 48]; // All zeros = not on curve
        let share_value = [0u8; 32];
        let proof = [0u8; 48];

        let result = verify_kzg_proof(&invalid_commitment, 1, &share_value, &proof);
        assert!(matches!(result, Err(KzgVerifyError::InvalidCommitment)));
    }

    /// Print G1 generator compressed bytes (for benchmark test vectors)
    #[test]
    fn print_g1_generator_bytes() {
        use ark_serialize::CanonicalSerialize;
        let g1 = G1Affine::generator();
        let mut buf = [0u8; 48];
        g1.serialize_compressed(&mut buf[..]).unwrap();
        println!("G1_GENERATOR_COMPRESSED: {:?}", buf);
    }

    /// Verify TAU_G2_BYTES is a valid G2 point (can be deserialized).
    /// This ensures the embedded Ethereum KZG Ceremony constant is well-formed.
    #[test]
    fn test_tau_g2_validity() {
        use ark_serialize::CanonicalDeserialize;

        // Verify TAU_G2_BYTES deserializes to a valid G2 point
        let result = G2Affine::deserialize_compressed(&TAU_G2_BYTES[..]);
        assert!(
            result.is_ok(),
            "TAU_G2_BYTES failed to deserialize as valid G2 point: {:?}",
            result.err()
        );

        // Verify the point is not the identity (would be useless for verification)
        let point = result.unwrap();
        assert!(
            !point.is_zero(),
            "TAU_G2_BYTES is the identity point, which is invalid"
        );
    }
}

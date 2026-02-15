//! KZG Verification Logic for On-chain Proof Verification
//!
//! This module implements KZG opening proof verification using BLS12-381 curve.
//! Designed for Substrate runtime (no_std compatible).

use ark_bls12_381::{Bls12_381, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup};
use ark_ff::PrimeField;
use ark_serialize::CanonicalDeserialize;

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

/// Embedded tau_g2 from Ethereum KZG Ceremony (Powers of Tau)
/// This is [τ]₂ where τ is the secret from the trusted setup
/// Compressed G2 point (96 bytes)
const TAU_G2_BYTES: [u8; 96] = [
    // Ethereum KZG ceremony tau_g2 point (mainnet KZG_SETUP_G2[1])
    // This is a placeholder - need to use actual value from ceremony
    0x93, 0xe0, 0x2b, 0x60, 0x52, 0x71, 0x9f, 0x60,
    0x7d, 0xac, 0xd3, 0xa0, 0x88, 0x27, 0x4f, 0x65,
    0x59, 0x6b, 0xd0, 0xd0, 0x99, 0x20, 0xb6, 0x1a,
    0xb5, 0xda, 0x61, 0xbb, 0xdc, 0x7f, 0x50, 0x49,
    0x33, 0x4c, 0xf1, 0x12, 0x13, 0x94, 0x5d, 0x57,
    0xe5, 0xac, 0x7d, 0x05, 0x5d, 0x04, 0x2b, 0x7e,
    0x02, 0x4a, 0xa2, 0xb2, 0xf0, 0x8f, 0x0a, 0x91,
    0x26, 0x08, 0x05, 0x27, 0x2d, 0xc5, 0x10, 0x51,
    0xc6, 0xe4, 0x7a, 0xd4, 0xfa, 0x40, 0x3b, 0x02,
    0xb4, 0x51, 0x0b, 0x64, 0x7a, 0xe3, 0xd1, 0x77,
    0x06, 0x34, 0x65, 0x08, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

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

    // Deserialize tau_g2
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
}

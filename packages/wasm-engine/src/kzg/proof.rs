//! KZG Proof Generation and Verification
//!
//! KZG opening proofの生成と検証。

use ark_bls12_381::{Bls12_381, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup};
use ark_poly::DenseUVPolynomial;
use ark_serialize::CanonicalDeserialize;
use ark_std::vec::Vec;

use super::{
    srs::get_srs,
    vss::{KzgCommitment, KzgProof, VssShare},
    KzgError,
};

/// Verify a KZG opening proof.
///
/// Verifies that proof π is valid for commitment C at point (index, value).
///
/// Verification equation: e(C - [y]₁, [1]₂) = e(π, [τ]₂ - [x]₂)
/// Or equivalently: e(C - [y]₁, [1]₂) · e(-π, [τ]₂ - [x]₂) = 1
///
/// # Arguments
/// * `commitment` - KZG commitment to polynomial
/// * `index` - Evaluation point (share index)
/// * `value` - Claimed evaluation value (32 bytes)
/// * `proof` - KZG opening proof
///
/// # Returns
/// * true if proof is valid
pub fn verify_kzg_proof(
    commitment: &KzgCommitment,
    index: u8,
    value: &[u8; 32],
    proof: &KzgProof,
) -> Result<bool, KzgError> {
    let srs = get_srs()?;

    // Deserialize commitment
    let c = G1Affine::deserialize_compressed(&commitment.bytes[..])
        .map_err(|_| KzgError::InvalidCommitment)?;

    // Deserialize proof
    let pi = G1Affine::deserialize_compressed(&proof.bytes[..])
        .map_err(|_| KzgError::InvalidProof)?;

    // Convert value to scalar
    let y = super::encoding::bytes_to_scalar(value);

    // Evaluation point
    let x = Fr::from(index as u64);

    // G1 generator (should be same as srs.powers_of_g1[0] but use standard)
    let g1_gen = G1Affine::generator();
    
    // G2 generator
    let g2_gen = G2Affine::generator();

    // Compute C - [y]₁ (commitment minus y times generator)
    let y_g1 = (G1Projective::from(g1_gen) * y).into_affine();
    let c_minus_y = (G1Projective::from(c) - G1Projective::from(y_g1)).into_affine();

    // Compute [τ]₂ - [x]₂ (tau_g2 minus x times g2_gen)
    let x_g2 = (G2Projective::from(g2_gen) * x).into_affine();
    let tau_minus_x_g2 = (G2Projective::from(srs.tau_g2) - G2Projective::from(x_g2)).into_affine();

    // Pairing check: e(C - [y]₁, [1]₂) = e(π, [τ - x]₂)
    // Equivalent to: e(C - [y]₁, [1]₂) · e(-π, [τ - x]₂) = 1
    let lhs = Bls12_381::pairing(c_minus_y, g2_gen);
    let rhs = Bls12_381::pairing(pi, tau_minus_x_g2);

    Ok(lhs == rhs)
}

/// Generate KZG proof for a specific share.
///
/// This function regenerates a proof given the polynomial coefficients.
/// Storage nodes use this to respond to challenges.
///
/// # Arguments
/// * `commitment` - The KZG commitment (for validation)
/// * `share` - The share to prove
/// * `polynomial_coeffs` - Polynomial coefficients serialized as 32-byte scalars
///
/// # Returns
/// * KZG proof for the share
pub fn vss_prove(
    commitment: &KzgCommitment,
    share: &VssShare,
    polynomial_coeffs: &[u8],
) -> Result<KzgProof, KzgError> {
    use ark_poly::univariate::DensePolynomial;
    
    let srs = get_srs()?;

    // Decode polynomial coefficients
    let coeffs = decode_polynomial_coeffs(polynomial_coeffs)?;
    
    if coeffs.is_empty() {
        return Err(KzgError::EncodingError("Empty polynomial coefficients".into()));
    }

    // Construct polynomial from coefficients
    let polynomial = DensePolynomial::from_coefficients_vec(coeffs);

    // Verify commitment matches polynomial (Issue 9 fix)
    let computed_commitment = super::vss::compute_commitment(&polynomial, srs)?;
    if computed_commitment.bytes != commitment.bytes {
        return Err(KzgError::CommitmentMismatch);
    }

    // Generate proof for this share
    let proof = super::vss::generate_single_proof(&polynomial, share.index, srs)?;

    Ok(proof)
}

/// Decode polynomial coefficients from bytes.
fn decode_polynomial_coeffs(bytes: &[u8]) -> Result<Vec<Fr>, KzgError> {
    if bytes.len() % 32 != 0 {
        return Err(KzgError::EncodingError(
            "Polynomial coefficients must be 32-byte aligned".into(),
        ));
    }

    let mut coeffs = Vec::with_capacity(bytes.len() / 32);
    for chunk in bytes.chunks(32) {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(chunk);
        coeffs.push(super::encoding::bytes_to_scalar(&arr));
    }

    Ok(coeffs)
}

#[cfg(test)]
mod tests {
    // Tests will be implemented in tasks.md T027-T028
}

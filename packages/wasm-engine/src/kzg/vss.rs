//! KZG-VSS: Verifiable Secret Sharing using KZG Commitments
//!
//! 同一の多項式でSSS（シェア生成）とKZG（コミットメント）の両役割を果たす。

use ark_bls12_381::{Fr, G1Projective};
use ark_ec::CurveGroup;
use ark_ff::{Field, PrimeField};
use ark_poly::{univariate::DensePolynomial, DenseUVPolynomial, Polynomial};
use ark_std::vec::Vec;
use ark_std::Zero;

use super::{
    compression::{compress, decompress},
    encoding::{decode_from_scalars, encode_to_scalars, BYTES_PER_SCALAR},
    srs::get_srs,
    KzgError,
};

/// VSS Share
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VssShare {
    /// Share index (1..=n)
    pub index: u8,
    /// Share value (BLS12-381 scalar, 32 bytes)
    pub value: [u8; 32],
}

/// KZG Commitment (compressed G1 point)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KzgCommitment {
    /// Compressed G1 point (48 bytes)
    pub bytes: [u8; 48],
}

/// KZG Proof (compressed G1 point)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KzgProof {
    /// Compressed G1 point (48 bytes)
    pub bytes: [u8; 48],
}

/// Result of vss_split operation
#[derive(Clone, Debug)]
pub struct VssSplitResult {
    /// KZG commitment
    pub commitment: KzgCommitment,
    /// Generated shares (n shares)
    pub shares: Vec<VssShare>,
    /// KZG proofs for each share (n proofs)
    pub proofs: Vec<KzgProof>,
    /// Whether compression was applied
    pub compressed: bool,
    /// Original data length (for reconstruction after decompression)
    pub original_len: usize,
    /// Processed data length (after compression, before encoding)
    pub processed_len: usize,
    /// Multi-segment flag (for >32KB data)
    pub multi_segment: bool,
    /// Number of segments (if multi-segment)
    pub segment_count: usize,
}

/// Maximum data size (32MB)
pub const MAX_DATA_SIZE: usize = 32 * 1024 * 1024;

/// Maximum segment size (32KB worth of scalars)
pub const MAX_SEGMENT_SIZE: usize = 32 * 1024;

/// Split data into k-of-n VSS shares with KZG commitments.
///
/// # Arguments
/// * `data` - Data to split (max 32MB)
/// * `threshold` - Minimum shares needed for recovery (k)
/// * `share_count` - Total shares to generate (n)
///
/// # Returns
/// * VssSplitResult containing commitment, shares, and proofs
pub fn vss_split(data: &[u8], threshold: u8, share_count: u8) -> Result<VssSplitResult, KzgError> {
    // Validate inputs
    if data.is_empty() || data.len() > MAX_DATA_SIZE {
        return Err(KzgError::DataTooLarge);
    }
    if threshold < 1 || threshold > share_count || share_count < 2 {
        return Err(KzgError::InvalidThreshold);
    }

    let srs = get_srs()?;

    // Step 1: Compress data if beneficial
    let (processed_data, compressed) = compress(data);
    let original_len = data.len();

    // Step 2: Encode to scalars
    let scalars = encode_to_scalars(&processed_data)?;

    // Step 3: Construct polynomial with random coefficients for threshold
    // f(x) = a_0 + a_1*x + ... + a_{k-1}*x^{k-1}
    // where a_0 encodes data and remaining coefficients are for threshold
    let polynomial = construct_polynomial(&scalars, threshold as usize)?;

    // Step 4: Compute KZG commitment C = [f(τ)]₁
    let commitment = compute_commitment(&polynomial, srs)?;

    // Step 5: Evaluate polynomial at points 1..=n to get shares
    let mut shares = Vec::with_capacity(share_count as usize);
    for i in 1..=share_count {
        let x = Fr::from(i as u64);
        let y = polynomial.evaluate(&x);
        shares.push(VssShare {
            index: i,
            value: super::encoding::scalar_to_bytes(&y),
        });
    }

    // Step 6: Generate KZG proofs for each share
    let proofs = generate_proofs(&polynomial, &shares, srs)?;

    // Check if multi-segment
    let segment_count = (processed_data.len() + MAX_SEGMENT_SIZE - 1) / MAX_SEGMENT_SIZE;
    let multi_segment = segment_count > 1;
    let processed_len = processed_data.len();

    Ok(VssSplitResult {
        commitment,
        shares,
        proofs,
        compressed,
        original_len,
        processed_len,
        multi_segment,
        segment_count,
    })
}

/// Recover data from k or more shares.
///
/// # Arguments
/// * `shares` - At least k shares for recovery
/// * `threshold` - Minimum shares required (k)
/// * `compressed` - Whether data was compressed (from VssSplitResult)
/// * `original_len` - Original data length (before compression)
/// * `processed_len` - Processed data length (after compression, from VssSplitResult)
///
/// # Returns
/// * Recovered data
pub fn vss_recover(
    shares: &[VssShare],
    threshold: u8,
    compressed: bool,
    original_len: usize,
    processed_len: usize,
) -> Result<Vec<u8>, KzgError> {
    if (shares.len() as u8) < threshold {
        return Err(KzgError::InsufficientShares);
    }

    // Validate share indices (no duplicates, in range)
    let mut seen_indices = [false; 256];
    for share in shares {
        if share.index == 0 {
            return Err(KzgError::InvalidShareIndex);
        }
        if seen_indices[share.index as usize] {
            return Err(KzgError::InvalidShareIndex);
        }
        seen_indices[share.index as usize] = true;
    }

    // Use first `threshold` shares for Lagrange interpolation
    let used_shares = &shares[..threshold as usize];

    // Convert shares to (x, y) points
    let points: Vec<(Fr, Fr)> = used_shares
        .iter()
        .map(|s| {
            let x = Fr::from(s.index as u64);
            let y = super::encoding::bytes_to_scalar(&s.value);
            (x, y)
        })
        .collect();

    // Lagrange interpolate to recover polynomial
    let polynomial = lagrange_interpolate(&points)?;

    // Extract coefficients (data scalars)
    let scalars: Vec<Fr> = polynomial.coeffs().to_vec();

    // Decode scalars back to bytes using the processed_len
    let processed_data = decode_from_scalars(&scalars, processed_len)?;

    // Decompress if necessary
    if compressed {
        decompress(&processed_data)
    } else {
        Ok(processed_data)
    }
}

// === Internal Helper Functions ===

/// Construct polynomial f(x) with guaranteed threshold security.
///
/// For k-of-n threshold secret sharing, the polynomial must have degree k-1.
/// This ensures that k shares are required for Lagrange interpolation.
///
/// The polynomial is constructed as:
///   f(x) = data_0 + data_1*x + ... + data_{m-1}*x^{m-1} + r_m*x^m + ... + r_{k-2}*x^{k-2}
/// where:
///   - data_i are the encoded data scalars
///   - r_i are random coefficients (if needed to reach degree k-1)
///
/// # Arguments
/// * `scalars` - Encoded data as field scalars
/// * `threshold` - Minimum shares needed for recovery (k)
///
/// # Returns
/// * Polynomial of degree max(m-1, k-1) where m = len(scalars)
pub(crate) fn construct_polynomial(scalars: &[Fr], threshold: usize) -> Result<DensePolynomial<Fr>, KzgError> {
    if scalars.is_empty() {
        return Err(KzgError::EncodingError("No scalars for polynomial".into()));
    }
    if threshold < 1 {
        return Err(KzgError::InvalidThreshold);
    }

    let data_count = scalars.len();
    // Polynomial degree must be at least threshold - 1
    let required_coeffs = threshold;
    
    if data_count >= required_coeffs {
        // Data is large enough, threshold security is inherent
        Ok(DensePolynomial::from_coefficients_vec(scalars.to_vec()))
    } else {
        // Data is smaller than threshold, need to pad with random coefficients
        // This ensures exactly k shares are needed for interpolation
        let mut coeffs = scalars.to_vec();
        
        // Generate random padding coefficients
        let padding_count = required_coeffs - data_count;
        let random_coeffs = generate_random_scalars(padding_count)?;
        coeffs.extend(random_coeffs);
        
        Ok(DensePolynomial::from_coefficients_vec(coeffs))
    }
}

/// Generate cryptographically secure random field scalars.
fn generate_random_scalars(count: usize) -> Result<Vec<Fr>, KzgError> {
    let mut scalars = Vec::with_capacity(count);
    
    for _ in 0..count {
        // Generate 32 random bytes
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes)
            .map_err(|e| KzgError::EncodingError(format!("RNG error: {}", e)))?;
        
        // Convert to field element (mod field order)
        let scalar = Fr::from_le_bytes_mod_order(&bytes);
        
        // Ensure non-zero for security (extremely unlikely to be zero, but check anyway)
        if scalar.is_zero() {
            // Retry with different random bytes
            let mut retry_bytes = [0u8; 32];
            retry_bytes[0] = 1; // Set non-zero
            getrandom::getrandom(&mut retry_bytes[1..])
                .map_err(|e| KzgError::EncodingError(format!("RNG error: {}", e)))?;
            scalars.push(Fr::from_le_bytes_mod_order(&retry_bytes));
        } else {
            scalars.push(scalar);
        }
    }
    
    Ok(scalars)
}

/// Compute KZG commitment C = [f(τ)]₁ = Σ(a_i * [τ^i]₁).
pub(super) fn compute_commitment(
    polynomial: &DensePolynomial<Fr>,
    srs: &super::srs::Srs,
) -> Result<KzgCommitment, KzgError> {
    let coeffs = polynomial.coeffs();
    
    if coeffs.len() > srs.powers_of_g1.len() {
        return Err(KzgError::EncodingError(format!(
            "Polynomial degree {} exceeds SRS size {}",
            coeffs.len(),
            srs.powers_of_g1.len()
        )));
    }

    // C = Σ(a_i * [τ^i]₁)
    let mut commitment = G1Projective::zero();
    for (i, coeff) in coeffs.iter().enumerate() {
        commitment += srs.powers_of_g1[i] * coeff;
    }

    // Serialize to compressed form
    let affine = commitment.into_affine();
    let mut bytes = [0u8; 48];
    use ark_serialize::CanonicalSerialize;
    affine
        .serialize_compressed(&mut bytes[..])
        .map_err(|e| KzgError::EncodingError(format!("Failed to serialize commitment: {}", e)))?;

    Ok(KzgCommitment { bytes })
}

/// Generate KZG opening proofs for each share.
///
/// Proof for point (i, f(i)): π_i = [(f(x) - f(i)) / (x - i)]₁
fn generate_proofs(
    polynomial: &DensePolynomial<Fr>,
    shares: &[VssShare],
    srs: &super::srs::Srs,
) -> Result<Vec<KzgProof>, KzgError> {
    let mut proofs = Vec::with_capacity(shares.len());

    for share in shares {
        let proof = generate_single_proof(polynomial, share.index, srs)?;
        proofs.push(proof);
    }

    Ok(proofs)
}

/// Generate a single KZG opening proof for a share.
///
/// This is used by storage nodes to re-prove their holdings.
///
/// # Arguments
/// * `polynomial` - The original polynomial f(x)
/// * `share_index` - The share index (1..=n)
/// * `srs` - Structured Reference String
///
/// # Returns
/// * KZG proof for the share
pub(crate) fn generate_single_proof(
    polynomial: &DensePolynomial<Fr>,
    share_index: u8,
    srs: &super::srs::Srs,
) -> Result<KzgProof, KzgError> {
    let x_i = Fr::from(share_index as u64);
    let y_i = polynomial.evaluate(&x_i);

    // Compute quotient polynomial q(x) = (f(x) - f(i)) / (x - i)
    let quotient = compute_quotient_polynomial(polynomial, x_i, y_i)?;

    // Commit to quotient: π = [q(τ)]₁
    let proof_commitment = compute_commitment(&quotient, srs)?;

    Ok(KzgProof {
        bytes: proof_commitment.bytes,
    })
}

/// Compute quotient polynomial q(x) = (f(x) - y) / (x - point).
fn compute_quotient_polynomial(
    polynomial: &DensePolynomial<Fr>,
    point: Fr,
    value: Fr,
) -> Result<DensePolynomial<Fr>, KzgError> {
    // f(x) - value
    let mut numerator_coeffs = polynomial.coeffs().to_vec();
    if numerator_coeffs.is_empty() {
        numerator_coeffs.push(Fr::zero());
    }
    numerator_coeffs[0] -= value;
    let numerator = DensePolynomial::from_coefficients_vec(numerator_coeffs);

    // Divide by (x - point) using synthetic division
    let divisor = DensePolynomial::from_coefficients_vec(vec![-point, Fr::from(1u64)]);
    
    // Polynomial division
    let (quotient, remainder) = divide_polynomials(&numerator, &divisor)?;
    
    // The remainder should be zero if f(point) = value
    if !remainder.is_zero() {
        return Err(KzgError::EncodingError("Quotient division has non-zero remainder".into()));
    }

    Ok(quotient)
}

/// Divide polynomial a by b, returning (quotient, remainder).
fn divide_polynomials(
    a: &DensePolynomial<Fr>,
    b: &DensePolynomial<Fr>,
) -> Result<(DensePolynomial<Fr>, DensePolynomial<Fr>), KzgError> {
    if b.is_zero() {
        return Err(KzgError::EncodingError("Division by zero polynomial".into()));
    }

    let mut remainder = a.clone();
    // (#35-FINDING-03) propagate errors instead of unwrap-panic in Wasm.
    let b_lead = b
        .coeffs()
        .last()
        .ok_or_else(|| KzgError::EncodingError("Divisor polynomial has no leading coefficient".into()))?;
    let b_lead_inv = b_lead
        .inverse()
        .ok_or_else(|| KzgError::EncodingError("Divisor leading coefficient has no inverse".into()))?;
    let b_degree = b.degree();

    let mut quotient_coeffs = vec![Fr::zero(); a.degree().saturating_sub(b_degree) + 1];

    while !remainder.is_zero() && remainder.degree() >= b_degree {
        let r_lead = *remainder
            .coeffs()
            .last()
            .ok_or_else(|| KzgError::EncodingError("Remainder polynomial empty during division".into()))?;
        let coeff = r_lead * b_lead_inv;
        let shift = remainder.degree() - b_degree;

        quotient_coeffs[shift] = coeff;

        // remainder -= coeff * x^shift * b
        let mut sub_coeffs = vec![Fr::zero(); shift + b.coeffs().len()];
        for (i, &bc) in b.coeffs().iter().enumerate() {
            sub_coeffs[shift + i] = coeff * bc;
        }
        let sub_poly = DensePolynomial::from_coefficients_vec(sub_coeffs);

        // Since DensePolynomial doesn't have Sub trait easily, we subtract manually
        let mut new_remainder_coeffs = remainder.coeffs().to_vec();
        for (i, coeff) in sub_poly.coeffs().iter().enumerate() {
            if i < new_remainder_coeffs.len() {
                new_remainder_coeffs[i] -= coeff;
            }
        }
        // Remove trailing zeros
        while new_remainder_coeffs.last() == Some(&Fr::zero()) {
            new_remainder_coeffs.pop();
        }
        remainder = DensePolynomial::from_coefficients_vec(new_remainder_coeffs);
    }

    Ok((
        DensePolynomial::from_coefficients_vec(quotient_coeffs),
        remainder,
    ))
}

/// Lagrange interpolation to recover polynomial from points.
/// 
/// Exposed as pub(crate) for regenerate_share (013-slashing-repair)
pub(crate) fn lagrange_interpolate(points: &[(Fr, Fr)]) -> Result<DensePolynomial<Fr>, KzgError> {
    if points.is_empty() {
        return Err(KzgError::InsufficientShares);
    }

    let n = points.len();
    let mut result = DensePolynomial::from_coefficients_vec(vec![Fr::zero()]);

    for i in 0..n {
        let (x_i, y_i) = points[i];

        // Compute Lagrange basis polynomial L_i(x) = Π_{j≠i} (x - x_j) / (x_i - x_j)
        let mut basis = DensePolynomial::from_coefficients_vec(vec![Fr::from(1u64)]);
        let mut denominator = Fr::from(1u64);

        for j in 0..n {
            if i != j {
                let (x_j, _) = points[j];
                // basis *= (x - x_j)
                let factor = DensePolynomial::from_coefficients_vec(vec![-x_j, Fr::from(1u64)]);
                basis = multiply_polynomials(&basis, &factor);
                // denominator *= (x_i - x_j)
                denominator *= x_i - x_j;
            }
        }

        // Scale by y_i / denominator (#35-FINDING-03: propagate inverse failure)
        let denom_inv = denominator
            .inverse()
            .ok_or_else(|| KzgError::EncodingError("Lagrange basis denominator is zero (duplicate x)".into()))?;
        let scale = y_i * denom_inv;
        let scaled: Vec<Fr> = basis.coeffs().iter().map(|c| *c * scale).collect();
        let term = DensePolynomial::from_coefficients_vec(scaled);

        // Add to result
        result = add_polynomials(&result, &term);
    }

    Ok(result)
}

/// Multiply two polynomials.
fn multiply_polynomials(
    a: &DensePolynomial<Fr>,
    b: &DensePolynomial<Fr>,
) -> DensePolynomial<Fr> {
    if a.is_zero() || b.is_zero() {
        return DensePolynomial::from_coefficients_vec(vec![]);
    }

    let a_coeffs = a.coeffs();
    let b_coeffs = b.coeffs();
    let result_len = a_coeffs.len() + b_coeffs.len() - 1;
    let mut result = vec![Fr::zero(); result_len];

    for (i, &ac) in a_coeffs.iter().enumerate() {
        for (j, &bc) in b_coeffs.iter().enumerate() {
            result[i + j] += ac * bc;
        }
    }

    DensePolynomial::from_coefficients_vec(result)
}

/// Add two polynomials.
fn add_polynomials(
    a: &DensePolynomial<Fr>,
    b: &DensePolynomial<Fr>,
) -> DensePolynomial<Fr> {
    let max_len = core::cmp::max(a.coeffs().len(), b.coeffs().len());
    let mut result = vec![Fr::zero(); max_len];

    for (i, &c) in a.coeffs().iter().enumerate() {
        result[i] += c;
    }
    for (i, &c) in b.coeffs().iter().enumerate() {
        result[i] += c;
    }

    // Remove trailing zeros
    while result.last() == Some(&Fr::zero()) && result.len() > 1 {
        result.pop();
    }

    DensePolynomial::from_coefficients_vec(result)
}

/// Regenerate a share at a new index from existing k shares (013-slashing-repair)
///
/// Used by the self-repair protocol to create new shares for replacement nodes
/// using Lagrange interpolation. The new share comes with a KZG proof for
/// on-chain verification.
///
/// # Arguments
/// * `shares` - k or more existing shares from donor nodes
/// * `threshold` - The k value (minimum shares for recovery)
/// * `new_index` - Index for the new share (should be > n, typically 6+)
/// * `commitment` - KZG commitment for proof verification
///
/// # Returns
/// * (VssShare, KzgProof) - The regenerated share and its proof
///
/// # Errors
/// * `InsufficientShares` - Less than threshold shares provided
/// * `InvalidIndex` - new_index conflicts with existing share index
pub fn regenerate_share(
    shares: &[VssShare],
    threshold: u8,
    new_index: u8,
    commitment: &KzgCommitment,
) -> Result<(VssShare, KzgProof), KzgError> {
    // Validate inputs
    if shares.len() < threshold as usize {
        return Err(KzgError::InsufficientShares);
    }
    if new_index == 0 {
        return Err(KzgError::InvalidThreshold);
    }

    // Check that new_index doesn't conflict with existing share indices
    for share in shares {
        if share.index == new_index {
            return Err(KzgError::InvalidThreshold);
        }
    }

    // Get SRS for proof generation
    let srs = super::srs::get_srs()?;

    // Convert shares to points for Lagrange interpolation
    // Use exactly k shares (first k if more provided)
    let points: Vec<(Fr, Fr)> = shares
        .iter()
        .take(threshold as usize)
        .map(|share| {
            let x = Fr::from(share.index as u64);
            let y = Fr::from_le_bytes_mod_order(&share.value);
            (x, y)
        })
        .collect();

    // Recover polynomial via Lagrange interpolation
    let polynomial = lagrange_interpolate(&points)?;

    // (#34-FINDING-02) Verify the recovered polynomial commits to the SAME
    // commitment as the original. Without this check, malicious donor nodes
    // could collude to produce shares that interpolate to an arbitrary
    // polynomial — the resulting "regenerated share" would then be unusable
    // for actual recovery and the new holder would silently hold garbage.
    //
    // Computing C' = [f(τ)]₁ from the interpolated polynomial and comparing
    // against the supplied commitment is mathematically equivalent to verifying
    // each donor share with `verify_kzg_proof`, but only requires one G1
    // multi-scalar multiplication instead of N pairings.
    let recomputed = compute_commitment(&polynomial, srs)?;
    if recomputed.bytes != commitment.bytes {
        return Err(KzgError::CommitmentMismatch);
    }

    // Evaluate at new_index to get new share value
    let new_x = Fr::from(new_index as u64);
    let new_y = polynomial.evaluate(&new_x);

    // Convert Fr to [u8; 32]
    let mut value = [0u8; 32];
    {
        use ark_ff::biginteger::BigInteger256;
        let bigint: BigInteger256 = new_y.into();
        for (i, limb) in bigint.0.iter().enumerate() {
            let bytes = limb.to_le_bytes();
            value[i * 8..(i + 1) * 8].copy_from_slice(&bytes);
        }
    }

    let new_share = VssShare {
        index: new_index,
        value,
    };

    // Generate KZG proof for the new share
    let proof = generate_single_proof(&polynomial, new_index, srs)?;

    Ok((new_share, proof))
}

// (#41-FINDING-10) `calculate_processed_len` was unused dead code; removed.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kzg::srs::init_test_srs;

    /// Test that threshold parameter actually controls polynomial degree.
    /// With k=3 threshold, polynomial should have degree >= 2.
    #[test]
    fn test_construct_polynomial_respects_threshold() {
        init_test_srs(16).unwrap();
        
        // Small data: 5 bytes = 1 scalar
        let data = b"Hello";
        let scalars = crate::kzg::encoding::encode_to_scalars(data).unwrap();
        assert_eq!(scalars.len(), 1, "Should encode to 1 scalar");
        
        // Construct polynomial with threshold=3
        let poly = construct_polynomial(&scalars, 3).unwrap();
        
        // Polynomial should have degree >= 2 (threshold - 1)
        assert!(
            poly.degree() >= 2,
            "Polynomial degree {} should be >= 2 for threshold=3",
            poly.degree()
        );
        
        // Verify we have 3 coefficients
        assert_eq!(
            poly.coeffs().len(),
            3,
            "Should have {} coefficients for threshold=3",
            3
        );
    }

    /// Test that threshold security is enforced:
    /// k-1 shares should NOT be able to recover the data.
    #[test]
    fn test_threshold_security_enforced() {
        init_test_srs(16).unwrap();
        
        // Small data that fits in 1 scalar
        let original_data = b"Secret!";
        
        // 3-of-5 split
        let result = vss_split(original_data, 3, 5).unwrap();
        
        // Verify 5 shares were generated
        assert_eq!(result.shares.len(), 5);
        
        // Try to recover with only 2 shares (less than threshold=3)
        // This should fail or return wrong data
        let insufficient_shares = vec![result.shares[0].clone(), result.shares[1].clone()];
        
        let recover_result = vss_recover(
            &insufficient_shares,
            3, // threshold
            result.compressed,
            result.original_len,
            result.processed_len,
        );
        
        // Should fail with InsufficientShares error
        assert!(
            recover_result.is_err(),
            "Should fail with insufficient shares"
        );
        
        // Verify recovery works with exactly 3 shares
        let sufficient_shares = vec![
            result.shares[0].clone(),
            result.shares[1].clone(),
            result.shares[2].clone(),
        ];
        
        let recovered = vss_recover(
            &sufficient_shares,
            3,
            result.compressed,
            result.original_len,
            result.processed_len,
        ).unwrap();
        
        assert_eq!(recovered, original_data, "Should recover original data with k shares");
    }

    /// Test polynomial construction with large data (many scalars)
    #[test]
    fn test_construct_polynomial_large_data_inherent_threshold() {
        init_test_srs(16).unwrap();
        
        // Large data: 100 bytes = 4 scalars (100 / 31 = 3.2 -> 4)
        let data = vec![0xABu8; 100];
        let scalars = crate::kzg::encoding::encode_to_scalars(&data).unwrap();
        assert!(scalars.len() >= 3, "Should encode to at least 3 scalars");
        
        // With threshold=3 and 4 data scalars, no padding needed
        let poly = construct_polynomial(&scalars, 3).unwrap();
        
        // Degree should be data_count - 1
        assert_eq!(
            poly.degree(),
            scalars.len() - 1,
            "Degree should match data scalars when data > threshold"
        );
    }

    // ============ Tests for regenerate_share (013-slashing-repair T012) ============

    /// Test regenerate_share creates valid share at new index
    #[test]
    fn test_regenerate_share_creates_valid_share() {
        init_test_srs(16).unwrap();
        
        // Create 3-of-5 split
        let original_data = b"Self-repair test data";
        let result = vss_split(original_data, 3, 5).unwrap();
        
        // Take first 3 shares (enough to regenerate)
        let donor_shares = vec![
            result.shares[0].clone(),
            result.shares[1].clone(),
            result.shares[2].clone(),
        ];
        
        // Regenerate share at index 6 (beyond original 5)
        let (new_share, _proof) = regenerate_share(
            &donor_shares,
            3,
            6,
            &result.commitment,
        ).unwrap();
        
        assert_eq!(new_share.index, 6, "New share should have index 6");
        
        // Verify the new share works for recovery
        // Use new share + 2 original shares
        let recovery_shares = vec![
            result.shares[0].clone(),
            result.shares[1].clone(),
            new_share.clone(),
        ];
        
        let recovered = vss_recover(
            &recovery_shares,
            3,
            result.compressed,
            result.original_len,
            result.processed_len,
        ).unwrap();
        
        assert_eq!(recovered, original_data, "Should recover with regenerated share");
    }

    /// Test regenerate_share fails with insufficient shares
    #[test]
    fn test_regenerate_share_fails_with_insufficient_shares() {
        init_test_srs(16).unwrap();
        
        let original_data = b"Test";
        let result = vss_split(original_data, 3, 5).unwrap();
        
        // Only 2 shares (less than threshold 3)
        let insufficient_shares = vec![
            result.shares[0].clone(),
            result.shares[1].clone(),
        ];
        
        let regenerate_result = regenerate_share(
            &insufficient_shares,
            3,
            6,
            &result.commitment,
        );
        
        assert!(regenerate_result.is_err(), "Should fail with insufficient shares");
    }

    /// Test regenerate_share fails when new_index conflicts with existing
    #[test]
    fn test_regenerate_share_fails_on_index_conflict() {
        init_test_srs(16).unwrap();
        
        let original_data = b"Conflict test";
        let result = vss_split(original_data, 3, 5).unwrap();
        
        let donor_shares = vec![
            result.shares[0].clone(),
            result.shares[1].clone(),
            result.shares[2].clone(),
        ];
        
        // Try to regenerate at index 1 (conflicts with shares[0])
        let regenerate_result = regenerate_share(
            &donor_shares,
            3,
            1,
            &result.commitment,
        );
        
        assert!(regenerate_result.is_err(), "Should fail on index conflict");
    }

    /// Test regenerate_share works with more than k shares
    #[test]
    fn test_regenerate_share_with_extra_shares() {
        init_test_srs(16).unwrap();
        
        let original_data = b"Extra shares test";
        let result = vss_split(original_data, 3, 5).unwrap();
        
        // Provide all 5 shares (more than threshold 3)
        let all_shares = result.shares.clone();
        
        let (new_share, _proof) = regenerate_share(
            &all_shares,
            3,
            7,
            &result.commitment,
        ).unwrap();
        
        assert_eq!(new_share.index, 7, "Should create share at index 7");
        
        // Verify recovery works
        let recovery_shares = vec![
            result.shares[3].clone(),
            result.shares[4].clone(),
            new_share.clone(),
        ];
        
        let recovered = vss_recover(
            &recovery_shares,
            3,
            result.compressed,
            result.original_len,
            result.processed_len,
        ).unwrap();
        
        assert_eq!(recovered, original_data, "Should recover with new share");
    }
}

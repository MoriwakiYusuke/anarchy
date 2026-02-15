//! KZG Proof Generation Module
//!
//! Generates KZG opening proofs for storage node challenges.
//! Uses arkworks BLS12-381 implementation.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, error};
use anyhow::{Result, bail};

use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::CurveGroup;
use ark_ff::PrimeField;
use ark_poly::univariate::DensePolynomial;
use ark_poly::DenseUVPolynomial;
use ark_serialize::CanonicalSerialize;

use crate::storage::FragmentId;

/// Stored share data for proof generation
#[derive(Clone)]
pub struct StoredShare {
    /// Content hash
    pub content_hash: [u8; 32],
    /// Share index (1-based)
    pub share_index: u8,
    /// Share value (32 bytes)
    pub share_value: [u8; 32],
    /// Polynomial coefficients for proof generation
    /// Each coefficient is 32 bytes (BLS12-381 scalar)
    pub polynomial_coeffs: Vec<u8>,
}

/// KZG Prover for generating holding proofs
pub struct KzgProver {
    /// Stored shares indexed by (content_hash, share_index)
    shares: RwLock<HashMap<([u8; 32], u8), StoredShare>>,
    /// Embedded SRS powers of tau (G1 points)
    /// These are loaded from a trusted setup
    srs_g1: Vec<G1Affine>,
}

impl KzgProver {
    /// Create a new prover with embedded SRS
    pub fn new() -> Self {
        // TODO: Load actual SRS from trusted setup file
        // For now, use empty SRS (will fail proof generation)
        Self {
            shares: RwLock::new(HashMap::new()),
            srs_g1: Vec::new(),
        }
    }

    /// Create a prover with provided SRS (for testing)
    pub fn with_srs(srs_g1: Vec<G1Affine>) -> Self {
        Self {
            shares: RwLock::new(HashMap::new()),
            srs_g1,
        }
    }

    /// Store a share for later proof generation
    pub async fn store_share(&self, share: StoredShare) {
        debug!(
            content_hash = hex::encode(share.content_hash),
            share_index = share.share_index,
            "Storing share for proof generation"
        );
        let key = (share.content_hash, share.share_index);
        let mut shares = self.shares.write().await;
        shares.insert(key, share);
    }

    /// Remove a share (when no longer holding)
    pub async fn remove_share(&self, content_hash: &[u8; 32], share_index: u8) {
        let key = (*content_hash, share_index);
        let mut shares = self.shares.write().await;
        shares.remove(&key);
    }

    /// Generate proof for a challenge
    ///
    /// Returns (share_value, proof_bytes) on success
    pub async fn generate_proof_for_challenge(
        &self,
        content_hash: &[u8; 32],
        share_index: u8,
    ) -> Result<([u8; 32], [u8; 48])> {
        let key = (*content_hash, share_index);

        // Get stored share
        let share = {
            let shares = self.shares.read().await;
            shares.get(&key).cloned()
                .ok_or_else(|| anyhow::anyhow!("Share not found for proof generation"))?
        };

        // Verify we have SRS
        if self.srs_g1.is_empty() {
            bail!("SRS not loaded, cannot generate proof");
        }

        // Decode polynomial coefficients
        let coeffs = self.decode_polynomial_coeffs(&share.polynomial_coeffs)?;

        if coeffs.is_empty() {
            bail!("Empty polynomial coefficients");
        }

        // Construct polynomial from coefficients
        let polynomial = DensePolynomial::from_coefficients_vec(coeffs);

        // Generate proof: π = Σ h_i * [τ^i]₁ where h(x) = (f(x) - f(index)) / (x - index)
        let proof = self.generate_kzg_proof(&polynomial, share_index)?;

        // Serialize proof to compressed form (48 bytes)
        let mut proof_bytes = [0u8; 48];
        proof.serialize_compressed(&mut proof_bytes[..])
            .map_err(|e| anyhow::anyhow!("Failed to serialize proof: {}", e))?;

        info!(
            content_hash = hex::encode(content_hash),
            share_index = share_index,
            "Generated KZG proof successfully"
        );

        Ok((share.share_value, proof_bytes))
    }

    /// Generate KZG opening proof for polynomial at given point
    fn generate_kzg_proof(
        &self,
        polynomial: &DensePolynomial<Fr>,
        index: u8,
    ) -> Result<G1Affine> {
        // Evaluation point
        let x = Fr::from(index as u64);

        // Evaluate f(x)
        let y = self.evaluate_polynomial(polynomial, x);

        // Compute quotient polynomial h(x) = (f(x) - y) / (x - index)
        // This is the polynomial division for KZG proof
        let quotient = self.compute_quotient(polynomial, x, y)?;

        // Commit to quotient: π = Σ h_i * [τ^i]₁
        let proof = self.commit_to_polynomial(&quotient)?;

        Ok(proof)
    }

    /// Evaluate polynomial at a point
    fn evaluate_polynomial(&self, poly: &DensePolynomial<Fr>, x: Fr) -> Fr {
        let mut result = Fr::from(0u64);
        let mut x_power = Fr::from(1u64);

        for coeff in poly.coeffs().iter() {
            result += *coeff * x_power;
            x_power *= x;
        }

        result
    }

    /// Compute quotient polynomial h(x) = (f(x) - y) / (x - z) where f(z) = y
    fn compute_quotient(
        &self,
        polynomial: &DensePolynomial<Fr>,
        z: Fr,
        _y: Fr,
    ) -> Result<DensePolynomial<Fr>> {
        let coeffs = polynomial.coeffs();
        if coeffs.is_empty() {
            bail!("Empty polynomial");
        }

        // Synthetic division: divide by (x - z)
        // If f(z) = y, then (f(x) - y) is divisible by (x - z)
        let n = coeffs.len();
        let mut quotient_coeffs = vec![Fr::from(0u64); n - 1];

        if n > 1 {
            quotient_coeffs[n - 2] = coeffs[n - 1];
            for i in (0..n - 2).rev() {
                quotient_coeffs[i] = coeffs[i + 1] + z * quotient_coeffs[i + 1];
            }
        }

        Ok(DensePolynomial::from_coefficients_vec(quotient_coeffs))
    }

    /// Commit to polynomial using SRS
    fn commit_to_polynomial(&self, polynomial: &DensePolynomial<Fr>) -> Result<G1Affine> {
        let coeffs = polynomial.coeffs();

        if coeffs.len() > self.srs_g1.len() {
            bail!(
                "Polynomial degree {} exceeds SRS size {}",
                coeffs.len() - 1,
                self.srs_g1.len() - 1
            );
        }

        // C = Σ f_i * [τ^i]₁
        let mut commitment = G1Projective::default();
        for (i, coeff) in coeffs.iter().enumerate() {
            commitment += self.srs_g1[i] * coeff;
        }

        Ok(commitment.into_affine())
    }

    /// Decode polynomial coefficients from bytes
    fn decode_polynomial_coeffs(&self, bytes: &[u8]) -> Result<Vec<Fr>> {
        if bytes.len() % 32 != 0 {
            bail!("Polynomial coefficients must be 32-byte aligned");
        }

        let mut coeffs = Vec::with_capacity(bytes.len() / 32);
        for chunk in bytes.chunks(32) {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(chunk);
            coeffs.push(Fr::from_le_bytes_mod_order(&arr));
        }

        Ok(coeffs)
    }

    /// Get number of stored shares
    pub async fn share_count(&self) -> usize {
        self.shares.read().await.len()
    }
}

impl Default for KzgProver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_retrieve_share() {
        let prover = KzgProver::new();

        let share = StoredShare {
            content_hash: [1u8; 32],
            share_index: 1,
            share_value: [2u8; 32],
            polynomial_coeffs: vec![0u8; 64], // 2 coefficients
        };

        prover.store_share(share.clone()).await;
        assert_eq!(prover.share_count().await, 1);

        prover.remove_share(&share.content_hash, share.share_index).await;
        assert_eq!(prover.share_count().await, 0);
    }

    #[tokio::test]
    async fn test_proof_generation_fails_without_srs() {
        let prover = KzgProver::new();

        let share = StoredShare {
            content_hash: [1u8; 32],
            share_index: 1,
            share_value: [2u8; 32],
            polynomial_coeffs: vec![0u8; 64], // 2 coefficients
        };

        prover.store_share(share).await;

        let result = prover.generate_proof_for_challenge(&[1u8; 32], 1).await;
        assert!(result.is_err()); // Should fail because SRS is empty
    }
}

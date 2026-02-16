//! KZG Proof Generation Module
//!
//! Generates KZG opening proofs for storage node challenges.
//! Uses arkworks BLS12-381 implementation.
//!
//! T082: SRS loading implementation

use std::collections::HashMap;
use std::path::Path;
use tokio::sync::RwLock;
use tracing::{info, debug, warn};
use anyhow::{Result, bail, Context};

use ark_bls12_381::{Fr, G1Affine, G1Projective, G2Affine};
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::PrimeField;
use ark_poly::univariate::DensePolynomial;
use ark_poly::DenseUVPolynomial;
use ark_serialize::{CanonicalSerialize, CanonicalDeserialize};



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
    /// [τ]₂ for pairing verification (not used for proof generation, but kept for completeness)
    _tau_g2: Option<G2Affine>,
}

impl KzgProver {
    /// Create a new prover (empty SRS - must call load_srs or init_test_srs)
    pub fn new() -> Self {
        Self {
            shares: RwLock::new(HashMap::new()),
            srs_g1: Vec::new(),
            _tau_g2: None,
        }
    }

    /// Create a prover with provided SRS (for testing)
    pub fn with_srs(srs_g1: Vec<G1Affine>) -> Self {
        Self {
            shares: RwLock::new(HashMap::new()),
            srs_g1,
            _tau_g2: None,
        }
    }

    /// Load SRS from file (T082)
    ///
    /// Supports both binary format and Ethereum KZG Ceremony text format.
    ///
    /// Binary format:
    /// - First 4 bytes: number of G1 points (u32 LE)
    /// - G1 points: 48 bytes each (compressed BLS12-381 G1)
    /// - G2 point: 96 bytes (compressed BLS12-381 G2)
    ///
    /// Text format (Ethereum KZG Ceremony):
    /// - Line 1: number of G1 points
    /// - Line 2: number of G2 points
    /// - Following lines: G1 points as hex strings (48 bytes each)
    /// - Following lines: G2 points as hex strings (96 bytes each)
    pub fn load_srs_from_file(&mut self, path: &Path) -> Result<()> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read SRS file: {:?}", path))?;
        
        // Try to detect format: text files start with ASCII digits
        if bytes.len() > 4 && bytes[0].is_ascii_digit() {
            // Likely text format
            let text = std::str::from_utf8(&bytes)
                .with_context(|| "SRS file appears to be text but is not valid UTF-8")?;
            self.load_srs_from_ceremony_text(text)
        } else {
            // Binary format
            self.load_srs_from_bytes(&bytes)
        }
    }

    /// Load SRS from Ethereum KZG Ceremony text format
    pub fn load_srs_from_ceremony_text(&mut self, text: &str) -> Result<()> {
        let lines: Vec<&str> = text.lines().collect();
        
        if lines.len() < 4 {
            bail!("SRS text too short");
        }
        
        let num_g1: usize = lines[0].trim().parse()
            .with_context(|| "Invalid G1 count")?;
        let num_g2: usize = lines[1].trim().parse()
            .with_context(|| "Invalid G2 count")?;
        
        if num_g1 == 0 {
            bail!("SRS must have at least 1 G1 point");
        }
        if num_g2 < 2 {
            bail!("SRS must have at least 2 G2 points (need G2[1])");
        }
        
        let expected_lines = 2 + num_g1 + num_g2;
        if lines.len() < expected_lines {
            bail!("Expected {} lines, got {}", expected_lines, lines.len());
        }
        
        // Parse G1 points
        let mut powers_of_g1 = Vec::with_capacity(num_g1);
        for i in 0..num_g1 {
            let hex_str = lines[2 + i].trim();
            let bytes = hex::decode(hex_str)
                .with_context(|| format!("G1[{}] hex error", i))?;
            
            if bytes.len() != 48 {
                bail!("G1[{}] wrong size: expected 48 bytes, got {}", i, bytes.len());
            }
            
            let point = G1Affine::deserialize_compressed(&bytes[..])
                .map_err(|e| anyhow::anyhow!("G1[{}] deserialize error: {}", i, e))?;
            powers_of_g1.push(point);
        }
        
        // Parse G2[1] (second G2 point) as tau_g2
        let tau_g2_line = 2 + num_g1 + 1;
        let tau_g2_hex = lines[tau_g2_line].trim();
        let tau_g2_bytes = hex::decode(tau_g2_hex)
            .with_context(|| "tau_g2 hex error")?;
        
        if tau_g2_bytes.len() != 96 {
            bail!("tau_g2 wrong size: expected 96 bytes, got {}", tau_g2_bytes.len());
        }
        
        let tau_g2 = G2Affine::deserialize_compressed(&tau_g2_bytes[..])
            .map_err(|e| anyhow::anyhow!("tau_g2 deserialize error: {}", e))?;
        
        info!("Loaded Ethereum KZG Ceremony SRS with {} G1 points", powers_of_g1.len());
        
        self.srs_g1 = powers_of_g1;
        self._tau_g2 = Some(tau_g2);
        
        Ok(())
    }

    /// Load SRS from bytes (binary format)
    pub fn load_srs_from_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() < 4 + 48 + 96 {
            bail!("SRS file too small: expected at least {} bytes, got {}", 4 + 48 + 96, bytes.len());
        }

        let num_g1 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let g1_size = 48; // Compressed G1
        let g2_size = 96; // Compressed G2
        let expected_size = 4 + num_g1 * g1_size + g2_size;

        if bytes.len() < expected_size {
            bail!("SRS file too small: expected {} bytes for {} G1 points, got {}", 
                  expected_size, num_g1, bytes.len());
        }

        let mut powers_of_g1 = Vec::with_capacity(num_g1);
        let mut offset = 4;

        for i in 0..num_g1 {
            let point = G1Affine::deserialize_compressed(&bytes[offset..offset + g1_size])
                .map_err(|e| anyhow::anyhow!("Failed to deserialize G1 point {}: {}", i, e))?;
            powers_of_g1.push(point);
            offset += g1_size;
        }

        let tau_g2 = G2Affine::deserialize_compressed(&bytes[offset..offset + g2_size])
            .map_err(|e| anyhow::anyhow!("Failed to deserialize G2 point: {}", e))?;

        info!("Loaded SRS with {} G1 points", powers_of_g1.len());
        
        self.srs_g1 = powers_of_g1;
        self._tau_g2 = Some(tau_g2);
        
        Ok(())
    }

    /// Initialize test SRS with deterministic (INSECURE) tau (T082)
    ///
    /// WARNING: This SRS is NOT cryptographically secure. Only for development/testing.
    pub fn init_test_srs(&mut self, max_degree: usize) -> Result<()> {
        warn!("Using INSECURE test SRS - DO NOT use in production!");
        
        // Use deterministic tau = 12345 (INSECURE - same as wasm-engine)
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
        let g2_generator = G2Affine::generator();
        let tau_g2 = (g2_generator * tau).into();

        info!("Initialized test SRS with {} G1 points (INSECURE)", powers_of_g1.len());
        
        self.srs_g1 = powers_of_g1;
        self._tau_g2 = Some(tau_g2);
        
        Ok(())
    }

    /// Check if SRS is loaded
    pub fn is_srs_loaded(&self) -> bool {
        !self.srs_g1.is_empty()
    }

    /// Get SRS degree (max polynomial degree)
    pub fn srs_degree(&self) -> usize {
        if self.srs_g1.is_empty() {
            0
        } else {
            self.srs_g1.len() - 1
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

/// Create a KzgProver with SRS based on configuration (T082)
///
/// - If srs_path is provided and exists, loads SRS from file
/// - If dev_mode is true and no srs_path, initializes test SRS
/// - Otherwise, returns prover without SRS (proof generation will fail)
pub fn create_prover(srs_path: &str, dev_mode: bool) -> Result<KzgProver> {
    let mut prover = KzgProver::new();
    
    if !srs_path.is_empty() {
        let path = Path::new(srs_path);
        if path.exists() {
            prover.load_srs_from_file(path)?;
            info!("Loaded SRS from file: {:?}", path);
        } else {
            bail!("SRS file not found: {:?}", path);
        }
    } else if dev_mode {
        // Use test SRS for development (degree 1024 is sufficient for most test cases)
        prover.init_test_srs(1024)?;
        info!("Initialized test SRS (degree 1024) for development mode");
    } else {
        warn!("No SRS loaded - KZG proof generation will fail");
    }
    
    Ok(prover)
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

    #[test]
    fn test_init_test_srs() {
        let mut prover = KzgProver::new();
        assert!(!prover.is_srs_loaded());
        
        prover.init_test_srs(256).unwrap();
        
        assert!(prover.is_srs_loaded());
        assert_eq!(prover.srs_degree(), 256);
    }

    #[tokio::test]
    async fn test_proof_generation_with_test_srs() {
        let mut prover = KzgProver::new();
        prover.init_test_srs(256).unwrap();

        // Create a simple polynomial with 2 coefficients: f(x) = a0 + a1*x
        let mut poly_coeffs = vec![0u8; 64];
        // a0 = 1, a1 = 2
        poly_coeffs[0] = 1;
        poly_coeffs[32] = 2;

        let share = StoredShare {
            content_hash: [1u8; 32],
            share_index: 1,
            share_value: [3u8; 32], // f(1) = 1 + 2*1 = 3
            polynomial_coeffs: poly_coeffs,
        };

        prover.store_share(share).await;

        let result = prover.generate_proof_for_challenge(&[1u8; 32], 1).await;
        assert!(result.is_ok(), "Proof generation should succeed with test SRS");
        
        let (share_value, proof) = result.unwrap();
        assert_eq!(share_value[0], 3);
        assert!(proof.iter().any(|&b| b != 0), "Proof should not be all zeros");
    }

    #[test]
    fn test_create_prover_dev_mode() {
        let prover = create_prover("", true).unwrap();
        assert!(prover.is_srs_loaded());
        assert_eq!(prover.srs_degree(), 1024);
    }

    #[test]
    fn test_create_prover_no_srs() {
        let prover = create_prover("", false).unwrap();
        assert!(!prover.is_srs_loaded());
    }
}

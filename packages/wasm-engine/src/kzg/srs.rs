//! SRS (Structured Reference String) Loading
//!
//! Ethereum KZG Ceremony (EIP-4844) の Powers of Tau 成果物を使用。

use ark_bls12_381::{G1Affine, G2Affine};
use ark_serialize::CanonicalDeserialize;
use ark_std::vec::Vec;
use std::sync::OnceLock;

use super::KzgError;

/// SRS (Structured Reference String) for KZG commitments
pub struct Srs {
    /// Powers of tau in G1: [τ^0]₁, [τ^1]₁, ..., [τ^{n-1}]₁
    pub powers_of_g1: Vec<G1Affine>,
    /// [τ]₂ for pairing verification
    pub tau_g2: G2Affine,
}

/// Global SRS instance (thread-safe one-time initialization)
static GLOBAL_SRS: OnceLock<Srs> = OnceLock::new();

/// Initialize the SRS from raw bytes.
///
/// # Arguments
/// * `srs_bytes` - SRS file bytes (Ethereum KZG Ceremony format)
///
/// # Returns
/// - `Ok(())` if SRS was initialized successfully or was already initialized
/// - `Err` if loading fails
pub fn init_srs(srs_bytes: &[u8]) -> Result<(), KzgError> {
    // Early return if already initialized (idempotent call)
    // Note: This check is redundant but avoids parsing overhead when already loaded.
    // The actual initialization via get_or_init is atomic.
    if GLOBAL_SRS.get().is_some() {
        return Ok(());
    }
    
    // Parse SRS before attempting initialization
    let srs = load_srs_from_bytes(srs_bytes)?;
    
    // Atomic initialization - if another thread won, their value is kept
    GLOBAL_SRS.get_or_init(|| srs);
    Ok(())
}

/// Get reference to the global SRS.
pub fn get_srs() -> Result<&'static Srs, KzgError> {
    GLOBAL_SRS.get().ok_or(KzgError::SrsNotLoaded)
}

/// Check if SRS is initialized.
pub fn is_srs_initialized() -> bool {
    GLOBAL_SRS.get().is_some()
}

/// Initialize a test SRS for testing purposes.
///
/// # ⚠️ SECURITY WARNING ⚠️
///
/// This SRS is **NOT cryptographically secure**. It uses a publicly known tau value
/// (tau = 12345), which means the "trusted setup" is completely broken.
/// Anyone can forge proofs when this SRS is used.
///
/// **ONLY use this for unit tests, NEVER for production!**
///
/// # Compile-time Protection
///
/// This function is gated behind `#[cfg(any(test, feature = "test-utils"))]`,
/// which means it can ONLY be compiled when:
/// - Running `cargo test` (the `test` cfg is automatically set)
/// - Explicitly enabling the `test-utils` feature (which should never be done in production)
///
/// This provides a compile-time guarantee that production builds cannot accidentally
/// use this insecure test SRS.
#[cfg(any(test, feature = "test-utils"))]
pub fn init_test_srs(max_degree: usize) -> Result<(), KzgError> {
    use ark_bls12_381::G1Affine;
    use ark_ec::AffineRepr;
    use ark_ff::Field;
    use ark_bls12_381::Fr;

    // Early return if already initialized (idempotent)
    if GLOBAL_SRS.get().is_some() {
        return Ok(());
    }

    // ============================================================
    // ⚠️ INSECURE: Known tau value for testing only ⚠️
    // This value is publicly known and allows anyone to forge proofs.
    // Production MUST use the Ethereum KZG Ceremony trusted setup.
    // ============================================================
    #[cfg(not(any(test, debug_assertions)))]
    compile_error!("init_test_srs must not be used in release builds. Use init_srs_from_ceremony_text instead.");

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

    // Atomic initialization
    GLOBAL_SRS.get_or_init(|| srs);

    Ok(())
}

/// Initialize SRS from Ethereum KZG Ceremony text format.
///
/// This is the format used by the official Ethereum trusted setup:
/// https://github.com/ethereum/c-kzg-4844/blob/main/src/trusted_setup.txt
///
/// Format:
/// - Line 1: number of G1 points (e.g., "4096")
/// - Line 2: number of G2 points (e.g., "65")
/// - Lines 3 to (2 + num_g1): G1 points as hex strings (96 chars = 48 bytes compressed)
/// - Lines (3 + num_g1) to end: G2 points as hex strings (192 chars = 96 bytes compressed)
///
/// We use G2[1] (second G2 point) as tau_g2 for verification.
pub fn init_srs_from_ceremony_text(text: &str) -> Result<(), KzgError> {
    if GLOBAL_SRS.get().is_some() {
        return Ok(());
    }
    
    let srs = load_srs_from_ceremony_text(text)?;
    GLOBAL_SRS.get_or_init(|| srs);
    Ok(())
}

/// Load SRS from Ethereum KZG Ceremony text format.
///
/// This returns an Srs struct without initializing the global SRS.
/// Useful for testing and inspection.
pub fn load_srs_from_ceremony_text(text: &str) -> Result<Srs, KzgError> {
    let lines: Vec<&str> = text.lines().collect();
    
    if lines.len() < 4 {
        return Err(KzgError::EncodingError("SRS text too short".into()));
    }
    
    let num_g1: usize = lines[0].trim().parse()
        .map_err(|_| KzgError::EncodingError("Invalid G1 count".into()))?;
    let num_g2: usize = lines[1].trim().parse()
        .map_err(|_| KzgError::EncodingError("Invalid G2 count".into()))?;
    
    if num_g1 == 0 {
        return Err(KzgError::EncodingError("SRS must have at least 1 G1 point".into()));
    }
    if num_g2 < 2 {
        return Err(KzgError::EncodingError("SRS must have at least 2 G2 points (need G2[1])".into()));
    }
    
    let expected_lines = 2 + num_g1 + num_g2;
    if lines.len() < expected_lines {
        return Err(KzgError::EncodingError(format!(
            "Expected {} lines, got {}", expected_lines, lines.len()
        )));
    }
    
    // Parse G1 points (lines 2 to 2+num_g1-1, 0-indexed)
    let mut powers_of_g1 = Vec::with_capacity(num_g1);
    for i in 0..num_g1 {
        let hex_str = lines[2 + i].trim();
        let bytes = hex_to_bytes(hex_str)
            .map_err(|e| KzgError::EncodingError(format!("G1[{}] hex error: {}", i, e)))?;
        
        if bytes.len() != 48 {
            return Err(KzgError::EncodingError(format!(
                "G1[{}] wrong size: expected 48 bytes, got {}", i, bytes.len()
            )));
        }
        
        let point = G1Affine::deserialize_compressed(&bytes[..])
            .map_err(|e| KzgError::EncodingError(format!("G1[{}] deserialize error: {}", i, e)))?;
        powers_of_g1.push(point);
    }
    
    // Parse G2[1] (second G2 point) as tau_g2
    // G2 points start at line (2 + num_g1), G2[1] is at line (2 + num_g1 + 1)
    let tau_g2_line = 2 + num_g1 + 1;
    let tau_g2_hex = lines[tau_g2_line].trim();
    let tau_g2_bytes = hex_to_bytes(tau_g2_hex)
        .map_err(|e| KzgError::EncodingError(format!("tau_g2 hex error: {}", e)))?;
    
    if tau_g2_bytes.len() != 96 {
        return Err(KzgError::EncodingError(format!(
            "tau_g2 wrong size: expected 96 bytes, got {}", tau_g2_bytes.len()
        )));
    }
    
    let tau_g2 = G2Affine::deserialize_compressed(&tau_g2_bytes[..])
        .map_err(|e| KzgError::EncodingError(format!("tau_g2 deserialize error: {}", e)))?;
    
    Ok(Srs {
        powers_of_g1,
        tau_g2,
    })
}

/// Convert hex string to bytes.
fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("Hex string has odd length".into());
    }
    
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|e| format!("Invalid hex at position {}: {}", i, e))
        })
        .collect()
}

/// Load SRS from bytes.
///
/// Format (simplified binary format):
/// - 4 bytes: num_g1 (u32 LE)
/// - num_g1 × 48 bytes: G1 points (compressed BLS12-381 G1)
/// - 96 bytes: G2 point (compressed BLS12-381 G2)
fn load_srs_from_bytes(bytes: &[u8]) -> Result<Srs, KzgError> {
    const HEADER_SIZE: usize = 4;
    const G1_SIZE: usize = 48;
    const G2_SIZE: usize = 96;
    
    // Minimum: header + at least 1 G1 point + G2 point
    let min_size = HEADER_SIZE + G1_SIZE + G2_SIZE;
    if bytes.len() < min_size {
        return Err(KzgError::EncodingError(format!(
            "SRS file too small: minimum {} bytes, got {}",
            min_size,
            bytes.len()
        )));
    }
    
    let num_g1 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if num_g1 == 0 {
        return Err(KzgError::EncodingError("SRS must contain at least 1 G1 point".into()));
    }
    
    let expected_size = HEADER_SIZE + num_g1 * G1_SIZE + G2_SIZE;
    
    if bytes.len() < expected_size {
        return Err(KzgError::EncodingError(format!(
            "SRS file too small: expected {} bytes, got {}",
            expected_size,
            bytes.len()
        )));
    }

    let mut powers_of_g1 = Vec::with_capacity(num_g1);
    let mut offset = HEADER_SIZE;

    for _ in 0..num_g1 {
        let point = G1Affine::deserialize_compressed(&bytes[offset..offset + G1_SIZE])
            .map_err(|e| KzgError::EncodingError(format!("Failed to deserialize G1 point: {}", e)))?;
        powers_of_g1.push(point);
        offset += G1_SIZE;
    }

    let tau_g2 = G2Affine::deserialize_compressed(&bytes[offset..offset + G2_SIZE])
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
    fn test_srs_initialization() {
        // Initialize test SRS
        init_test_srs(10).expect("Failed to init test SRS");
        
        // Should be initialized now
        assert!(is_srs_initialized());
        
        // Should be able to get the SRS
        let srs = get_srs().expect("Failed to get SRS");
        assert!(!srs.powers_of_g1.is_empty());
        
        // Re-initialization should be idempotent (no error)
        assert!(init_test_srs(10).is_ok());
    }

    #[test]
    fn test_hex_to_bytes() {
        assert_eq!(hex_to_bytes("").unwrap(), vec![]);
        assert_eq!(hex_to_bytes("00").unwrap(), vec![0u8]);
        assert_eq!(hex_to_bytes("ff").unwrap(), vec![255u8]);
        assert_eq!(hex_to_bytes("0102").unwrap(), vec![1u8, 2u8]);
        assert_eq!(hex_to_bytes("deadbeef").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
        
        // Error cases
        assert!(hex_to_bytes("0").is_err()); // odd length
        assert!(hex_to_bytes("gg").is_err()); // invalid hex
    }

    #[test]
    fn test_ceremony_text_parsing_errors() {
        // Too short
        assert!(load_srs_from_ceremony_text("").is_err());
        assert!(load_srs_from_ceremony_text("4096\n65\n").is_err());
        
        // Invalid counts
        assert!(load_srs_from_ceremony_text("abc\n65\npoint1\npoint2").is_err());
        assert!(load_srs_from_ceremony_text("0\n65\npoint1\npoint2").is_err());
        assert!(load_srs_from_ceremony_text("1\n1\npoint1\npoint2").is_err()); // need at least 2 G2
    }
}

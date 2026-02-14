//! PoW (Proof of Work) verification for node registration
//!
//! Implements Blake2b-based PoW with dynamic difficulty to prevent
//! DoS attacks on register_node extrinsic (FR-409).
//!
//! ## Difficulty Calculation
//!
//! The difficulty is calculated based on the number of recent registrations:
//! ```text
//! difficulty = min(BASE_DIFFICULTY + recent_registrations / 5, MAX_DIFFICULTY)
//! ```
//!
//! Where:
//! - BASE_DIFFICULTY = 12 (leading zero bits, ~1 second computation)
//! - MAX_DIFFICULTY = 24 (leading zero bits, ~17 minutes computation)
//! - recent_registrations = sum of registrations in the last 10 blocks

use sp_io::hashing::blake2_256;
use sp_std::vec::Vec;

/// Maximum PoW difficulty (leading zero bits)
pub const MAX_DIFFICULTY: u8 = 24;

/// PoW verification result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowResult {
    /// PoW is valid
    Valid,
    /// PoW does not meet difficulty requirement
    InsufficientDifficulty,
}

/// Calculate the current PoW difficulty based on recent registrations.
///
/// # Arguments
/// * `recent_registrations` - Total registrations in the observation period
/// * `base_difficulty` - Base difficulty (leading zero bits)
///
/// # Returns
/// The required difficulty (number of leading zero bits)
pub fn calculate_difficulty(recent_registrations: u32, base_difficulty: u8) -> u8 {
    let additional = recent_registrations.saturating_div(5) as u8;
    base_difficulty.saturating_add(additional).min(MAX_DIFFICULTY)
}

/// Verify a PoW nonce meets the required difficulty.
///
/// The hash is computed as: Blake2b-256(peer_id || nonce.to_le_bytes())
///
/// # Arguments
/// * `peer_id` - The PeerID being registered
/// * `nonce` - The nonce value to verify
/// * `difficulty` - Required number of leading zero bits
///
/// # Returns
/// `PowResult::Valid` if the hash has at least `difficulty` leading zero bits
pub fn verify_pow(peer_id: &[u8], nonce: u64, difficulty: u8) -> PowResult {
    let hash = compute_pow_hash(peer_id, nonce);
    
    if count_leading_zero_bits(&hash) >= difficulty {
        PowResult::Valid
    } else {
        PowResult::InsufficientDifficulty
    }
}

/// Compute the PoW hash for a given peer_id and nonce.
///
/// Hash = Blake2b-256(peer_id || nonce.to_le_bytes())
pub fn compute_pow_hash(peer_id: &[u8], nonce: u64) -> [u8; 32] {
    let mut input = Vec::with_capacity(peer_id.len() + 8);
    input.extend_from_slice(peer_id);
    input.extend_from_slice(&nonce.to_le_bytes());
    blake2_256(&input)
}

/// Count the number of leading zero bits in a hash.
/// Returns u8 with max value 255 (saturates for 32 all-zero bytes).
pub fn count_leading_zero_bits(hash: &[u8; 32]) -> u8 {
    let mut count = 0u8;
    for byte in hash.iter() {
        if *byte == 0 {
            count = count.saturating_add(8);
        } else {
            count = count.saturating_add(byte.leading_zeros() as u8);
            break;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_difficulty_base() {
        assert_eq!(calculate_difficulty(0, 12), 12);
        assert_eq!(calculate_difficulty(4, 12), 12);
        assert_eq!(calculate_difficulty(5, 12), 13);
        assert_eq!(calculate_difficulty(10, 12), 14);
    }

    #[test]
    fn test_calculate_difficulty_max() {
        // Should cap at MAX_DIFFICULTY
        assert_eq!(calculate_difficulty(100, 12), 24);
        assert_eq!(calculate_difficulty(1000, 12), 24);
    }

    #[test]
    fn test_count_leading_zero_bits() {
        // All zeros saturates at 255 (32*8 = 256 would overflow u8)
        assert_eq!(count_leading_zero_bits(&[0u8; 32]), 255);
        assert_eq!(count_leading_zero_bits(&[0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), 23);
        assert_eq!(count_leading_zero_bits(&[0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), 0);
        assert_eq!(count_leading_zero_bits(&[0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), 7);
    }

    #[test]
    fn test_verify_pow_easy() {
        // With difficulty 0, any nonce should be valid
        let peer_id = b"12D3KooWtest";
        assert_eq!(verify_pow(peer_id, 0, 0), PowResult::Valid);
    }

    #[test]
    fn test_verify_pow_hash_deterministic() {
        let peer_id = b"12D3KooWtest";
        let nonce = 12345u64;
        let hash1 = compute_pow_hash(peer_id, nonce);
        let hash2 = compute_pow_hash(peer_id, nonce);
        assert_eq!(hash1, hash2);
    }
}

//! # Shared PoW Primitives
//!
//! Common Proof-of-Work functions used by pallet-faucet and pallet-reaction.
//!
//! ## Functions
//! - `compute_challenge`: Generate challenge from block hash and account
//! - `verify_proof`: Verify PoW nonce meets difficulty
//! - `count_leading_zero_bits`: Count leading zeros in hash

#![cfg_attr(not(feature = "std"), no_std)]

use sp_io::hashing::blake2_256;
use sp_std::vec::Vec;

#[cfg(test)]
mod tests;

/// Compute challenge from block hash and account ID bytes.
///
/// `challenge = blake2_256(block_hash || account_bytes)`
///
/// # Arguments
/// * `block_hash` - The block hash bytes (32 bytes)
/// * `account_bytes` - The SCALE-encoded account ID
///
/// # Returns
/// 32-byte challenge hash
pub fn compute_challenge(block_hash: &[u8], account_bytes: &[u8]) -> [u8; 32] {
    let mut data = Vec::with_capacity(block_hash.len() + account_bytes.len());
    data.extend_from_slice(block_hash);
    data.extend_from_slice(account_bytes);
    blake2_256(&data)
}

/// Verify PoW proof meets difficulty requirement.
///
/// `hash = blake2_256(challenge || nonce.to_le_bytes())`
/// `valid = leading_zeros(hash) >= difficulty`
///
/// # Arguments
/// * `challenge` - 32-byte challenge
/// * `nonce` - The nonce to verify
/// * `difficulty` - Required number of leading zero bits
///
/// # Returns
/// `true` if proof is valid
pub fn verify_proof(challenge: &[u8; 32], nonce: u64, difficulty: u8) -> bool {
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(challenge);
    data.extend_from_slice(&nonce.to_le_bytes());
    let hash = blake2_256(&data);
    count_leading_zero_bits(&hash) >= difficulty
}

/// Count leading zero bits in a 32-byte hash.
///
/// # Arguments
/// * `hash` - 32-byte hash
///
/// # Returns
/// Number of leading zero bits (0-256)
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

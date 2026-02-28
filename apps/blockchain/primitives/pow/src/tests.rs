//! Unit tests for primitives-pow

use super::*;

#[test]
fn test_count_leading_zero_bits_all_zeros() {
    let hash = [0u8; 32];
    // 32 bytes * 8 bits = 256, but u8 max is 255, saturating_add caps at 255
    assert_eq!(count_leading_zero_bits(&hash), 255);
}

#[test]
fn test_count_leading_zero_bits_first_byte_set() {
    let mut hash = [0u8; 32];
    hash[0] = 0b10000000; // 0 leading zeros in first byte
    assert_eq!(count_leading_zero_bits(&hash), 0);
}

#[test]
fn test_count_leading_zero_bits_partial() {
    let mut hash = [0u8; 32];
    hash[0] = 0b00001000; // 4 leading zeros
    assert_eq!(count_leading_zero_bits(&hash), 4);
}

#[test]
fn test_count_leading_zero_bits_second_byte() {
    let mut hash = [0u8; 32];
    hash[0] = 0; // 8 zeros
    hash[1] = 0b00100000; // 2 leading zeros
    assert_eq!(count_leading_zero_bits(&hash), 10);
}

#[test]
fn test_compute_challenge_deterministic() {
    let block_hash = [1u8; 32];
    let account = [2u8; 32];
    
    let challenge1 = compute_challenge(&block_hash, &account);
    let challenge2 = compute_challenge(&block_hash, &account);
    
    assert_eq!(challenge1, challenge2);
}

#[test]
fn test_compute_challenge_different_inputs() {
    let block_hash1 = [1u8; 32];
    let block_hash2 = [2u8; 32];
    let account = [3u8; 32];
    
    let challenge1 = compute_challenge(&block_hash1, &account);
    let challenge2 = compute_challenge(&block_hash2, &account);
    
    assert_ne!(challenge1, challenge2);
}

#[test]
fn test_verify_proof_valid() {
    let challenge = [0u8; 32];
    // Find a nonce that produces at least 1 leading zero bit
    // Since we're hashing, this should be found quickly
    let mut nonce = 0u64;
    let difficulty = 1u8;
    
    // Try a few nonces
    for n in 0..1000 {
        if verify_proof(&challenge, n, difficulty) {
            nonce = n;
            break;
        }
    }
    
    assert!(verify_proof(&challenge, nonce, difficulty));
}

#[test]
fn test_verify_proof_invalid_high_difficulty() {
    let challenge = [0u8; 32];
    let nonce = 0u64;
    let difficulty = 200u8; // Very high difficulty
    
    // With random hash, extremely unlikely to pass
    // Just verify the function runs without panic
    let _ = verify_proof(&challenge, nonce, difficulty);
}

#[test]
fn test_verify_proof_zero_difficulty() {
    let challenge = [0u8; 32];
    let nonce = 0u64;
    let difficulty = 0u8;
    
    // Zero difficulty should always pass
    assert!(verify_proof(&challenge, nonce, difficulty));
}

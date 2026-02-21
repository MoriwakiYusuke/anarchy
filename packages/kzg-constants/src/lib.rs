//! KZG Cryptographic Constants for Anarchy
//!
//! This crate provides the single source of truth for KZG-related constants
//! used across pallet-storage, storage-node, and wasm-engine.
//!
//! # TAU_G2_BYTES
//! This is [τ]₂ from the Ethereum KZG Ceremony (EIP-4844 trusted setup).
//! It's the second element of KZG_SETUP_G2 from the Powers of Tau ceremony.

#![cfg_attr(not(feature = "std"), no_std)]

/// [τ]₂ - The BLS12-381 G2 point from Ethereum KZG Ceremony
///
/// This is KZG_SETUP_G2[1] from the Ethereum KZG Ceremony (EIP-4844).
/// All components (pallet-storage, storage-node, wasm-engine) MUST use
/// this exact value to ensure KZG proof compatibility.
///
/// The value is in compressed G2 format (96 bytes).
///
/// # Validation
/// This constant is validated at compile time to be a valid BLS12-381 G2 point
/// when ark-bls12-381 is available (see tests).
pub const TAU_G2_BYTES: [u8; 96] = [
    // KZG_SETUP_G2[1] from Ethereum KZG Ceremony (EIP-4844 trusted setup)
    0xb5, 0xbf, 0xd7, 0xdd, 0x8c, 0xde, 0xb1, 0x28,
    0x84, 0x3b, 0xc2, 0x87, 0x23, 0x0a, 0xf3, 0x89,
    0x26, 0x18, 0x70, 0x75, 0xcb, 0xfb, 0xef, 0xa8,
    0x10, 0x09, 0xa2, 0xce, 0x61, 0x5a, 0xc5, 0x3d,
    0x29, 0x14, 0xe5, 0x87, 0x0c, 0xb4, 0x52, 0xd2,
    0xaf, 0xaa, 0xab, 0x24, 0xf3, 0x49, 0x9f, 0x72,
    0x18, 0x5c, 0xbf, 0xee, 0x53, 0x49, 0x27, 0x14,
    0x73, 0x44, 0x29, 0xb7, 0xb3, 0x86, 0x08, 0xe2,
    0x39, 0x26, 0xc9, 0x11, 0xcc, 0xec, 0xea, 0xc9,
    0xa3, 0x68, 0x51, 0x47, 0x7b, 0xa4, 0xc6, 0x0b,
    0x08, 0x70, 0x41, 0xde, 0x62, 0x10, 0x00, 0xed,
    0xc9, 0x8e, 0xda, 0xda, 0x20, 0xc1, 0xde, 0xf2,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tau_g2_bytes_is_96_bytes() {
        assert_eq!(TAU_G2_BYTES.len(), 96);
    }

    #[test]
    fn tau_g2_bytes_first_byte_is_valid_g2_flag() {
        // BLS12-381 G2 compressed format: first byte has flags
        // Bit 7 (0x80): compression flag (set for compressed)
        // Bit 6 (0x40): infinity flag (should be unset)
        // Bit 5 (0x20): sign bit
        let first_byte = TAU_G2_BYTES[0];
        // Must have compression flag set
        assert!(first_byte & 0x80 != 0, "G2 point should be in compressed format");
        // Must not be point at infinity
        assert!(first_byte & 0x40 == 0, "G2 point should not be at infinity");
    }

    #[test]
    fn tau_g2_bytes_not_all_zeros() {
        // Ensure non-trivial value (not all zeros)
        let sum: u32 = TAU_G2_BYTES.iter().map(|&b| b as u32).sum();
        assert!(sum > 0, "TAU_G2_BYTES should not be all zeros");
    }

    /// Validates that TAU_G2_BYTES deserializes to a valid BLS12-381 G2 point.
    /// This is the definitive test that the constant is cryptographically valid.
    #[test]
    fn tau_g2_bytes_deserializes_to_valid_g2_point() {
        use ark_bls12_381::G2Affine;
        use ark_ec::AffineRepr;
        use ark_serialize::CanonicalDeserialize;

        // Deserialize TAU_G2_BYTES as a compressed G2 point
        let result = G2Affine::deserialize_compressed(&TAU_G2_BYTES[..]);
        assert!(
            result.is_ok(),
            "TAU_G2_BYTES failed to deserialize as valid G2 point: {:?}",
            result.err()
        );

        let g2_point = result.unwrap();

        // Ensure it's not the identity (zero) point
        assert!(
            !g2_point.is_zero(),
            "TAU_G2_BYTES is the identity point, which is invalid for KZG"
        );

        // Verify it's on the curve (implicitly checked by successful deserialization)
        // but we can also check explicitly
        assert!(
            g2_point.is_on_curve(),
            "TAU_G2_BYTES is not on the BLS12-381 G2 curve"
        );

        // Verify it's in the correct subgroup
        assert!(
            g2_point.is_in_correct_subgroup_assuming_on_curve(),
            "TAU_G2_BYTES is not in the correct G2 subgroup"
        );
    }
}

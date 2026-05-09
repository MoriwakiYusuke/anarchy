//! Per-fragment metadata stored alongside the bytes.
//!
//! Phase 2 of TODO §4.9. Phase 1 stored only `(key → bytes)`; this layer adds
//! a parallel `(key → SCALE(Metadata))` table so we can:
//!
//! - verify-on-read using `data_hash` (catches bit-rot — Phase 1 had no check)
//! - drive LRU eviction off `last_accessed_at`
//! - report fragment age / ref count via metrics & RPCs
//!
//! ## Layout (versioned)
//!
//! Stored as SCALE-encoded bytes in the meta table. The first byte is a
//! `version` tag so future fields can be added without a flag-day migration.
//! Currently only `V1` exists — readers refuse anything else.

use parity_scale_codec::{Decode, Encode};

/// Metadata format version. Bumped when the wire layout changes.
pub const META_V1: u8 = 1;

/// Per-fragment metadata.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    /// Format version (always [`META_V1`] for now).
    pub version: u8,
    /// Stored byte length. Redundant with `value.len()` from the data table,
    /// but kept here so `delete` can decrement the used-bytes counter without
    /// loading the value.
    pub size: u64,
    /// Unix seconds at first store.
    pub created_at: u64,
    /// Unix seconds at last successful retrieve. Currently set equal to
    /// `created_at` and not updated on read — Phase 2 deferred lazy
    /// touch-on-read updates to a future slice (paired with LRU eviction
    /// in TODO §4.9).
    pub last_accessed_at: u64,
    /// How many logical references point at this blob. Currently always 1
    /// (we don't dedupe across posts), reserved for future content-addressed
    /// dedupe.
    pub ref_count: u32,
    /// Blake2-256 of the stored bytes. Used for `verify_on_read`.
    pub data_hash: [u8; 32],
}

impl Metadata {
    /// Build a fresh metadata record for a just-stored fragment.
    pub fn fresh(size: u64, data_hash: [u8; 32], now_unix: u64) -> Self {
        Self {
            version: META_V1,
            size,
            created_at: now_unix,
            last_accessed_at: now_unix,
            ref_count: 1,
            data_hash,
        }
    }

    /// Encode to SCALE bytes for storage.
    pub fn encode_to_vec(&self) -> Vec<u8> {
        self.encode()
    }

    /// Decode from SCALE bytes. Returns an error if the stored version is
    /// not recognized — we'd rather fail loudly than silently misinterpret a
    /// future-format record.
    pub fn decode_from_slice(bytes: &[u8]) -> anyhow::Result<Self> {
        let meta = <Metadata as Decode>::decode(&mut &bytes[..])
            .map_err(|e| anyhow::anyhow!("Failed to SCALE-decode metadata: {}", e))?;
        if meta.version != META_V1 {
            anyhow::bail!(
                "Unknown metadata version {} (expected {})",
                meta.version,
                META_V1
            );
        }
        Ok(meta)
    }
}

/// Current Unix timestamp in seconds. Used as the time source for metadata
/// records. Centralized here so tests can swap it later if needed.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_roundtrip() {
        let m = Metadata::fresh(1234, [42u8; 32], 1_700_000_000);
        let bytes = m.encode_to_vec();
        let decoded = Metadata::decode_from_slice(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn metadata_rejects_unknown_version() {
        let mut bytes = Metadata::fresh(0, [0u8; 32], 0).encode_to_vec();
        bytes[0] = 0xFF; // bogus version
        let err = Metadata::decode_from_slice(&bytes).unwrap_err();
        assert!(err.to_string().contains("Unknown metadata version"));
    }
}

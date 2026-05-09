//! Low-level redb wrapper.
//!
//! Holds the `Database` handle and the table definitions used by
//! [`super::FragmentStore`]. Phase 2 of TODO §4.9 — split out from
//! `mod.rs` so the domain layer doesn't repeat redb plumbing and so a
//! future engine swap (fjall, …) only touches this file.
//!
//! ## Tables
//!
//! - [`FRAGMENTS`] — `fragment_id (32B) → bytes`. Hash-based (legacy) API.
//! - [`POST_FRAGMENTS`] — `(post_id, index) → bytes`. Post-based API.
//! - [`FRAGMENT_META`] — `fragment_id → SCALE(Metadata)`.
//! - [`POST_FRAGMENT_META`] — `(post_id, index) → SCALE(Metadata)`.
//! - [`SYSTEM`] — small key/value scratchpad, currently used for
//!   `total_used_bytes` (so we can skip the O(N) startup scan).

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use redb::{Database, TableDefinition};

// ============================================================================
// Table definitions
// ============================================================================

pub(super) const FRAGMENTS: TableDefinition<&[u8; 32], &[u8]> =
    TableDefinition::new("fragments");
pub(super) const POST_FRAGMENTS: TableDefinition<(u64, u32), &[u8]> =
    TableDefinition::new("post_fragments");
pub(super) const FRAGMENT_META: TableDefinition<&[u8; 32], &[u8]> =
    TableDefinition::new("fragment_meta");
pub(super) const POST_FRAGMENT_META: TableDefinition<(u64, u32), &[u8]> =
    TableDefinition::new("post_fragment_meta");
pub(super) const SYSTEM: TableDefinition<&str, u64> = TableDefinition::new("system");

/// Key for the persisted total-used-bytes counter inside [`SYSTEM`].
pub(super) const KEY_TOTAL_USED_BYTES: &str = "total_used_bytes";

// ============================================================================
// Engine
// ============================================================================

/// Thin wrapper around an open redb database. Cheap to clone (`Arc` inside).
#[derive(Clone)]
pub struct Engine {
    pub(super) db: Arc<Database>,
}

impl Engine {
    /// Open (or create) the database at `path`. The parent directory must
    /// already exist.
    pub fn open(path: &Path) -> Result<Self> {
        let db = Database::create(path).with_context(|| {
            format!("Failed to open redb database at {}", path.display())
        })?;
        Ok(Self { db: Arc::new(db) })
    }
}

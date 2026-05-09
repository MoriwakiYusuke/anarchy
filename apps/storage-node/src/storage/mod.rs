//! Fragment storage management
//!
//! Backed by an embedded redb (B-tree, ACID) database at
//! `{data_dir}/fragments.redb`. Phase 1 of TODO §4.9 swapped out the
//! per-fragment filesystem layout (`fragments/{xx}/{yy}/{hex}.bin`,
//! `fragments/{post_id}/{index}.bin`) for redb tables. Phase 2 (this file
//! split + metadata table + persistent counter + verify-on-read) builds
//! on top.
//!
//! ## Layering
//!
//! - [`engine`] — low-level redb plumbing (Database handle + table defs).
//! - [`metadata`] — SCALE-encoded per-fragment metadata.
//! - [`FragmentStore`] (this file) — public domain layer the rest of the
//!   crate sees. Composes Engine + Metadata to do the actual work.
//!
//! ## Tables (defined in `engine.rs`)
//!
//! - `fragments` — `[u8;32] → bytes` (legacy hash-based API).
//! - `post_fragments` — `(u64, u32) → bytes` (post-based API).
//! - `fragment_meta` / `post_fragment_meta` — `key → SCALE(Metadata)`.
//! - `system` — scratchpad, currently `"total_used_bytes" → u64`.
//!
//! ## Atomicity
//!
//! Each `store` writes data, metadata, and the running used-bytes counter
//! under one [`redb::WriteTransaction`]. A crash between operations either
//! commits the whole entry or none of it.
//!
//! ## Used-bytes counter
//!
//! Persisted in the `system` table on every change (Phase 2). Loaded once
//! at open time. The legacy O(N) fallback scan still exists for the case
//! where the counter is absent (first start or recovery from a corrupted
//! `system` table).
//!
//! ## verify_on_read
//!
//! When [`FragmentStore::new_with_verify`] is called with `verify=true`,
//! every `retrieve` recomputes Blake2-256 of the bytes and checks it
//! against `Metadata::data_hash`. Mismatch returns an error (caught by
//! the RPC handler, logged, and surfaced as a 500). Off by default so
//! per-call hashing only kicks in for operators who want bit-rot
//! detection.

pub mod engine;
pub mod metadata;

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use blake2::digest::consts::U32;
use blake2::{Blake2b, Digest};
use redb::{ReadableDatabase, ReadableTable, ReadableTableMetadata, TableError};
use tracing::{debug, info, warn};

use crate::storage::engine::{
    Engine, FRAGMENTS, FRAGMENT_META, KEY_TOTAL_USED_BYTES, POST_FRAGMENTS, POST_FRAGMENT_META,
    SYSTEM,
};
use crate::storage::metadata::{now_unix, Metadata};

// ============================================================================
// Security Constants (T074)
// ============================================================================

/// Maximum fragment size: 1GB (1073741824 bytes)
/// Fragments exceeding this size will be rejected.
/// Large files (images/videos up to 256MB) are split into SSS shards before storage.
pub const MAX_FRAGMENT_SIZE: usize = 1024 * 1024 * 1024;

/// Maximum post ID value (for sanity check)
pub const MAX_POST_ID: u64 = u64::MAX / 2;

/// Maximum fragment index
pub const MAX_FRAGMENT_INDEX: u32 = 255;

/// Fragment ID type (32 bytes, Blake2-256 hash)
pub type FragmentId = [u8; 32];

// ============================================================================
// FragmentStore
// ============================================================================

/// Fragment store backed by redb.
pub struct FragmentStore {
    engine: Engine,
    capacity: u64,
    /// In-memory mirror of `system."total_used_bytes"`. Kept in sync inside
    /// the same write txn that mutates fragments — see `commit_used_delta`.
    used: Arc<AtomicU64>,
    /// When `true`, every `retrieve` recomputes Blake2-256 and compares
    /// against `Metadata::data_hash`. Cheap when off, ~CPU-bound when on.
    verify_on_read: bool,
}

impl FragmentStore {
    /// Open the store with verification disabled (the default).
    pub fn new(data_dir: &str, capacity: u64) -> Result<Self> {
        Self::new_with_verify(data_dir, capacity, false)
    }

    /// Open the store, optionally enabling Blake2 verification on every read.
    pub fn new_with_verify(data_dir: &str, capacity: u64, verify_on_read: bool) -> Result<Self> {
        let dir = Path::new(data_dir);
        std::fs::create_dir_all(dir).context("Failed to create data directory")?;
        let db_path = dir.join("fragments.redb");

        let engine = Engine::open(&db_path)?;
        let used = Self::load_or_recover_used(&engine)?;

        info!(
            used_bytes = used,
            capacity_bytes = capacity,
            verify_on_read,
            db_path = %db_path.display(),
            "Fragment store opened (redb)"
        );

        Ok(Self {
            engine,
            capacity,
            used: Arc::new(AtomicU64::new(used)),
            verify_on_read,
        })
    }

    // ========================================================================
    // Hash-based API (legacy)
    // ========================================================================

    /// Store a fragment under its 32-byte location-based ID.
    ///
    /// Note: fragment_id is computed from `hash(merkle_root || index)` by the
    /// caller, not from the data content. The `Metadata.data_hash` field
    /// (set here) stores the actual content hash for `verify_on_read`.
    pub fn store(&self, fragment_id: FragmentId, data: &[u8]) -> Result<()> {
        if data.len() > MAX_FRAGMENT_SIZE {
            bail!(
                "Fragment size {} exceeds maximum allowed {} bytes",
                data.len(),
                MAX_FRAGMENT_SIZE
            );
        }

        let new_size = data.len() as u64;

        // (#31-H-1) Reserve capacity atomically before writing.
        let prev = self.used.fetch_add(new_size, Ordering::Relaxed);
        if prev + new_size > self.capacity {
            self.used.fetch_sub(new_size, Ordering::Relaxed);
            bail!(
                "Storage quota exceeded: used {} + {} > capacity {}",
                prev,
                new_size,
                self.capacity
            );
        }

        let data_hash = Self::compute_hash(data);
        let meta = Metadata::fresh(new_size, data_hash, now_unix()).encode_to_vec();

        let already_existed = match self.insert_fragment(&fragment_id, data, &meta) {
            Ok(existed) => existed,
            Err(e) => {
                self.used.fetch_sub(new_size, Ordering::Relaxed);
                return Err(e);
            }
        };

        if already_existed {
            // Idempotent — refund the reservation we made above.
            self.used.fetch_sub(new_size, Ordering::Relaxed);
            debug!(fragment_id = %hex::encode(fragment_id), "Fragment already exists, skipping");
            return Ok(());
        }

        info!(
            fragment_id = %hex::encode(fragment_id),
            size = data.len(),
            "Fragment stored"
        );
        Ok(())
    }

    /// Retrieve a fragment by ID. When `verify_on_read` is enabled, also
    /// recompute Blake2-256 and check it against the stored `data_hash`.
    pub fn retrieve(&self, fragment_id: &FragmentId) -> Result<Option<Vec<u8>>> {
        let txn = self.engine.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::Error::from(e).context("Failed to open fragments table")),
        };
        let bytes = match table.get(fragment_id).context("Failed to read fragment")? {
            Some(v) => v.value().to_vec(),
            None => return Ok(None),
        };

        if self.verify_on_read {
            self.verify_fragment_integrity(&txn, fragment_id, &bytes)?;
        }

        Ok(Some(bytes))
    }

    /// Check if a fragment exists.
    pub fn exists(&self, fragment_id: &FragmentId) -> bool {
        match self.fragment_exists_inner(fragment_id) {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "exists() check failed");
                false
            }
        }
    }

    /// Delete a fragment by ID.
    pub fn delete(&self, fragment_id: &FragmentId) -> Result<bool> {
        let txn = self.engine.db.begin_write().context("Failed to begin write txn")?;
        let removed_size = {
            let mut data_table = match txn.open_table(FRAGMENTS) {
                Ok(t) => t,
                Err(TableError::TableDoesNotExist(_)) => return Ok(false),
                Err(e) => return Err(anyhow::Error::from(e).context("open fragments")),
            };
            // Split `?` from the match: keeping the `?` inline holds the
            // `ControlFlow` temporary alive across the early-return path,
            // which extends the AccessGuard's borrow past `data_table`'s
            // drop.
            let removed = data_table.remove(fragment_id).context("remove fragment")?;
            let size = match removed {
                Some(old) => old.value().len() as u64,
                None => return Ok(false),
            };
            // Best-effort metadata cleanup. Missing metadata is not fatal —
            // it can happen for fragments stored in Phase 1 before the
            // meta table existed.
            let _ = match txn.open_table(FRAGMENT_META) {
                Ok(mut t) => t.remove(fragment_id).map(|_| ()),
                Err(TableError::TableDoesNotExist(_)) => Ok(()),
                Err(e) => return Err(anyhow::Error::from(e).context("open fragment_meta")),
            };
            size
        };
        Self::write_used_delta(&txn, -(removed_size as i64))?;
        txn.commit().context("commit delete txn")?;
        self.used.fetch_sub(removed_size, Ordering::Relaxed);
        info!(
            fragment_id = %hex::encode(fragment_id),
            freed_bytes = removed_size,
            "Fragment deleted"
        );
        Ok(true)
    }

    /// Delete every hash-based fragment.
    pub fn delete_all(&self) -> Result<usize> {
        let txn = self.engine.db.begin_write().context("Failed to begin write txn")?;
        let (deleted, freed) = {
            let mut data_table = match txn.open_table(FRAGMENTS) {
                Ok(t) => t,
                Err(TableError::TableDoesNotExist(_)) => return Ok(0),
                Err(e) => return Err(anyhow::Error::from(e).context("open fragments")),
            };
            let entries: Vec<([u8; 32], u64)> = data_table
                .iter()
                .context("iterate fragments")?
                .map(|r| {
                    r.map(|(k, v)| (*k.value(), v.value().len() as u64))
                        .map_err(anyhow::Error::from)
                })
                .collect::<Result<_>>()?;

            let mut deleted = 0usize;
            let mut freed = 0u64;
            // Open meta table once; ignore TableDoesNotExist.
            let mut meta_table_opt = match txn.open_table(FRAGMENT_META) {
                Ok(t) => Some(t),
                Err(TableError::TableDoesNotExist(_)) => None,
                Err(e) => return Err(anyhow::Error::from(e).context("open fragment_meta")),
            };
            for (key, size) in entries {
                if data_table.remove(&key)?.is_some() {
                    deleted += 1;
                    freed += size;
                    if let Some(meta) = meta_table_opt.as_mut() {
                        let _ = meta.remove(&key);
                    }
                }
            }
            (deleted, freed)
        };
        Self::write_used_delta(&txn, -(freed as i64))?;
        txn.commit().context("commit delete_all txn")?;
        self.used.fetch_sub(freed, Ordering::Relaxed);
        Ok(deleted)
    }

    /// Get current used capacity.
    pub fn used_bytes(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }

    /// Get total capacity.
    pub fn capacity_bytes(&self) -> u64 {
        self.capacity
    }

    /// Get number of stored hash-based fragments.
    ///
    /// Hash-based only — kept this way because `main.rs`'s GC loop expects
    /// the legacy semantics (it pairs with `list_fragments` and `delete`
    /// which are also hash-only). Use [`Self::total_fragment_count`] for
    /// metrics or anything that should include post-based fragments.
    pub fn fragment_count(&self) -> Result<usize> {
        let txn = self.engine.db.begin_read().context("Failed to begin read txn")?;
        match txn.open_table(FRAGMENTS) {
            Ok(t) => Ok(t.len().context("count fragments")? as usize),
            Err(TableError::TableDoesNotExist(_)) => Ok(0),
            Err(e) => Err(anyhow::Error::from(e).context("open fragments")),
        }
    }

    /// Get number of stored post-based fragments.
    pub fn post_fragment_count(&self) -> Result<usize> {
        let txn = self.engine.db.begin_read().context("Failed to begin read txn")?;
        match txn.open_table(POST_FRAGMENTS) {
            Ok(t) => Ok(t.len().context("count post fragments")? as usize),
            Err(TableError::TableDoesNotExist(_)) => Ok(0),
            Err(e) => Err(anyhow::Error::from(e).context("open post_fragments")),
        }
    }

    /// Total number of stored fragments across both tables. Use this for
    /// observability (e.g. `storage_fragments_total` Prometheus gauge).
    pub fn total_fragment_count(&self) -> Result<usize> {
        Ok(self.fragment_count()? + self.post_fragment_count()?)
    }

    /// List all hash-based fragment IDs.
    pub fn list_fragments(&self) -> Result<Vec<FragmentId>> {
        let txn = self.engine.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(anyhow::Error::from(e).context("open fragments")),
        };
        let mut ids = Vec::with_capacity(table.len().unwrap_or(0) as usize);
        for entry in table.iter().context("iterate fragments")? {
            let (k, _) = entry?;
            ids.push(*k.value());
        }
        Ok(ids)
    }

    /// Read the metadata record for a hash-based fragment, if present.
    pub fn fragment_metadata(&self, fragment_id: &FragmentId) -> Result<Option<Metadata>> {
        let txn = self.engine.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(FRAGMENT_META) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::Error::from(e).context("open fragment_meta")),
        };
        let bytes = match table.get(fragment_id).context("read meta")? {
            Some(v) => v.value().to_vec(),
            None => return Ok(None),
        };
        Ok(Some(Metadata::decode_from_slice(&bytes)?))
    }

    // ========================================================================
    // Post-based storage API (T059)
    // ========================================================================

    /// Store a fragment by `post_id` and `index` (T059).
    pub fn store_post_fragment(&self, post_id: u64, index: u32, data: &[u8]) -> Result<()> {
        if data.len() > MAX_FRAGMENT_SIZE {
            bail!(
                "Fragment size {} exceeds maximum allowed {} bytes",
                data.len(),
                MAX_FRAGMENT_SIZE
            );
        }
        if post_id > MAX_POST_ID {
            bail!("Invalid post_id {}: exceeds maximum {}", post_id, MAX_POST_ID);
        }
        if index > MAX_FRAGMENT_INDEX {
            bail!("Invalid index {}: exceeds maximum {}", index, MAX_FRAGMENT_INDEX);
        }

        let new_size = data.len() as u64;
        let prev = self.used.fetch_add(new_size, Ordering::Relaxed);
        if prev + new_size > self.capacity {
            self.used.fetch_sub(new_size, Ordering::Relaxed);
            bail!(
                "Storage quota exceeded: used {} + {} > capacity {}",
                prev,
                new_size,
                self.capacity
            );
        }

        let data_hash = Self::compute_hash(data);
        let meta = Metadata::fresh(new_size, data_hash, now_unix()).encode_to_vec();

        let already_existed = match self.insert_post_fragment(post_id, index, data, &meta) {
            Ok(existed) => existed,
            Err(e) => {
                self.used.fetch_sub(new_size, Ordering::Relaxed);
                return Err(e);
            }
        };

        if already_existed {
            self.used.fetch_sub(new_size, Ordering::Relaxed);
            debug!(post_id, index, "Post fragment already exists, skipping");
            return Ok(());
        }

        info!(post_id, index, size = data.len(), "Post fragment stored");
        Ok(())
    }

    /// Retrieve a fragment by `post_id` and `index`.
    pub fn retrieve_post_fragment(&self, post_id: u64, index: u32) -> Result<Option<Vec<u8>>> {
        let txn = self.engine.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(POST_FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::Error::from(e).context("open post_fragments")),
        };
        let bytes = match table.get(&(post_id, index)).context("read post fragment")? {
            Some(v) => v.value().to_vec(),
            None => return Ok(None),
        };

        if self.verify_on_read {
            self.verify_post_fragment_integrity(&txn, post_id, index, &bytes)?;
        }

        Ok(Some(bytes))
    }

    /// Check if a post fragment exists.
    pub fn post_fragment_exists(&self, post_id: u64, index: u32) -> bool {
        match self.post_fragment_exists_inner(post_id, index) {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "post_fragment_exists() check failed");
                false
            }
        }
    }

    /// List fragment indices stored for a given post.
    pub fn list_post_fragments(&self, post_id: u64) -> Result<Vec<u32>> {
        let txn = self.engine.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(POST_FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(anyhow::Error::from(e).context("open post_fragments")),
        };
        let mut indices = Vec::new();
        for entry in table
            .range((post_id, 0u32)..=(post_id, u32::MAX))
            .context("range scan post_fragments")?
        {
            let (k, _) = entry?;
            indices.push(k.value().1);
        }
        Ok(indices)
    }

    /// Delete every fragment belonging to a post.
    pub fn delete_post_fragments(&self, post_id: u64) -> Result<()> {
        let txn = self.engine.db.begin_write().context("Failed to begin write txn")?;
        let freed = {
            let mut data_table = match txn.open_table(POST_FRAGMENTS) {
                Ok(t) => t,
                Err(TableError::TableDoesNotExist(_)) => return Ok(()),
                Err(e) => return Err(anyhow::Error::from(e).context("open post_fragments")),
            };
            let entries: Vec<((u64, u32), u64)> = data_table
                .range((post_id, 0u32)..=(post_id, u32::MAX))
                .context("range scan post_fragments")?
                .map(|r| {
                    r.map(|(k, v)| (k.value(), v.value().len() as u64))
                        .map_err(anyhow::Error::from)
                })
                .collect::<Result<_>>()?;

            let mut freed = 0u64;
            let mut meta_table_opt = match txn.open_table(POST_FRAGMENT_META) {
                Ok(t) => Some(t),
                Err(TableError::TableDoesNotExist(_)) => None,
                Err(e) => return Err(anyhow::Error::from(e).context("open post_fragment_meta")),
            };
            for (key, size) in entries {
                if data_table.remove(&key)?.is_some() {
                    freed += size;
                    if let Some(meta) = meta_table_opt.as_mut() {
                        let _ = meta.remove(&key);
                    }
                }
            }
            freed
        };
        Self::write_used_delta(&txn, -(freed as i64))?;
        txn.commit().context("commit delete_post_fragments txn")?;
        self.used.fetch_sub(freed, Ordering::Relaxed);
        info!(post_id, freed_bytes = freed, "Post fragments deleted");
        Ok(())
    }

    /// Read the metadata record for a post fragment, if present.
    pub fn post_fragment_metadata(&self, post_id: u64, index: u32) -> Result<Option<Metadata>> {
        let txn = self.engine.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(POST_FRAGMENT_META) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::Error::from(e).context("open post_fragment_meta")),
        };
        let bytes = match table.get(&(post_id, index)).context("read meta")? {
            Some(v) => v.value().to_vec(),
            None => return Ok(None),
        };
        Ok(Some(Metadata::decode_from_slice(&bytes)?))
    }

    // ========================================================================
    // Hash helper (public for tests / external callers)
    // ========================================================================

    /// Compute Blake2-256 hash of arbitrary bytes.
    pub fn compute_hash(data: &[u8]) -> FragmentId {
        let mut hasher = Blake2b::<U32>::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Insert into `fragments` + `fragment_meta`. Returns `true` if the
    /// fragment already existed (caller treats this as idempotent success).
    fn insert_fragment(
        &self,
        fragment_id: &FragmentId,
        data: &[u8],
        meta_bytes: &[u8],
    ) -> Result<bool> {
        let txn = self.engine.db.begin_write().context("begin write")?;
        let already_existed = {
            let mut data_table = txn.open_table(FRAGMENTS).context("open fragments")?;
            if data_table
                .get(fragment_id)
                .context("probe fragments")?
                .is_some()
            {
                true
            } else {
                data_table
                    .insert(fragment_id, data)
                    .context("insert fragment")?;
                let mut meta_table =
                    txn.open_table(FRAGMENT_META).context("open fragment_meta")?;
                meta_table
                    .insert(fragment_id, meta_bytes)
                    .context("insert metadata")?;
                false
            }
        };
        if !already_existed {
            Self::write_used_delta(&txn, data.len() as i64)?;
            txn.commit().context("commit fragment insert")?;
        }
        Ok(already_existed)
    }

    fn insert_post_fragment(
        &self,
        post_id: u64,
        index: u32,
        data: &[u8],
        meta_bytes: &[u8],
    ) -> Result<bool> {
        let txn = self.engine.db.begin_write().context("begin write")?;
        let already_existed = {
            let mut data_table = txn
                .open_table(POST_FRAGMENTS)
                .context("open post_fragments")?;
            if data_table
                .get(&(post_id, index))
                .context("probe post_fragments")?
                .is_some()
            {
                true
            } else {
                data_table
                    .insert(&(post_id, index), data)
                    .context("insert post fragment")?;
                let mut meta_table = txn
                    .open_table(POST_FRAGMENT_META)
                    .context("open post_fragment_meta")?;
                meta_table
                    .insert(&(post_id, index), meta_bytes)
                    .context("insert post metadata")?;
                false
            }
        };
        if !already_existed {
            Self::write_used_delta(&txn, data.len() as i64)?;
            txn.commit().context("commit post fragment insert")?;
        }
        Ok(already_existed)
    }

    fn fragment_exists_inner(&self, fragment_id: &FragmentId) -> Result<bool> {
        let txn = self.engine.db.begin_read().context("begin read")?;
        let table = match txn.open_table(FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(false),
            Err(e) => return Err(anyhow::Error::from(e)),
        };
        Ok(table.get(fragment_id)?.is_some())
    }

    fn post_fragment_exists_inner(&self, post_id: u64, index: u32) -> Result<bool> {
        let txn = self.engine.db.begin_read().context("begin read")?;
        let table = match txn.open_table(POST_FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(false),
            Err(e) => return Err(anyhow::Error::from(e)),
        };
        Ok(table.get(&(post_id, index))?.is_some())
    }

    /// Verify a hash-based fragment's bytes against its stored `data_hash`.
    /// Used only when `verify_on_read` is enabled.
    fn verify_fragment_integrity(
        &self,
        txn: &redb::ReadTransaction,
        fragment_id: &FragmentId,
        bytes: &[u8],
    ) -> Result<()> {
        let meta_table = match txn.open_table(FRAGMENT_META) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => {
                bail!(
                    "verify_on_read requested but fragment_meta table is missing \
                     (data may pre-date Phase 2 — wipe & rebuild)"
                );
            }
            Err(e) => return Err(anyhow::Error::from(e)),
        };
        let bytes_meta = meta_table
            .get(fragment_id)
            .context("read metadata for verify")?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "verify_on_read failed: missing metadata for fragment {}",
                    hex::encode(fragment_id)
                )
            })?;
        let meta = Metadata::decode_from_slice(bytes_meta.value())?;
        let actual = Self::compute_hash(bytes);
        if actual != meta.data_hash {
            bail!(
                "verify_on_read: hash mismatch for fragment {} (stored={}, actual={})",
                hex::encode(fragment_id),
                hex::encode(meta.data_hash),
                hex::encode(actual)
            );
        }
        Ok(())
    }

    fn verify_post_fragment_integrity(
        &self,
        txn: &redb::ReadTransaction,
        post_id: u64,
        index: u32,
        bytes: &[u8],
    ) -> Result<()> {
        let meta_table = match txn.open_table(POST_FRAGMENT_META) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => {
                bail!(
                    "verify_on_read requested but post_fragment_meta table is missing \
                     (data may pre-date Phase 2 — wipe & rebuild)"
                );
            }
            Err(e) => return Err(anyhow::Error::from(e)),
        };
        let bytes_meta = meta_table
            .get(&(post_id, index))
            .context("read metadata for verify")?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "verify_on_read failed: missing metadata for post {}/{}",
                    post_id,
                    index
                )
            })?;
        let meta = Metadata::decode_from_slice(bytes_meta.value())?;
        let actual = Self::compute_hash(bytes);
        if actual != meta.data_hash {
            bail!(
                "verify_on_read: hash mismatch for post {}/{} (stored={}, actual={})",
                post_id,
                index,
                hex::encode(meta.data_hash),
                hex::encode(actual)
            );
        }
        Ok(())
    }

    /// Persist a `±delta` change to the SYSTEM[total_used_bytes] counter
    /// inside an existing write txn. Caller commits.
    fn write_used_delta(txn: &redb::WriteTransaction, delta: i64) -> Result<()> {
        let mut sys = txn.open_table(SYSTEM).context("open system")?;
        let prev = sys
            .get(KEY_TOTAL_USED_BYTES)
            .context("read used counter")?
            .map(|v| v.value())
            .unwrap_or(0);
        let next = if delta >= 0 {
            prev.saturating_add(delta as u64)
        } else {
            prev.saturating_sub((-delta) as u64)
        };
        sys.insert(KEY_TOTAL_USED_BYTES, &next)
            .context("write used counter")?;
        Ok(())
    }

    /// On open: prefer the persistent counter, fall back to a full scan only
    /// if the counter is missing (first run, or recovery from a wiped SYSTEM
    /// table). Recovery writes the result back so subsequent restarts are
    /// fast again.
    fn load_or_recover_used(engine: &Engine) -> Result<u64> {
        // Fast path: read the persisted counter.
        {
            let txn = engine.db.begin_read().context("begin read")?;
            match txn.open_table(SYSTEM) {
                Ok(t) => {
                    if let Some(v) = t.get(KEY_TOTAL_USED_BYTES).context("read used counter")? {
                        return Ok(v.value());
                    }
                }
                Err(TableError::TableDoesNotExist(_)) => { /* fall through */ }
                Err(e) => return Err(anyhow::Error::from(e).context("open system")),
            }
        }

        // Slow path: scan both data tables. Phase 1 used this on every start;
        // Phase 2 runs it only when the counter is unavailable.
        warn!("system.total_used_bytes counter missing — falling back to O(N) scan");
        let total = Self::scan_used_bytes(engine)?;

        // Persist for next time.
        let txn = engine.db.begin_write().context("begin write for counter recovery")?;
        {
            let mut sys = txn.open_table(SYSTEM).context("open system")?;
            sys.insert(KEY_TOTAL_USED_BYTES, &total)
                .context("write recovered counter")?;
        }
        txn.commit().context("commit counter recovery")?;
        info!(used_bytes = total, "Recovered total_used_bytes via fallback scan");
        Ok(total)
    }

    /// Sum the byte lengths of every value in both data tables. Used only
    /// by [`Self::load_or_recover_used`].
    fn scan_used_bytes(engine: &Engine) -> Result<u64> {
        let txn = engine.db.begin_read().context("begin read")?;
        let mut total: u64 = 0;

        match txn.open_table(FRAGMENTS) {
            Ok(table) => {
                for entry in table.iter()? {
                    let (_, v) = entry?;
                    total += v.value().len() as u64;
                }
            }
            Err(TableError::TableDoesNotExist(_)) => {}
            Err(e) => return Err(anyhow::Error::from(e)),
        }

        match txn.open_table(POST_FRAGMENTS) {
            Ok(table) => {
                for entry in table.iter()? {
                    let (_, v) = entry?;
                    total += v.value().len() as u64;
                }
            }
            Err(TableError::TableDoesNotExist(_)) => {}
            Err(e) => return Err(anyhow::Error::from(e)),
        }

        Ok(total)
    }
}

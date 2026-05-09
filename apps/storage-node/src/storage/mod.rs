//! Fragment storage management
//!
//! Backed by an embedded redb (B-tree, ACID) database at
//! `{data_dir}/fragments.redb`. The previous filesystem layout
//! (`fragments/{xx}/{yy}/{hex}.bin` and `fragments/{post_id}/{index}.bin`)
//! is gone — see TODO §4.9 for the rationale (inode inflation, walkdir
//! O(N) startup, missing atomicity).
//!
//! ## Logical tables
//!
//! 1. **`fragments`** — keyed by 32-byte FragmentId, value is the fragment bytes.
//!    Used for the legacy hash-based API (`store` / `retrieve` / …).
//! 2. **`post_fragments`** — keyed by `(post_id, index)`, value is the fragment bytes.
//!    Used by the post-based API (`store_post_fragment` / …). Per-post listing
//!    uses `range((post_id, 0)..=(post_id, u32::MAX))` for a tight prefix scan.
//!
//! Both tables are written under one [`redb::WriteTransaction`] per call, so a
//! crash mid-write either commits the whole entry or none of it. The previous
//! `File::create → write_all` two-phase write left partially written
//! `.bin` files on power loss.
//!
//! Total usage is tracked with an in-memory [`AtomicU64`] and recovered on
//! startup by iterating both tables. For 1M fragments this is dominated by
//! sequential disk reads inside redb (much faster than `walkdir` + per-file
//! `stat(2)`).

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use blake2::digest::consts::U32;
use blake2::{Blake2b, Digest};
use redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition, TableError,
};
use tracing::{debug, info, warn};

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
// redb table definitions
// ============================================================================

const FRAGMENTS: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("fragments");
const POST_FRAGMENTS: TableDefinition<(u64, u32), &[u8]> = TableDefinition::new("post_fragments");

// ============================================================================
// FragmentStore
// ============================================================================

/// Fragment store backed by redb.
pub struct FragmentStore {
    db: Arc<Database>,
    capacity: u64,
    /// Cached running total of bytes stored across both tables. Persisted
    /// implicitly via the underlying values; recovered on `new()` by scanning.
    used: Arc<AtomicU64>,
}

impl FragmentStore {
    /// Open (or create) the fragment store at `{data_dir}/fragments.redb`.
    pub fn new(data_dir: &str, capacity: u64) -> Result<Self> {
        let dir = Path::new(data_dir);
        std::fs::create_dir_all(dir).context("Failed to create data directory")?;
        let db_path = dir.join("fragments.redb");

        let db = Database::create(&db_path).with_context(|| {
            format!("Failed to open redb database at {}", db_path.display())
        })?;

        let used = Self::scan_used_bytes(&db)?;
        info!(
            used_bytes = used,
            capacity_bytes = capacity,
            db_path = %db_path.display(),
            "Fragment store opened (redb)"
        );

        Ok(Self {
            db: Arc::new(db),
            capacity,
            used: Arc::new(AtomicU64::new(used)),
        })
    }

    /// Store a fragment under its 32-byte location-based ID.
    ///
    /// Note: fragment_id is computed from `hash(merkle_root || index)` by the
    /// caller, not from the data content. Data integrity is verified via
    /// Merkle proof on the blockchain node before reaching the storage node.
    pub fn store(&self, fragment_id: FragmentId, data: &[u8]) -> Result<()> {
        if data.len() > MAX_FRAGMENT_SIZE {
            bail!(
                "Fragment size {} exceeds maximum allowed {} bytes",
                data.len(),
                MAX_FRAGMENT_SIZE
            );
        }

        let new_size = data.len() as u64;

        // (#31-H-1) Reserve capacity atomically before writing. The previous
        // load-then-add allowed two concurrent callers to both pass the
        // capacity check and then both write, exceeding the quota.
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

        let already_existed = match self.insert_fragment(&fragment_id, data) {
            Ok(existed) => existed,
            Err(e) => {
                self.used.fetch_sub(new_size, Ordering::Relaxed);
                return Err(e);
            }
        };

        if already_existed {
            // Idempotent path — refund the reservation we made above.
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

    /// Retrieve a fragment by ID.
    pub fn retrieve(&self, fragment_id: &FragmentId) -> Result<Option<Vec<u8>>> {
        let txn = self.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::Error::from(e).context("Failed to open fragments table")),
        };
        let result = table
            .get(fragment_id)
            .context("Failed to read fragment")?
            .map(|v| v.value().to_vec());

        // Note: fragment_id is location-based (hash(merkle_root || index)),
        // not content-based, so we don't verify hash on read. (Optional
        // Blake2 verify-on-read is tracked in TODO §4.9 Phase 2.)
        Ok(result)
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
    ///
    /// Returns Ok(true) if fragment was deleted, Ok(false) if it didn't exist.
    pub fn delete(&self, fragment_id: &FragmentId) -> Result<bool> {
        let txn = self.db.begin_write().context("Failed to begin write txn")?;
        let removed_size = {
            let mut table = match txn.open_table(FRAGMENTS) {
                Ok(t) => t,
                Err(TableError::TableDoesNotExist(_)) => return Ok(false),
                Err(e) => {
                    return Err(anyhow::Error::from(e).context("Failed to open fragments table"))
                }
            };
            // Split `?` from the match: keeping the `?` inline holds the
            // `ControlFlow` temporary alive across the early-return path,
            // which extends the AccessGuard's borrow past `table`'s drop.
            let removed = table.remove(fragment_id).context("Failed to remove fragment")?;
            match removed {
                Some(old) => old.value().len() as u64,
                None => return Ok(false),
            }
        };
        txn.commit().context("Failed to commit delete txn")?;
        self.used.fetch_sub(removed_size, Ordering::Relaxed);
        info!(
            fragment_id = %hex::encode(fragment_id),
            freed_bytes = removed_size,
            "Fragment deleted"
        );
        Ok(true)
    }

    /// Delete every hash-based fragment (pool-based GC when reward pool is depleted).
    /// Post-based fragments are untouched, matching the previous behavior where
    /// `walk_fragments` only matched 64-hex stems.
    pub fn delete_all(&self) -> Result<usize> {
        let txn = self.db.begin_write().context("Failed to begin write txn")?;
        let (deleted, freed) = {
            let mut table = match txn.open_table(FRAGMENTS) {
                Ok(t) => t,
                Err(TableError::TableDoesNotExist(_)) => return Ok(0),
                Err(e) => {
                    return Err(anyhow::Error::from(e).context("Failed to open fragments table"))
                }
            };
            // Collect first, then remove — redb forbids mutating a table while
            // iterating it.
            let entries: Vec<([u8; 32], u64)> = table
                .iter()
                .context("Failed to iterate fragments")?
                .map(|r| {
                    r.map(|(k, v)| (*k.value(), v.value().len() as u64))
                        .map_err(anyhow::Error::from)
                })
                .collect::<Result<_>>()?;

            let mut deleted = 0usize;
            let mut freed = 0u64;
            for (key, size) in entries {
                if table.remove(&key)?.is_some() {
                    deleted += 1;
                    freed += size;
                }
            }
            (deleted, freed)
        };
        txn.commit().context("Failed to commit delete_all txn")?;
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
    pub fn fragment_count(&self) -> Result<usize> {
        let txn = self.db.begin_read().context("Failed to begin read txn")?;
        match txn.open_table(FRAGMENTS) {
            Ok(t) => Ok(t.len().context("Failed to count fragments")? as usize),
            Err(TableError::TableDoesNotExist(_)) => Ok(0),
            Err(e) => Err(anyhow::Error::from(e).context("Failed to open fragments table")),
        }
    }

    /// List all hash-based fragment IDs.
    pub fn list_fragments(&self) -> Result<Vec<FragmentId>> {
        let txn = self.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => {
                return Err(anyhow::Error::from(e).context("Failed to open fragments table"))
            }
        };
        let mut ids = Vec::with_capacity(table.len().unwrap_or(0) as usize);
        for entry in table.iter().context("Failed to iterate fragments")? {
            let (k, _) = entry?;
            ids.push(*k.value());
        }
        Ok(ids)
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
        // Atomic reservation, mirroring the hash-based path. The previous
        // load-then-add pattern allowed two concurrent writers to race past
        // the quota check.
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

        let already_existed = match self.insert_post_fragment(post_id, index, data) {
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
        let txn = self.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(POST_FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => {
                return Err(anyhow::Error::from(e).context("Failed to open post_fragments table"))
            }
        };
        let result = table
            .get(&(post_id, index))
            .context("Failed to read post fragment")?
            .map(|v| v.value().to_vec());
        Ok(result)
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
        let txn = self.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(POST_FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => {
                return Err(anyhow::Error::from(e).context("Failed to open post_fragments table"))
            }
        };
        let mut indices = Vec::new();
        let lo = (post_id, 0u32);
        let hi = (post_id, u32::MAX);
        for entry in table.range(lo..=hi).context("Failed to range-scan post fragments")? {
            let (k, _) = entry?;
            indices.push(k.value().1);
        }
        Ok(indices)
    }

    /// Delete every fragment belonging to a post.
    pub fn delete_post_fragments(&self, post_id: u64) -> Result<()> {
        let txn = self.db.begin_write().context("Failed to begin write txn")?;
        let freed = {
            let mut table = match txn.open_table(POST_FRAGMENTS) {
                Ok(t) => t,
                Err(TableError::TableDoesNotExist(_)) => return Ok(()),
                Err(e) => {
                    return Err(
                        anyhow::Error::from(e).context("Failed to open post_fragments table")
                    )
                }
            };
            let lo = (post_id, 0u32);
            let hi = (post_id, u32::MAX);
            let entries: Vec<((u64, u32), u64)> = table
                .range(lo..=hi)
                .context("Failed to range-scan post fragments")?
                .map(|r| {
                    r.map(|(k, v)| (k.value(), v.value().len() as u64))
                        .map_err(anyhow::Error::from)
                })
                .collect::<Result<_>>()?;

            let mut freed = 0u64;
            for (key, size) in entries {
                if table.remove(&key)?.is_some() {
                    freed += size;
                }
            }
            freed
        };
        txn.commit().context("Failed to commit delete_post_fragments txn")?;
        self.used.fetch_sub(freed, Ordering::Relaxed);
        info!(post_id, freed_bytes = freed, "Post fragments deleted");
        Ok(())
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

    /// Insert into `fragments` table. Returns `true` if the fragment already
    /// existed (caller treats this as idempotent success).
    fn insert_fragment(&self, fragment_id: &FragmentId, data: &[u8]) -> Result<bool> {
        let txn = self.db.begin_write().context("Failed to begin write txn")?;
        let already_existed = {
            let mut table = txn
                .open_table(FRAGMENTS)
                .context("Failed to open fragments table")?;
            if table
                .get(fragment_id)
                .context("Failed to probe fragments table")?
                .is_some()
            {
                true
            } else {
                table
                    .insert(fragment_id, data)
                    .context("Failed to insert fragment")?;
                false
            }
        };
        if !already_existed {
            txn.commit().context("Failed to commit fragment insert")?;
        }
        Ok(already_existed)
    }

    /// Insert into `post_fragments` table. Returns `true` if `(post_id, index)`
    /// already had a value.
    fn insert_post_fragment(&self, post_id: u64, index: u32, data: &[u8]) -> Result<bool> {
        let txn = self.db.begin_write().context("Failed to begin write txn")?;
        let already_existed = {
            let mut table = txn
                .open_table(POST_FRAGMENTS)
                .context("Failed to open post_fragments table")?;
            if table
                .get(&(post_id, index))
                .context("Failed to probe post_fragments table")?
                .is_some()
            {
                true
            } else {
                table
                    .insert(&(post_id, index), data)
                    .context("Failed to insert post fragment")?;
                false
            }
        };
        if !already_existed {
            txn.commit()
                .context("Failed to commit post fragment insert")?;
        }
        Ok(already_existed)
    }

    fn fragment_exists_inner(&self, fragment_id: &FragmentId) -> Result<bool> {
        let txn = self.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(false),
            Err(e) => return Err(anyhow::Error::from(e)),
        };
        Ok(table.get(fragment_id)?.is_some())
    }

    fn post_fragment_exists_inner(&self, post_id: u64, index: u32) -> Result<bool> {
        let txn = self.db.begin_read().context("Failed to begin read txn")?;
        let table = match txn.open_table(POST_FRAGMENTS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(false),
            Err(e) => return Err(anyhow::Error::from(e)),
        };
        Ok(table.get(&(post_id, index))?.is_some())
    }

    /// Recover the running `used` counter on startup by scanning both tables.
    /// O(N) over fragment count but sequential redb iteration — orders of
    /// magnitude faster than `walkdir` over the previous `.bin` layout.
    fn scan_used_bytes(db: &Database) -> Result<u64> {
        let txn = db.begin_read().context("Failed to begin read txn")?;
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

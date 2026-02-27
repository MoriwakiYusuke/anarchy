//! Fragment storage management
//!
//! Handles local disk storage for fragments with hash verification.
//!
//! ## Storage Modes
//!
//! 1. **Hash-based** (legacy): `fragments/{hex[0..2]}/{hex[2..4]}/{hex}.bin`
//! 2. **Post-based** (new): `fragments/{post_id}/{index}.bin`

#[cfg(test)]
mod tests;

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use anyhow::{Context, Result, bail};
use blake2::{Blake2b, Digest};
use blake2::digest::consts::U32;
use tracing::{info, debug, warn};

// ============================================================================
// Security Constants (T074)
// ============================================================================

/// Maximum fragment size: 256KB (262144 bytes)
/// Fragments exceeding this size will be rejected.
/// Kept small for DoS protection and memory efficiency.
pub const MAX_FRAGMENT_SIZE: usize = 256 * 1024;

/// Maximum post ID value (for sanity check)
pub const MAX_POST_ID: u64 = u64::MAX / 2;

/// Maximum fragment index
pub const MAX_FRAGMENT_INDEX: u32 = 255;

/// Fragment ID type (32 bytes, Blake2-256 hash)
pub type FragmentId = [u8; 32];

/// Fragment store for local disk storage
pub struct FragmentStore {
    /// Base directory for fragments
    base_dir: PathBuf,
    /// Maximum capacity in bytes
    capacity: u64,
    /// Current used capacity (atomic for thread safety)
    used: Arc<AtomicU64>,
}

impl FragmentStore {
    /// Create a new fragment store
    pub fn new(data_dir: &str, capacity: u64) -> Result<Self> {
        let base_dir = Path::new(data_dir).join("fragments");
        fs::create_dir_all(&base_dir)
            .context("Failed to create fragments directory")?;

        // Calculate current usage
        let used = Self::calculate_usage(&base_dir)?;
        info!(used_bytes = used, capacity_bytes = capacity, "Fragment store opened");

        Ok(Self {
            base_dir,
            capacity,
            used: Arc::new(AtomicU64::new(used)),
        })
    }

    /// Store a fragment
    /// 
    /// Note: fragment_id is computed from hash(merkle_root || index) by the caller,
    /// not from the data content. Data integrity is verified via Merkle proof
    /// on the blockchain node before reaching the storage node.
    pub fn store(&self, fragment_id: FragmentId, data: &[u8]) -> Result<()> {
        // ============================================================
        // Security Validation (T074)
        // ============================================================

        // Fragment size check (max 256KB)
        if data.len() > MAX_FRAGMENT_SIZE {
            bail!(
                "Fragment size {} exceeds maximum allowed {} bytes",
                data.len(),
                MAX_FRAGMENT_SIZE
            );
        }

        // Check capacity
        let new_size = data.len() as u64;
        let current_used = self.used.load(Ordering::Relaxed);
        if current_used + new_size > self.capacity {
            bail!("Storage quota exceeded: used {} + {} > capacity {}", 
                current_used, new_size, self.capacity);
        }

        // Get path and create parent dirs
        let path = self.fragment_path(&fragment_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create fragment directory")?;
        }

        // Check if already exists (idempotent)
        if path.exists() {
            debug!(fragment_id = %hex::encode(fragment_id), "Fragment already exists, skipping");
            return Ok(());
        }

        // Write file
        let mut file = File::create(&path)
            .context("Failed to create fragment file")?;
        file.write_all(data)
            .context("Failed to write fragment data")?;

        // Update usage counter
        self.used.fetch_add(new_size, Ordering::Relaxed);

        info!(
            fragment_id = %hex::encode(fragment_id),
            size = data.len(),
            "Fragment stored"
        );

        Ok(())
    }

    /// Retrieve a fragment by ID
    pub fn retrieve(&self, fragment_id: &FragmentId) -> Result<Option<Vec<u8>>> {
        let path = self.fragment_path(fragment_id);
        
        if !path.exists() {
            return Ok(None);
        }

        let mut file = File::open(&path)
            .context("Failed to open fragment file")?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .context("Failed to read fragment data")?;

        // Note: fragment_id is location-based (hash(merkle_root || index)),
        // not content-based, so we don't verify hash on read.

        Ok(Some(data))
    }

    /// Check if a fragment exists
    pub fn exists(&self, fragment_id: &FragmentId) -> bool {
        self.fragment_path(fragment_id).exists()
    }

    /// Delete a fragment by ID (T085: GC integration)
    /// 
    /// Returns Ok(true) if fragment was deleted, Ok(false) if it didn't exist
    pub fn delete(&self, fragment_id: &FragmentId) -> Result<bool> {
        let path = self.fragment_path(fragment_id);
        
        if !path.exists() {
            return Ok(false);
        }

        // Get file size before deletion for usage tracking
        let size = std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or(0);

        // Delete file
        std::fs::remove_file(&path)
            .context("Failed to delete fragment file")?;

        // Update usage counter
        self.used.fetch_sub(size, Ordering::Relaxed);

        info!(
            fragment_id = %hex::encode(fragment_id),
            freed_bytes = size,
            "Fragment deleted"
        );

        Ok(true)
    }

    /// Delete all fragments (for pool-based GC when reward pool is depleted)
    ///
    /// Returns the number of fragments deleted
    pub fn delete_all(&self) -> Result<usize> {
        let mut ids = Vec::new();
        
        // Collect all fragment IDs first
        Self::walk_fragments(&self.base_dir, &mut |id| {
            ids.push(id);
            Ok(())
        })?;
        
        let mut deleted = 0;
        for id in &ids {
            match self.delete(id) {
                Ok(true) => deleted += 1,
                Ok(false) => {} // Already deleted
                Err(e) => warn!(
                    fragment_id = %hex::encode(id),
                    error = %e,
                    "Failed to delete fragment during delete_all"
                ),
            }
        }
        
        Ok(deleted)
    }

    /// Get current used capacity
    pub fn used_bytes(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }

    /// Get total capacity
    pub fn capacity_bytes(&self) -> u64 {
        self.capacity
    }

    /// Get number of stored fragments
    pub fn fragment_count(&self) -> Result<usize> {
        let mut count = 0;
        Self::walk_fragments(&self.base_dir, &mut |_| {
            count += 1;
            Ok(())
        })?;
        Ok(count)
    }

    /// List all fragment IDs
    pub fn list_fragments(&self) -> Result<Vec<FragmentId>> {
        let mut fragments = Vec::new();
        Self::walk_fragments(&self.base_dir, &mut |id| {
            fragments.push(id);
            Ok(())
        })?;
        Ok(fragments)
    }

    // === Post-based storage API (T059: fragments/{post_id}/{index}.bin) ===

    /// Store a fragment by post_id and index
    /// 
    /// Path: fragments/{post_id}/{index}.bin
    pub fn store_post_fragment(&self, post_id: u64, index: u32, data: &[u8]) -> Result<()> {
        // ============================================================
        // Security Validation (T074)
        // ============================================================

        // Fragment size check (max 256KB)
        if data.len() > MAX_FRAGMENT_SIZE {
            bail!(
                "Fragment size {} exceeds maximum allowed {} bytes",
                data.len(),
                MAX_FRAGMENT_SIZE
            );
        }

        // Post ID sanity check
        if post_id > MAX_POST_ID {
            bail!("Invalid post_id {}: exceeds maximum {}", post_id, MAX_POST_ID);
        }

        // Fragment index check
        if index > MAX_FRAGMENT_INDEX {
            bail!("Invalid index {}: exceeds maximum {}", index, MAX_FRAGMENT_INDEX);
        }

        // Check capacity
        let new_size = data.len() as u64;
        let current_used = self.used.load(Ordering::Relaxed);
        if current_used + new_size > self.capacity {
            bail!("Storage quota exceeded: used {} + {} > capacity {}", 
                current_used, new_size, self.capacity);
        }

        // Get path and create parent dirs
        let path = self.post_fragment_path(post_id, index);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create fragment directory")?;
        }

        // Check if already exists (idempotent)
        if path.exists() {
            debug!(post_id = post_id, index = index, "Post fragment already exists, skipping");
            return Ok(());
        }

        // Write file
        let mut file = File::create(&path)
            .context("Failed to create fragment file")?;
        file.write_all(data)
            .context("Failed to write fragment data")?;

        // Update usage counter
        self.used.fetch_add(new_size, Ordering::Relaxed);

        info!(
            post_id = post_id,
            index = index,
            size = data.len(),
            "Post fragment stored"
        );

        Ok(())
    }

    /// Retrieve a fragment by post_id and index
    pub fn retrieve_post_fragment(&self, post_id: u64, index: u32) -> Result<Option<Vec<u8>>> {
        let path = self.post_fragment_path(post_id, index);
        
        if !path.exists() {
            return Ok(None);
        }

        let mut file = File::open(&path)
            .context("Failed to open fragment file")?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .context("Failed to read fragment data")?;

        Ok(Some(data))
    }

    /// Check if a post fragment exists
    pub fn post_fragment_exists(&self, post_id: u64, index: u32) -> bool {
        self.post_fragment_path(post_id, index).exists()
    }

    /// List fragment indices for a post
    pub fn list_post_fragments(&self, post_id: u64) -> Result<Vec<u32>> {
        let post_dir = self.base_dir.join(post_id.to_string());
        
        if !post_dir.exists() {
            return Ok(Vec::new());
        }

        let mut indices = Vec::new();
        for entry in fs::read_dir(&post_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "bin") {
                if let Some(stem) = path.file_stem() {
                    if let Some(stem_str) = stem.to_str() {
                        if let Ok(index) = stem_str.parse::<u32>() {
                            indices.push(index);
                        }
                    }
                }
            }
        }

        Ok(indices)
    }

    /// Delete all fragments for a post
    pub fn delete_post_fragments(&self, post_id: u64) -> Result<()> {
        let post_dir = self.base_dir.join(post_id.to_string());
        
        if post_dir.exists() {
            // Calculate size to free
            let mut freed_size: u64 = 0;
            for entry in fs::read_dir(&post_dir)? {
                let entry = entry?;
                if let Ok(meta) = entry.metadata() {
                    freed_size += meta.len();
                }
            }

            // Remove directory
            fs::remove_dir_all(&post_dir)
                .context("Failed to remove post fragments directory")?;

            // Update usage counter
            self.used.fetch_sub(freed_size, Ordering::Relaxed);

            info!(post_id = post_id, freed_bytes = freed_size, "Post fragments deleted");
        }

        Ok(())
    }

    /// Get the file path for a post fragment: fragments/{post_id}/{index}.bin
    fn post_fragment_path(&self, post_id: u64, index: u32) -> PathBuf {
        self.base_dir
            .join(post_id.to_string())
            .join(format!("{}.bin", index))
    }

    // === Internal helpers ===

    /// Get the file path for a fragment (hierarchical: aa/bb/aabb...def.bin)
    fn fragment_path(&self, fragment_id: &FragmentId) -> PathBuf {
        let hex = hex::encode(fragment_id);
        self.base_dir
            .join(&hex[0..2])
            .join(&hex[2..4])
            .join(format!("{}.bin", hex))
    }

    /// Compute Blake2-256 hash (public for testing)
    pub fn compute_hash(data: &[u8]) -> FragmentId {
        let mut hasher = Blake2b::<U32>::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }

    /// Compute Blake2-256 hash (internal alias)
    #[allow(dead_code)] // Utility function for future use
    fn hash(data: &[u8]) -> FragmentId {
        Self::compute_hash(data)
    }

    /// Calculate current disk usage
    fn calculate_usage(base_dir: &Path) -> Result<u64> {
        let mut total = 0u64;
        Self::walk_fragments(base_dir, &mut |_| {
            // Note: Could also accumulate file sizes here
            Ok(())
        })?;

        // Alternative: walk all .bin files and sum sizes
        if base_dir.exists() {
            for entry in walkdir::WalkDir::new(base_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "bin"))
            {
                if let Ok(meta) = entry.metadata() {
                    total += meta.len();
                }
            }
        }

        Ok(total)
    }

    /// Walk all fragment files
    fn walk_fragments<F>(base_dir: &Path, f: &mut F) -> Result<()>
    where
        F: FnMut(FragmentId) -> Result<()>,
    {
        if !base_dir.exists() {
            return Ok(());
        }

        for entry in walkdir::WalkDir::new(base_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "bin"))
        {
            if let Some(stem) = entry.path().file_stem() {
                if let Some(hex_str) = stem.to_str() {
                    if hex_str.len() == 64 {
                        if let Ok(bytes) = hex::decode(hex_str) {
                            if bytes.len() == 32 {
                                let mut id = [0u8; 32];
                                id.copy_from_slice(&bytes);
                                f(id)?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

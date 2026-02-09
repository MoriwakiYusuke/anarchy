//! Fragment storage management
//!
//! Handles local disk storage for fragments with hash verification.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use anyhow::{Context, Result, bail};
use blake2::{Blake2b, Digest};
use blake2::digest::consts::U32;
use tracing::{info, warn, debug};

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

    /// Store a fragment with hash verification
    pub fn store(&self, fragment_id: FragmentId, data: &[u8]) -> Result<()> {
        // Verify hash matches
        let computed_hash = Self::hash(data);
        if computed_hash != fragment_id {
            bail!("Fragment hash mismatch: expected {:?}, got {:?}", 
                hex::encode(fragment_id), hex::encode(computed_hash));
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

        // Verify hash on read
        let computed_hash = Self::hash(&data);
        if computed_hash != *fragment_id {
            warn!(
                fragment_id = %hex::encode(fragment_id),
                "Fragment hash mismatch on read, data may be corrupted"
            );
            // Still return data, but warn
        }

        Ok(Some(data))
    }

    /// Check if a fragment exists
    pub fn exists(&self, fragment_id: &FragmentId) -> bool {
        self.fragment_path(fragment_id).exists()
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

    // === Internal helpers ===

    /// Get the file path for a fragment (hierarchical: aa/bb/aabb...def.bin)
    fn fragment_path(&self, fragment_id: &FragmentId) -> PathBuf {
        let hex = hex::encode(fragment_id);
        self.base_dir
            .join(&hex[0..2])
            .join(&hex[2..4])
            .join(format!("{}.bin", hex))
    }

    /// Compute Blake2-256 hash
    fn hash(data: &[u8]) -> FragmentId {
        let mut hasher = Blake2b::<U32>::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
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
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "bin"))
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
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "bin"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_fragment() -> (FragmentId, Vec<u8>) {
        let data = b"Hello, Anarchy storage!".to_vec();
        let id = FragmentStore::hash(&data);
        (id, data)
    }

    #[test]
    fn test_store_and_retrieve() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let (id, data) = create_test_fragment();

        // Store
        store.store(id, &data).unwrap();
        assert!(store.exists(&id));

        // Retrieve
        let retrieved = store.retrieve(&id).unwrap().unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn test_hash_verification() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let data = b"test data".to_vec();
        let wrong_id = [0u8; 32]; // Wrong hash

        // Should fail with hash mismatch
        let result = store.store(wrong_id, &data);
        assert!(result.is_err());
    }

    #[test]
    fn test_quota_enforcement() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 10).unwrap(); // 10 bytes only

        let data = vec![0u8; 100]; // 100 bytes
        let id = FragmentStore::hash(&data);

        // Should fail with quota exceeded
        let result = store.store(id, &data);
        assert!(result.is_err());
    }

    #[test]
    fn test_idempotent_store() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let (id, data) = create_test_fragment();

        // Store twice - should succeed both times
        store.store(id, &data).unwrap();
        store.store(id, &data).unwrap();

        // Should still exist
        assert!(store.exists(&id));
    }

    #[test]
    fn test_retrieve_nonexistent() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let id = [99u8; 32];
        let result = store.retrieve(&id).unwrap();
        assert!(result.is_none());
    }
}

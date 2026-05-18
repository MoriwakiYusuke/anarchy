//! Unit tests for fragment storage
//!
//! Tests both:
//! 1. Hash-based storage (legacy API)
//! 2. Post-based storage (new API for Post Storage Migration - T054)
//! 3. Phase 2 additions: metadata, persistent counter, verify_on_read

use super::*;
use tempfile::TempDir;

// === Existing hash-based tests (moved from mod.rs) ===

fn create_test_fragment() -> (FragmentId, Vec<u8>) {
    let data = b"Hello, Anarchy storage!".to_vec();
    let id = FragmentStore::compute_hash(&data);
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
fn test_location_based_id() {
    // Storage now uses location-based IDs (hash(merkle_root || index))
    // instead of content-based IDs (hash(data)).
    // This test verifies that any ID can be used to store and retrieve data.
    let temp = TempDir::new().unwrap();
    let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

    let data = b"test data".to_vec();
    let arbitrary_id = [42u8; 32]; // Not hash of data

    // Should succeed - no hash verification
    let result = store.store(arbitrary_id, &data);
    assert!(result.is_ok());
    
    // Should retrieve the same data
    let retrieved = store.retrieve(&arbitrary_id).unwrap().unwrap();
    assert_eq!(retrieved, data);
}

#[test]
fn test_quota_enforcement() {
    let temp = TempDir::new().unwrap();
    let store = FragmentStore::new(temp.path().to_str().unwrap(), 10).unwrap(); // 10 bytes only

    let data = vec![0u8; 100]; // 100 bytes
    let id = FragmentStore::compute_hash(&data);

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

// === Phase 2 (TODO §4.9): metadata, persistent counter, verify_on_read ===

mod phase2 {
    use super::*;
    use crate::storage::engine::{KEY_TOTAL_USED_BYTES, SYSTEM};

    /// `evict_lru` is a no-op when usage is already at/below target.
    #[test]
    fn test_evict_lru_noop_below_target() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();
        store.store([1u8; 32], &vec![0u8; 100]).unwrap();
        let stats = store.evict_lru(1_000_000).unwrap();
        assert_eq!(stats.bytes_freed, 0);
        assert_eq!(stats.fragments_evicted, 0);
        assert!(store.exists(&[1u8; 32]));
    }

    /// `evict_lru` removes oldest-touched entries first across both tables
    /// until the target is met.
    #[test]
    fn test_evict_lru_removes_oldest_first() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        // Three fragments, 100 bytes each → 300 used.
        let id_a = [1u8; 32];
        let id_b = [2u8; 32];
        store.store(id_a, &vec![0xAA; 100]).unwrap();
        store.store_post_fragment(7, 0, &vec![0xBB; 100]).unwrap();
        store.store(id_b, &vec![0xCC; 100]).unwrap();
        assert_eq!(store.used_bytes(), 300);

        // Touch id_a so it's the most-recently-accessed of the three.
        // (record_touch_* is internal — calling retrieve achieves the same.)
        std::thread::sleep(std::time::Duration::from_secs(1)); // ensure ts changes
        let _ = store.retrieve(&id_a).unwrap();
        store.flush_touch_buffer().unwrap();

        // Evict to 150 bytes. Should remove the two oldest, keeping id_a.
        let stats = store.evict_lru(150).unwrap();
        assert!(stats.bytes_freed >= 150, "freed at least 150, got {}", stats.bytes_freed);
        assert!(store.exists(&id_a), "most-recently-touched survives");
        assert!(store.used_bytes() <= 150);
    }

    /// `flush_touch_buffer` updates `last_accessed_at` in metadata.
    #[test]
    fn test_touch_buffer_persists_last_accessed_at() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        store.store_post_fragment(99, 0, b"touchable").unwrap();
        let initial = store.post_fragment_metadata(99, 0).unwrap().unwrap();

        // Sleep a second so the touch ts differs from created_at.
        std::thread::sleep(std::time::Duration::from_secs(1));
        let _ = store.retrieve_post_fragment(99, 0).unwrap();
        let n = store.flush_touch_buffer().unwrap();
        assert!(n >= 1, "expected at least 1 touch flushed, got {n}");

        let after = store.post_fragment_metadata(99, 0).unwrap().unwrap();
        assert!(
            after.last_accessed_at > initial.last_accessed_at,
            "last_accessed_at should advance after retrieve+flush ({} → {})",
            initial.last_accessed_at,
            after.last_accessed_at
        );
        assert_eq!(after.created_at, initial.created_at, "created_at unchanged");
    }

    /// `total_fragment_count` sums both tables. Regression test for the
    /// Phase 2 metrics fix — `fragment_count()` alone (hash-only) under-
    /// reports on a node that has accepted post-based fragments via the
    /// repair / network paths.
    #[test]
    fn test_total_fragment_count_sums_both_tables() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        // 1 hash-based + 2 post-based
        store.store([1u8; 32], b"hash-based").unwrap();
        store.store_post_fragment(42, 0, b"post 0").unwrap();
        store.store_post_fragment(42, 1, b"post 1").unwrap();

        assert_eq!(store.fragment_count().unwrap(), 1);
        assert_eq!(store.post_fragment_count().unwrap(), 2);
        assert_eq!(store.total_fragment_count().unwrap(), 3);
    }

    /// `store_post_fragment` writes a metadata row alongside the bytes,
    /// with size + Blake2 hash matching the data and `ref_count = 1`.
    #[test]
    fn test_store_writes_metadata() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let post_id: u64 = 17;
        let data = b"metadata test payload".to_vec();
        store.store_post_fragment(post_id, 0, &data).unwrap();

        let meta = store
            .post_fragment_metadata(post_id, 0)
            .unwrap()
            .expect("metadata should exist after store");
        assert_eq!(meta.size, data.len() as u64);
        assert_eq!(meta.data_hash, FragmentStore::compute_hash(&data));
        assert_eq!(meta.ref_count, 1);
        assert_eq!(meta.version, crate::storage::metadata::META_V1);
        assert!(meta.created_at > 0);
        assert_eq!(meta.last_accessed_at, meta.created_at); // not updated yet
    }

    /// `verify_on_read = true` accepts unmodified data.
    #[test]
    fn test_verify_on_read_passes_for_clean_data() {
        let temp = TempDir::new().unwrap();
        let store =
            FragmentStore::new_with_verify(temp.path().to_str().unwrap(), 1024 * 1024, true)
                .unwrap();

        let data: Vec<u8> = (0..256).map(|i| (i % 251) as u8).collect();
        let id = [9u8; 32];
        store.store(id, &data).unwrap();

        let got = store.retrieve(&id).unwrap().unwrap();
        assert_eq!(got, data);
    }

    /// `verify_on_read = true` rejects data whose stored bytes have been
    /// corrupted out-of-band (simulating bit-rot on disk).
    #[test]
    fn test_verify_on_read_catches_bit_rot() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().to_str().unwrap();

        let id = [7u8; 32];
        let data = b"original payload".to_vec();
        {
            let store = FragmentStore::new_with_verify(dir, 1024 * 1024, true).unwrap();
            store.store(id, &data).unwrap();
        }

        // Corrupt the bytes in the FRAGMENTS table directly via redb. This
        // simulates a single-bit flip from media wear.
        {
            let path = std::path::Path::new(dir).join("fragments.redb");
            let db = redb::Database::create(&path).unwrap();
            let txn = db.begin_write().unwrap();
            {
                let mut t = txn
                    .open_table(crate::storage::engine::FRAGMENTS)
                    .unwrap();
                let mut tampered = data.clone();
                tampered[0] ^= 0x01;
                t.insert(&id, tampered.as_slice()).unwrap();
            }
            txn.commit().unwrap();
        }

        // Reopen and try to read with verify on — should error.
        {
            let store = FragmentStore::new_with_verify(dir, 1024 * 1024, true).unwrap();
            let err = store.retrieve(&id).unwrap_err();
            assert!(
                err.to_string().contains("hash mismatch"),
                "expected hash mismatch error, got: {err}"
            );
        }

        // Same store with verify off should still return the (corrupted)
        // bytes — opened in its own scope so the previous Database is fully
        // closed (redb forbids two open handles on the same file).
        let store_no_verify = FragmentStore::new(dir, 1024 * 1024).unwrap();
        let bytes = store_no_verify.retrieve(&id).unwrap().unwrap();
        assert_eq!(bytes[0], data[0] ^ 0x01); // corruption visible
    }

    /// On reopen, the used-bytes counter is loaded from the persisted
    /// `system.total_used_bytes` key — no fallback scan.
    #[test]
    fn test_persistent_counter_loaded_on_reopen() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().to_str().unwrap();

        let data: Vec<u8> = (0..1024).map(|i| (i % 251) as u8).collect();
        {
            let store = FragmentStore::new(dir, 1024 * 1024).unwrap();
            store.store_post_fragment(1, 0, &data).unwrap();
            assert_eq!(store.used_bytes(), data.len() as u64);
        }

        // Inspect the redb file directly: SYSTEM.total_used_bytes should
        // hold the persisted size.
        {
            let path = std::path::Path::new(dir).join("fragments.redb");
            let db = redb::Database::create(&path).unwrap();
            let txn = db.begin_read().unwrap();
            let t = txn.open_table(SYSTEM).unwrap();
            let v = t.get(KEY_TOTAL_USED_BYTES).unwrap().unwrap().value();
            assert_eq!(v, data.len() as u64);
        }

        let store = FragmentStore::new(dir, 1024 * 1024).unwrap();
        assert_eq!(store.used_bytes(), data.len() as u64);
    }

    /// If the SYSTEM counter is somehow missing (e.g., wiped, or migrated
    /// from Phase 1 data), opening the store recovers the value via a full
    /// scan and writes it back so the next reopen is fast.
    #[test]
    fn test_counter_recovers_when_system_key_missing() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().to_str().unwrap();

        let data: Vec<u8> = vec![0xAB; 2048];
        {
            let store = FragmentStore::new(dir, 1024 * 1024).unwrap();
            store.store_post_fragment(2, 0, &data).unwrap();
        }

        // Wipe the persisted counter.
        {
            let path = std::path::Path::new(dir).join("fragments.redb");
            let db = redb::Database::create(&path).unwrap();
            let txn = db.begin_write().unwrap();
            {
                let mut t = txn.open_table(SYSTEM).unwrap();
                t.remove(KEY_TOTAL_USED_BYTES).unwrap();
            }
            txn.commit().unwrap();
        }

        // Reopen — should recompute via scan and persist.
        let store = FragmentStore::new(dir, 1024 * 1024).unwrap();
        assert_eq!(store.used_bytes(), data.len() as u64);

        // And after one more reopen (post-recovery), the counter is back
        // in SYSTEM and used_bytes still matches.
        drop(store);
        let store = FragmentStore::new(dir, 1024 * 1024).unwrap();
        assert_eq!(store.used_bytes(), data.len() as u64);
    }
}

// === New post-based storage tests (T054) ===
// These tests should FAIL first (TDD approach), then pass after T059 implementation

mod post_fragment_storage {
    use super::*;

    /// Test: Store fragment by post_id and index
    #[test]
    fn test_store_post_fragment() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let post_id: u64 = 12345;
        let index: u32 = 0;
        let data = b"Hello, SSS fragment!".to_vec();

        // Store fragment by post_id and index
        let result = store.store_post_fragment(post_id, index, &data);
        assert!(result.is_ok(), "Should store fragment successfully");

        // Verify presence via the public API (storage backend is now redb,
        // not a per-fragment file on disk).
        assert!(store.post_fragment_exists(post_id, index));
        let retrieved = store.retrieve_post_fragment(post_id, index).unwrap();
        assert_eq!(retrieved.unwrap(), data);
    }

    /// Test: Retrieve fragment by post_id and index
    #[test]
    fn test_retrieve_post_fragment() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let post_id: u64 = 999;
        let index: u32 = 2;
        let data = b"Fragment data for testing retrieval".to_vec();

        // Store first
        store.store_post_fragment(post_id, index, &data).unwrap();

        // Retrieve
        let result = store.retrieve_post_fragment(post_id, index);
        assert!(result.is_ok());
        
        let retrieved = result.unwrap();
        assert!(retrieved.is_some(), "Fragment should be found");
        assert_eq!(retrieved.unwrap(), data, "Retrieved data should match");
    }

    /// Test: Retrieve non-existent fragment returns None
    #[test]
    fn test_retrieve_nonexistent_post_fragment() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let result = store.retrieve_post_fragment(99999, 0);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none(), "Non-existent fragment should return None");
    }

    /// Test: Store multiple fragments for same post
    #[test]
    fn test_store_multiple_fragments_same_post() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let post_id: u64 = 42;
        let n: u32 = 5; // 5 fragments (n=5)

        for index in 0..n {
            let data = format!("Fragment {} data", index).into_bytes();
            store.store_post_fragment(post_id, index, &data).unwrap();
        }

        // Verify all fragments stored via the public API
        let listed = store.list_post_fragments(post_id).unwrap();
        assert_eq!(listed.len(), n as usize);
        for index in 0..n {
            assert!(store.post_fragment_exists(post_id, index));
        }
    }

    /// Test: Idempotent storage (storing same fragment twice succeeds)
    #[test]
    fn test_idempotent_post_fragment_storage() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let post_id: u64 = 100;
        let index: u32 = 0;
        let data = b"Idempotent fragment".to_vec();

        // Store twice
        store.store_post_fragment(post_id, index, &data).unwrap();
        store.store_post_fragment(post_id, index, &data).unwrap();

        // Should still retrieve correctly
        let retrieved = store.retrieve_post_fragment(post_id, index).unwrap().unwrap();
        assert_eq!(retrieved, data);
    }

    /// Test: List fragments for a post
    #[test]
    fn test_list_post_fragments() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let post_id: u64 = 77;

        // Store fragments 0, 2, 4 (sparse)
        for index in [0, 2, 4] {
            let data = format!("Fragment {}", index).into_bytes();
            store.store_post_fragment(post_id, index, &data).unwrap();
        }

        let indices = store.list_post_fragments(post_id).unwrap();
        assert_eq!(indices.len(), 3);
        assert!(indices.contains(&0));
        assert!(indices.contains(&2));
        assert!(indices.contains(&4));
    }

    /// Test: Check fragment existence
    #[test]
    fn test_post_fragment_exists() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let post_id: u64 = 555;
        let data = b"Test fragment".to_vec();

        // Not exists initially
        assert!(!store.post_fragment_exists(post_id, 0));

        // Store
        store.store_post_fragment(post_id, 0, &data).unwrap();

        // Now exists
        assert!(store.post_fragment_exists(post_id, 0));
        assert!(!store.post_fragment_exists(post_id, 1)); // Other index doesn't exist
    }

    /// Test: Quota enforcement for post fragments
    #[test]
    fn test_post_fragment_quota() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 50).unwrap(); // 50 bytes only

        let data = vec![0u8; 100]; // 100 bytes - exceeds quota

        let result = store.store_post_fragment(1, 0, &data);
        assert!(result.is_err(), "Should fail with quota exceeded");
    }

    /// Test: Reopening the store recovers persisted fragments and the
    /// used-bytes counter from the redb file (TODO §4.9).
    #[test]
    fn test_persistence_across_reopen() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().to_str().unwrap();

        let post_id: u64 = 314;
        let data: Vec<u8> = (0..512).map(|i| (i % 251) as u8).collect();

        // Drop the first store explicitly — redb writes are committed per
        // call, but we want to confirm a clean re-open path too.
        {
            let store = FragmentStore::new(dir, 1024 * 1024).unwrap();
            store.store_post_fragment(post_id, 0, &data).unwrap();
            assert_eq!(store.used_bytes(), data.len() as u64);
        }

        let store = FragmentStore::new(dir, 1024 * 1024).unwrap();
        assert_eq!(store.used_bytes(), data.len() as u64, "used counter recovered on reopen");
        let got = store.retrieve_post_fragment(post_id, 0).unwrap().unwrap();
        assert_eq!(got, data);
    }

    /// Test: Delete post fragments (cleanup)
    #[test]
    fn test_delete_post_fragments() {
        let temp = TempDir::new().unwrap();
        let store = FragmentStore::new(temp.path().to_str().unwrap(), 1024 * 1024).unwrap();

        let post_id: u64 = 123;

        // Store fragments
        for index in 0..3u32 {
            let data = format!("Fragment {}", index).into_bytes();
            store.store_post_fragment(post_id, index, &data).unwrap();
        }

        // Verify stored
        assert_eq!(store.list_post_fragments(post_id).unwrap().len(), 3);

        // Delete all fragments for post
        store.delete_post_fragments(post_id).unwrap();

        // Should be empty
        assert_eq!(store.list_post_fragments(post_id).unwrap().len(), 0);
    }
}


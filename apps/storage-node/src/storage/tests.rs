//! Unit tests for fragment storage
//!
//! Tests both:
//! 1. Hash-based storage (legacy API)
//! 2. Post-based storage (new API for Post Storage Migration - T054)

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

        // Verify file exists at correct path: fragments/{post_id}/{index}.bin
        let expected_path = temp.path().join("fragments").join("12345").join("0.bin");
        assert!(expected_path.exists(), "Fragment file should exist at fragments/12345/0.bin");
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

        // Verify all fragments stored
        for index in 0..n {
            let path = temp.path()
                .join("fragments")
                .join("42")
                .join(format!("{}.bin", index));
            assert!(path.exists(), "Fragment {} should exist", index);
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

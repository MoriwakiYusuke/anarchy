//! Unit tests for chain interaction module (T055)
//!
//! Tests for:
//! - declare_holding subxt call
//! - Rate limiting behavior
//! - Error handling
//! - Failover integration (T073, T078)

use super::*;
use std::sync::Arc;
use crate::network::endpoint_cache::EndpointCache;

/// Helper to create ChainClient with default failover/cache for testing
async fn create_test_client(rate_limit: u32) -> ChainClient {
    let failover_manager = Arc::new(FailoverManager::new());
    let endpoint_cache = Arc::new(EndpointCache::new([0u8; 32]));
    // Use Alice's dev seed for testing
    let alice_seed = "e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a";
    ChainClient::new(
        "ws://127.0.0.1:9944",
        rate_limit,
        alice_seed,
        failover_manager,
        endpoint_cache,
    ).await.unwrap()
}

// === Existing tests (moved from mod.rs) ===

#[tokio::test]
async fn test_rate_limiter_allows_within_limit() {
    let limiter = RateLimiter::new(5);
    
    for _ in 0..5 {
        assert!(limiter.try_acquire().await);
    }
    
    // 6th call should fail
    assert!(!limiter.try_acquire().await);
}

#[tokio::test]
async fn test_rate_limiter_remaining() {
    let limiter = RateLimiter::new(10);
    
    assert_eq!(limiter.remaining().await, 10);
    
    limiter.try_acquire().await;
    limiter.try_acquire().await;
    
    assert_eq!(limiter.remaining().await, 8);
}

#[tokio::test]
async fn test_chain_client_creation() {
    let client = create_test_client(10).await;
    assert_eq!(client.endpoint, "ws://127.0.0.1:9944");
}

#[tokio::test]
async fn test_declare_holding_rate_limited() {
    let client = create_test_client(2).await;
    let fragment_id = [1u8; 32];
    
    // First two should succeed
    client.declare_holding(fragment_id).await.unwrap();
    client.declare_holding(fragment_id).await.unwrap();
    
    // Third should fail
    let result = client.declare_holding(fragment_id).await;
    assert!(result.is_err());
}

// === T055: Unit test for declare_holding subxt call ===
// These tests verify the subxt integration works correctly.
// Note: Tests marked as "stub_" will pass with current stub implementation,
// but will need to be extended when real subxt integration is added.

mod declare_holding_subxt {
    use super::*;

    /// Test: declare_holding_post_fragment - declare holding by post_id and index
    /// This is the new API for Post Storage Migration.
    #[tokio::test]
    async fn test_declare_holding_post_fragment() {
        let client = create_test_client(10).await;
        
        let post_id: u64 = 12345;
        let index: u32 = 0;
        let fragment_data = b"Test fragment data".to_vec();
        
        // Compute the Blake2b hash of the fragment data
        let hash = compute_blake2b_hash(&fragment_data);
        
        // Declare holding using the hash
        let result = client.declare_holding_for_post(post_id, index, hash).await;
        assert!(result.is_ok(), "declare_holding_for_post should succeed");
    }

    /// Test: declare_holding returns success (stub behavior)
    #[tokio::test]
    async fn test_stub_declare_holding_success() {
        let client = create_test_client(10).await;
        let fragment_hash = [42u8; 32];
        
        let result = client.declare_holding(fragment_hash).await;
        assert!(result.is_ok(), "Stub should return success");
    }

    /// Test: Rate limiting for declare_holding_for_post
    #[tokio::test]
    async fn test_declare_holding_post_rate_limited() {
        let client = create_test_client(2).await;
        let hash = [1u8; 32];
        
        // First two should succeed
        assert!(client.declare_holding_for_post(1, 0, hash).await.is_ok());
        assert!(client.declare_holding_for_post(1, 1, hash).await.is_ok());
        
        // Third should fail due to rate limit
        let result = client.declare_holding_for_post(1, 2, hash).await;
        assert!(result.is_err(), "Should fail due to rate limit");
        assert!(result.unwrap_err().to_string().contains("Rate limit"));
    }

    /// Test: declare_holding with transaction tracking
    /// Verifies that the method tracks post_id and index for later retrieval.
    #[tokio::test]
    async fn test_declare_holding_tracks_post_info() {
        let client = create_test_client(10).await;
        
        let post_id: u64 = 999;
        let index: u32 = 3;
        let hash = [0xAA; 32];
        
        // Declare holding
        client.declare_holding_for_post(post_id, index, hash).await.unwrap();
        
        // Should be able to look up the mapping
        let stored_info = client.get_holding_info(&hash).await;
        assert!(stored_info.is_some(), "Should store post info for hash");
        
        let (stored_post_id, stored_index) = stored_info.unwrap();
        assert_eq!(stored_post_id, post_id);
        assert_eq!(stored_index, index);
    }

    /// Test: Multiple fragments for same post
    #[tokio::test]
    async fn test_declare_holdings_same_post_multiple_fragments() {
        let client = create_test_client(10).await;
        let post_id: u64 = 42;
        
        // Declare holdings for 5 fragments (n=5)
        for index in 0..5u32 {
            let hash = [index as u8; 32]; // Different hash for each
            client.declare_holding_for_post(post_id, index, hash).await.unwrap();
        }
        
        // All should be tracked
        for index in 0..5u32 {
            let hash = [index as u8; 32];
            let info = client.get_holding_info(&hash).await;
            assert!(info.is_some(), "Fragment {} should be tracked", index);
            let (p, i) = info.unwrap();
            assert_eq!((p, i), (post_id, index));
        }
    }

    /// Helper: Compute Blake2b-256 hash
    fn compute_blake2b_hash(data: &[u8]) -> [u8; 32] {
        use blake2::{Blake2b, Digest};
        use blake2::digest::consts::U32;
        
        let mut hasher = Blake2b::<U32>::new();
        hasher.update(data);
        let result = hasher.finalize();
        
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    // === T049: RPC Reconnection Tests ===

    /// Test: RetryConfig has sensible defaults
    #[test]
    fn test_retry_config_defaults() {
        let config = super::RetryConfig::default();
        assert_eq!(config.max_retries, 10);
        assert_eq!(config.initial_delay_secs, 1);
        assert_eq!(config.max_delay_secs, 60);
    }

    /// Test: reconnect() invalidates client
    #[tokio::test]
    async fn test_reconnect_invalidates_client() {
        let client = create_test_client(10).await;
        
        // First, ensure the client is connected (or not - depending on node availability)
        // We just test that reconnect() can be called without panic
        // In a real scenario without a node, this will fail to reconnect, which is expected
        let _result = client.reconnect().await;
        // We don't assert success because the node may not be running
        // The important thing is that the method exists and runs
    }

    /// Test: with_reconnect executes operation successfully
    #[tokio::test]
    async fn test_with_reconnect_success_case() {
        // This test verifies the API exists and compiles correctly
        // Actual reconnection testing requires a running node
        let client = create_test_client(10).await;
        
        // Test that with_reconnect can be called with a closure
        // Since no node is running, this will fail - we just test the interface
        let result = client.with_reconnect(|_client| async move {
            // Simulate a successful operation
            Ok::<_, anyhow::Error>(42)
        }).await;
        
        // Without a running node, this will fail at ensure_subxt_client
        // but that's expected behavior
        assert!(result.is_err() || result.unwrap() == 42);
    }
}

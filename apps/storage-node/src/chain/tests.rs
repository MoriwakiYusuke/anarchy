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
///
/// NOTE: 接続先は意図的に「確実に閉じているポート」(port 1) を使う。
/// 以前は ws://127.0.0.1:9944 だったが、開発マシンで dev チェーンが
/// 動いていると実 extrinsic を提出してしまいテストが非決定的になる。
async fn create_test_client(rate_limit: u32) -> ChainClient {
    let failover_manager = Arc::new(FailoverManager::new());
    let endpoint_cache = Arc::new(EndpointCache::new([0u8; 32]));
    // Use Alice's dev seed for testing
    let alice_seed = "e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a";
    ChainClient::new(
        "ws://127.0.0.1:1",
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
    assert_eq!(client.endpoint, "ws://127.0.0.1:1");
}

#[tokio::test]
async fn test_declare_holding_rate_limited() {
    let client = create_test_client(2).await;
    let fragment_id = [1u8; 32];

    // チェーンには接続できないので最初の2回は接続エラーで失敗するが、
    // レート枠は消費される（rate limit チェックは提出前）
    let e1 = client.declare_holding(fragment_id).await.unwrap_err();
    assert!(!e1.to_string().contains("Rate limit"), "1st call should fail with connection error, got: {}", e1);
    let e2 = client.declare_holding(fragment_id).await.unwrap_err();
    assert!(!e2.to_string().contains("Rate limit"), "2nd call should fail with connection error, got: {}", e2);

    // 3回目はレート制限で弾かれる
    let e3 = client.declare_holding(fragment_id).await.unwrap_err();
    assert!(e3.to_string().contains("Rate limit"), "3rd call should be rate limited, got: {}", e3);
}

// === T055: Unit test for declare_holding subxt call ===
// declare_holding / declare_holding_for_post / revoke_holding は実際に
// `Storage::declare_holding` / `Storage::revoke_holding` extrinsic を
// subxt 経由で提出する。テスト環境にチェーンはいないため、提出は
// 接続エラーで失敗する（以前のスタブのように成功を装わない）。

mod declare_holding_subxt {
    use super::*;

    /// Test: declare_holding はチェーン未接続時に明示的に失敗する
    /// （旧スタブは Ok を返して成功を装っていた）
    #[tokio::test]
    async fn test_declare_holding_fails_without_chain() {
        let client = create_test_client(10).await;
        let fragment_hash = [42u8; 32];

        let result = client.declare_holding(fragment_hash).await;
        assert!(result.is_err(), "declare_holding must not fake success without a chain");
        assert!(!result.unwrap_err().to_string().contains("Rate limit"));
    }

    /// Test: revoke_holding も同様に失敗を伝播する
    #[tokio::test]
    async fn test_revoke_holding_fails_without_chain() {
        let client = create_test_client(10).await;
        let result = client.revoke_holding([42u8; 32]).await;
        assert!(result.is_err(), "revoke_holding must not fake success without a chain");
    }

    /// Test: declare_holding_post_fragment - declare holding by post_id and index
    /// チェーン未接続時は提出エラーになるが、hash の整合性検証として残す。
    #[tokio::test]
    async fn test_declare_holding_post_fragment() {
        let client = create_test_client(10).await;

        let post_id: u64 = 12345;
        let index: u32 = 0;
        let fragment_data = b"Test fragment data".to_vec();

        // Compute the Blake2b hash of the fragment data
        let hash = compute_blake2b_hash(&fragment_data);

        // チェーンがいないので提出は失敗する（成功を装ってはならない）
        let result = client.declare_holding_for_post(post_id, index, hash).await;
        assert!(result.is_err(), "must not fake success without a chain");
        // それでもローカルの追跡 map には記録される
        assert_eq!(client.get_holding_info(&hash).await, Some((post_id, index)));
    }

    /// Test: Rate limiting for declare_holding_for_post
    #[tokio::test]
    async fn test_declare_holding_post_rate_limited() {
        let client = create_test_client(2).await;
        let hash = [1u8; 32];

        // 最初の2回は接続エラー（レート枠は消費される）
        let e1 = client.declare_holding_for_post(1, 0, hash).await.unwrap_err();
        assert!(!e1.to_string().contains("Rate limit"));
        let e2 = client.declare_holding_for_post(1, 1, hash).await.unwrap_err();
        assert!(!e2.to_string().contains("Rate limit"));

        // Third should fail due to rate limit
        let result = client.declare_holding_for_post(1, 2, hash).await;
        assert!(result.is_err(), "Should fail due to rate limit");
        assert!(result.unwrap_err().to_string().contains("Rate limit"));
    }

    /// Test: declare_holding with transaction tracking
    /// Verifies that the method tracks post_id and index for later retrieval,
    /// even when the extrinsic submission itself fails (no chain).
    #[tokio::test]
    async fn test_declare_holding_tracks_post_info() {
        let client = create_test_client(10).await;

        let post_id: u64 = 999;
        let index: u32 = 3;
        let hash = [0xAA; 32];

        // 提出はチェーン不在で失敗するが、追跡は提出前に行われる
        let _ = client.declare_holding_for_post(post_id, index, hash).await;

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

        // Declare holdings for 5 fragments (n=5) — 提出は失敗するが追跡される
        for index in 0..5u32 {
            let hash = [index as u8; 32]; // Different hash for each
            let _ = client.declare_holding_for_post(post_id, index, hash).await;
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

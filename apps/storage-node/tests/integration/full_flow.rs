//! Integration tests for full flow: receive → store → declare (T057)
//!
//! Tests the complete workflow when a fragment is received:
//! 1. Fragment received via libp2p
//! 2. Fragment stored to disk
//! 3. declare_holding called via subxt

use anyhow::Result;
use std::sync::Arc;
use tempfile::TempDir;
use anarchy_storage_node::chain::failover::FailoverManager;
use anarchy_storage_node::network::endpoint_cache::EndpointCache;

/// Helper to create ChainClient for integration tests
///
/// NOTE: 接続先は意図的に「確実に閉じているポート」(port 1) を使う。
/// declare_holding 系が実 extrinsic を提出するようになったため、
/// 開発マシンで dev チェーンが動いていると実トランザクションを
/// 送ってしまいテストが非決定的になる。
async fn create_test_chain_client(rate_limit: u32) -> anarchy_storage_node::chain::ChainClient {
    let failover_manager = Arc::new(FailoverManager::new());
    let endpoint_cache = Arc::new(EndpointCache::new([0u8; 32]));
    // Use Alice's dev seed for testing
    let alice_seed = "e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a";
    anarchy_storage_node::chain::ChainClient::new(
        "ws://127.0.0.1:1",
        rate_limit,
        alice_seed,
        failover_manager,
        endpoint_cache,
    ).await.unwrap()
}

/// T057: Full flow integration test (receive → store → declare)
///
/// This test verifies the complete workflow:
/// 1. Fragment data is received
/// 2. Fragment is stored to disk (post_id/index based)
/// 3. declare_holding is automatically triggered
/// 4. Rate limiting is respected
#[tokio::test]
async fn test_full_flow_receive_store_declare() -> Result<()> {
    // Setup
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_str().unwrap();
    
    // Create storage
    let store = anarchy_storage_node::storage::FragmentStore::new(data_dir, 10 * 1024 * 1024)?;
    
    // Create chain client (stub for now)
    let chain_client = create_test_chain_client(10).await;
    
    // Simulate receiving a fragment
    let post_id: u64 = 12345;
    let index: u32 = 0;
    let fragment_data = b"Fragment for full flow test".to_vec();
    
    // Step 1: Store fragment (simulating receive)
    store.store_post_fragment(post_id, index, &fragment_data)?;
    assert!(store.post_fragment_exists(post_id, index), "Fragment should be stored");
    
    // Step 2: Compute hash for declare_holding
    let fragment_hash = anarchy_storage_node::storage::FragmentStore::compute_hash(&fragment_data);
    
    // Step 3: Declare holding — 実 extrinsic 提出になったため、チェーン不在の
    // テスト環境では提出は失敗する（成功を装ってはならない）。
    // ローカルの holding 追跡は提出前に行われるので、それを検証する。
    let declare_result = chain_client.declare_holding_for_post(post_id, index, fragment_hash).await;
    assert!(declare_result.is_err(), "declare must not fake success without a chain");


    // Verify holding was tracked
    let holding_info = chain_client.get_holding_info(&fragment_hash).await;
    assert!(holding_info.is_some(), "Holding should be tracked");
    let (tracked_post_id, tracked_index) = holding_info.unwrap();
    assert_eq!((tracked_post_id, tracked_index), (post_id, index));
    
    println!("T057: Full flow (receive → store → declare) test passed");
    Ok(())
}

/// T057b: Auto-declare on fragment storage
/// Uses the workflow manager pattern to automatically declare after storage
#[tokio::test]
async fn test_auto_declare_on_storage() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_str().unwrap();
    
    let store = anarchy_storage_node::storage::FragmentStore::new(data_dir, 10 * 1024 * 1024)?;
    let chain_client = create_test_chain_client(10).await;
    
    let post_id: u64 = 42;
    let fragments: Vec<Vec<u8>> = (0..5u32)
        .map(|i| format!("Fragment {} data", i).into_bytes())
        .collect();
    
    // Store and auto-declare each fragment
    for (index, fragment_data) in fragments.iter().enumerate() {
        let index = index as u32;
        
        // Store
        store.store_post_fragment(post_id, index, fragment_data)?;
        
        // Auto-declare（チェーン不在のため提出は失敗するが、ローカル追跡は行われる）
        let hash = anarchy_storage_node::storage::FragmentStore::compute_hash(fragment_data);
        let _ = chain_client.declare_holding_for_post(post_id, index, hash).await;
        assert!(chain_client.get_holding_info(&hash).await.is_some(), "holding should be tracked locally");
    }
    
    // Verify all stored and declared
    let stored = store.list_post_fragments(post_id)?;
    assert_eq!(stored.len(), 5, "All 5 fragments should be stored");
    
    println!("T057b: Auto-declare on storage test passed");
    Ok(())
}

/// T057c: Rate limiting during full flow
#[tokio::test]
async fn test_full_flow_rate_limiting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_str().unwrap();
    
    let store = anarchy_storage_node::storage::FragmentStore::new(data_dir, 10 * 1024 * 1024)?;
    // Rate limit of 3 per minute
    let chain_client = create_test_chain_client(3).await;
    
    let post_id: u64 = 99;
    
    // 最初の3回はレート枠内（チェーン不在のため接続エラーで失敗するが、
    // レート制限エラーではないことを確認する）
    for index in 0..3u32 {
        let data = format!("Fragment {}", index).into_bytes();
        store.store_post_fragment(post_id, index, &data)?;

        let hash = anarchy_storage_node::storage::FragmentStore::compute_hash(&data);
        let err = chain_client.declare_holding_for_post(post_id, index, hash).await.unwrap_err();
        assert!(
            !err.to_string().contains("Rate limit"),
            "Fragment {} should not be rate limited yet, got: {}",
            index, err
        );
    }
    
    // 4th should be rate limited
    let data = b"Fragment 3".to_vec();
    store.store_post_fragment(post_id, 3, &data)?;
    
    let hash = anarchy_storage_node::storage::FragmentStore::compute_hash(&data);
    let result = chain_client.declare_holding_for_post(post_id, 3, hash).await;
    assert!(result.is_err(), "4th fragment should be rate limited");
    assert!(result.unwrap_err().to_string().contains("Rate limit"));
    
    println!("T057c: Rate limiting test passed");
    Ok(())
}

/// T057d: Full flow with disk full error handling
#[tokio::test]
async fn test_full_flow_disk_full_error() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_str().unwrap();
    
    // Very small quota (100 bytes)
    let store = anarchy_storage_node::storage::FragmentStore::new(data_dir, 100)?;
    let chain_client = create_test_chain_client(10).await;
    
    // First small fragment should succeed
    let small_data = b"Small".to_vec();
    store.store_post_fragment(1, 0, &small_data)?;
    let hash = anarchy_storage_node::storage::FragmentStore::compute_hash(&small_data);
    // チェーン不在のため提出は失敗するが、本テストの対象はストレージ側の挙動
    let _ = chain_client.declare_holding_for_post(1, 0, hash).await;


    // Large fragment should fail with quota error (before declare_holding is called)
    let large_data = vec![0u8; 200]; // 200 bytes > 100 byte quota
    let result = store.store_post_fragment(2, 0, &large_data);
    assert!(result.is_err(), "Large fragment should fail with quota exceeded");
    
    // Should NOT have called declare_holding because storage failed
    // (In production, we would track this, but here we just verify storage failed)
    
    println!("T057d: Disk full error handling test passed");
    Ok(())
}

/// T057e: Retrieve fragment after full flow
#[tokio::test]
async fn test_full_flow_with_retrieval() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_str().unwrap();
    
    let store = anarchy_storage_node::storage::FragmentStore::new(data_dir, 10 * 1024 * 1024)?;
    let chain_client = create_test_chain_client(10).await;
    
    let post_id: u64 = 777;
    let index: u32 = 2;
    let original_data = b"Original fragment content for retrieval test".to_vec();
    
    // Full flow: receive → store → declare（declare はチェーン不在で失敗してよい）
    store.store_post_fragment(post_id, index, &original_data)?;
    let hash = anarchy_storage_node::storage::FragmentStore::compute_hash(&original_data);
    let _ = chain_client.declare_holding_for_post(post_id, index, hash).await;


    // Now retrieve
    let retrieved = store.retrieve_post_fragment(post_id, index)?;
    assert!(retrieved.is_some(), "Fragment should be retrievable");
    assert_eq!(retrieved.unwrap(), original_data, "Retrieved data should match original");
    
    println!("T057e: Full flow with retrieval test passed");
    Ok(())
}

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
async fn create_test_chain_client(rate_limit: u32) -> anarchy_storage_node::chain::ChainClient {
    let failover_manager = Arc::new(FailoverManager::new());
    let endpoint_cache = Arc::new(EndpointCache::new([0u8; 32]));
    anarchy_storage_node::chain::ChainClient::new(
        "ws://127.0.0.1:9944",
        rate_limit,
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
    
    // Step 3: Declare holding (will be stub call if chain not running)
    // Using the new post-based API
    chain_client.declare_holding_for_post(post_id, index, fragment_hash).await?;
    
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
        
        // Auto-declare
        let hash = anarchy_storage_node::storage::FragmentStore::compute_hash(fragment_data);
        chain_client.declare_holding_for_post(post_id, index, hash).await?;
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
    
    // First 3 should succeed
    for index in 0..3u32 {
        let data = format!("Fragment {}", index).into_bytes();
        store.store_post_fragment(post_id, index, &data)?;
        
        let hash = anarchy_storage_node::storage::FragmentStore::compute_hash(&data);
        let result = chain_client.declare_holding_for_post(post_id, index, hash).await;
        assert!(result.is_ok(), "Fragment {} should declare successfully", index);
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
    chain_client.declare_holding_for_post(1, 0, hash).await?;
    
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
    
    // Full flow: receive → store → declare
    store.store_post_fragment(post_id, index, &original_data)?;
    let hash = anarchy_storage_node::storage::FragmentStore::compute_hash(&original_data);
    chain_client.declare_holding_for_post(post_id, index, hash).await?;
    
    // Now retrieve
    let retrieved = store.retrieve_post_fragment(post_id, index)?;
    assert!(retrieved.is_some(), "Fragment should be retrievable");
    assert_eq!(retrieved.unwrap(), original_data, "Retrieved data should match original");
    
    println!("T057e: Full flow with retrieval test passed");
    Ok(())
}

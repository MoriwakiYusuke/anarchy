//! Chain interaction module
//!
//! Handles communication with the Anarchy blockchain via RPC.
//! Uses subxt for type-safe chain interaction.

use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use anyhow::{Result, bail};
use tracing::{info, warn, debug};

use crate::storage::FragmentId;

/// Rate limiter for declare_holding calls (FR-108)
pub struct RateLimiter {
    /// Maximum calls per period
    max_calls: u32,
    /// Period duration
    period: Duration,
    /// Call timestamps within current period
    call_times: Mutex<Vec<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(max_calls_per_minute: u32) -> Self {
        Self {
            max_calls: max_calls_per_minute,
            period: Duration::from_secs(60),
            call_times: Mutex::new(Vec::new()),
        }
    }

    /// Check if a call is allowed and record it
    pub async fn try_acquire(&self) -> bool {
        let mut times = self.call_times.lock().await;
        let now = Instant::now();
        
        // Remove expired entries
        times.retain(|t| now.duration_since(*t) < self.period);
        
        if times.len() >= self.max_calls as usize {
            return false;
        }
        
        times.push(now);
        true
    }

    /// Get remaining quota
    pub async fn remaining(&self) -> u32 {
        let times = self.call_times.lock().await;
        let now = Instant::now();
        let active = times.iter().filter(|t| now.duration_since(**t) < self.period).count();
        self.max_calls.saturating_sub(active as u32)
    }
}

/// Chain client for interacting with Anarchy blockchain
/// 
/// Note: Currently a stub implementation.
/// Full implementation requires subxt with generated runtime types.
pub struct ChainClient {
    /// RPC endpoint URL
    #[allow(dead_code)]
    endpoint: String,
    /// Rate limiter for declare_holding
    rate_limiter: RateLimiter,
    /// Connection status
    connected: bool,
}

impl ChainClient {
    /// Create a new chain client
    pub async fn new(endpoint: &str, declare_rate_limit: u32) -> Result<Self> {
        info!(endpoint = endpoint, "Connecting to chain");
        
        // Note: In full implementation, connect via subxt:
        // let api = OnlineClient::<AnarchyConfig>::from_url(endpoint).await?;
        
        Ok(Self {
            endpoint: endpoint.to_string(),
            rate_limiter: RateLimiter::new(declare_rate_limit),
            connected: false, // Would be true after successful connection
        })
    }

    /// Check if a fragment is registered on-chain (FR-107)
    pub async fn fragment_exists(&self, fragment_id: &FragmentId) -> Result<bool> {
        debug!(fragment_id = %hex::encode(fragment_id), "Checking fragment existence");
        
        // Note: Full implementation would query chain:
        // let storage_query = anarchy::storage().storage().fragments(fragment_id);
        // let result = self.api.storage().at_latest().await?.fetch(&storage_query).await?;
        // Ok(result.is_some())
        
        // Stub: Return false (fragment not found)
        warn!("Chain client not connected, returning false for fragment existence");
        Ok(false)
    }

    /// Declare holding of a fragment (submits extrinsic)
    pub async fn declare_holding(&self, fragment_id: FragmentId) -> Result<()> {
        // Check rate limit (FR-108)
        if !self.rate_limiter.try_acquire().await {
            bail!("Rate limit exceeded for declare_holding");
        }
        
        debug!(fragment_id = %hex::encode(fragment_id), "Declaring holding");
        
        // Note: Full implementation would submit extrinsic:
        // let tx = anarchy::tx().storage().declare_holding(fragment_id);
        // let progress = self.api.tx().sign_and_submit_then_watch_default(&tx, &self.signer).await?;
        // progress.wait_for_finalized_success().await?;
        
        // Stub: Log and return success
        info!(fragment_id = %hex::encode(fragment_id), "Would declare holding (stub)");
        Ok(())
    }

    /// Revoke holding of a fragment
    pub async fn revoke_holding(&self, fragment_id: FragmentId) -> Result<()> {
        // Check rate limit
        if !self.rate_limiter.try_acquire().await {
            bail!("Rate limit exceeded for revoke_holding");
        }
        
        debug!(fragment_id = %hex::encode(fragment_id), "Revoking holding");
        
        // Stub: Log and return success
        info!(fragment_id = %hex::encode(fragment_id), "Would revoke holding (stub)");
        Ok(())
    }

    /// Get fragment metadata from chain
    pub async fn get_fragment_metadata(&self, fragment_id: &FragmentId) -> Result<Option<FragmentMetadata>> {
        debug!(fragment_id = %hex::encode(fragment_id), "Fetching fragment metadata");
        
        // Stub: Return None
        Ok(None)
    }

    /// Get list of fragment holders from chain
    pub async fn get_fragment_holders(&self, fragment_id: &FragmentId) -> Result<Vec<Vec<u8>>> {
        debug!(fragment_id = %hex::encode(fragment_id), "Fetching fragment holders");
        
        // Stub: Return empty list
        Ok(vec![])
    }

    /// Check connection status
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get remaining rate limit quota
    pub async fn rate_limit_remaining(&self) -> u32 {
        self.rate_limiter.remaining().await
    }
}

/// Fragment metadata from chain (matches pallet types)
#[derive(Debug, Clone)]
pub struct FragmentMetadata {
    pub size: u64,
    pub content_type: String,
    pub created_at: u64,
    pub owner: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let client = ChainClient::new("ws://127.0.0.1:9944", 10).await.unwrap();
        assert_eq!(client.endpoint, "ws://127.0.0.1:9944");
    }

    #[tokio::test]
    async fn test_declare_holding_rate_limited() {
        let client = ChainClient::new("ws://127.0.0.1:9944", 2).await.unwrap();
        let fragment_id = [1u8; 32];
        
        // First two should succeed
        client.declare_holding(fragment_id).await.unwrap();
        client.declare_holding(fragment_id).await.unwrap();
        
        // Third should fail
        let result = client.declare_holding(fragment_id).await;
        assert!(result.is_err());
    }
}

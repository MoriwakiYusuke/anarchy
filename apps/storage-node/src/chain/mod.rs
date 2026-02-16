//! Chain interaction module
//!
//! Handles communication with the Anarchy blockchain via RPC.
//! Uses subxt for type-safe chain interaction.

pub mod failover;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use anyhow::{Result, bail};
use tracing::{info, warn, debug};

use crate::network::endpoint_cache::{BlockchainEndpoint, EndpointCache};
use crate::storage::FragmentId;
use failover::FailoverManager;

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
    /// RPC endpoint URL (initial endpoint)
    #[allow(dead_code)]
    endpoint: String,
    /// Failover manager for blockchain connections (FR-510, FR-511)
    failover_manager: Arc<FailoverManager>,
    /// Endpoint cache for peer discovery (FR-507)
    endpoint_cache: Arc<EndpointCache>,
    /// Rate limiter for declare_holding
    rate_limiter: RateLimiter,
    /// Connection status
    connected: bool,
    /// Track holdings: hash → (post_id, index)
    holding_map: Mutex<HashMap<FragmentId, (u64, u32)>>,
}

impl ChainClient {
    /// Create a new chain client
    pub async fn new(
        endpoint: &str,
        declare_rate_limit: u32,
        failover_manager: Arc<FailoverManager>,
        endpoint_cache: Arc<EndpointCache>,
    ) -> Result<Self> {
        info!(endpoint = endpoint, "Connecting to chain");
        
        // Set initial endpoint as primary in failover manager
        let initial_endpoint = BlockchainEndpoint {
            url: endpoint.to_string(),
            chain_id: [0u8; 32], // Dev chain_id
            last_verified: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            latency_ms: 0,
            ttl_secs: 300,
        };
        failover_manager.set_primary(initial_endpoint).await;
        
        // Note: In full implementation, connect via subxt:
        // let api = OnlineClient::<AnarchyConfig>::from_url(endpoint).await?;
        
        Ok(Self {
            endpoint: endpoint.to_string(),
            failover_manager,
            endpoint_cache,
            rate_limiter: RateLimiter::new(declare_rate_limit),
            connected: false, // Would be true after successful connection
            holding_map: Mutex::new(HashMap::new()),
        })
    }

    /// Get the current active endpoint URL (from failover manager)
    pub async fn active_endpoint(&self) -> Option<String> {
        self.failover_manager.get_primary_url().await
    }

    /// Report a successful RPC call
    pub async fn report_success(&self) {
        self.failover_manager.record_primary_success().await;
    }

    /// Report a failed RPC call and potentially trigger failover
    /// 
    /// Returns the new primary URL if failover occurred
    pub async fn report_failure(&self) -> Option<String> {
        let new_primary = self.failover_manager.record_primary_failure().await;
        if new_primary.is_some() {
            // Try to add standbys from endpoint cache
            self.refresh_standbys_from_cache().await;
        }
        new_primary
    }

    /// Refresh standby connections from endpoint cache
    async fn refresh_standbys_from_cache(&self) {
        let endpoints = self.endpoint_cache.get_all_by_latency().await;
        let current_primary = self.failover_manager.get_primary_url().await;
        
        for endpoint in endpoints.into_iter().take(5) {
            // Don't add current primary as standby
            if Some(&endpoint.url) != current_primary.as_ref() {
                self.failover_manager.add_standby(endpoint).await;
            }
        }
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

    /// Declare holding for a post fragment (T060)
    /// 
    /// This tracks the post_id + index for the given fragment hash,
    /// then submits the declare_holding extrinsic.
    pub async fn declare_holding_for_post(
        &self,
        post_id: u64,
        index: u32,
        fragment_hash: FragmentId,
    ) -> Result<()> {
        // Check rate limit (FR-108)
        if !self.rate_limiter.try_acquire().await {
            bail!("Rate limit exceeded for declare_holding");
        }
        
        // Track the mapping: hash → (post_id, index)
        {
            let mut map = self.holding_map.lock().await;
            map.insert(fragment_hash, (post_id, index));
        }
        
        debug!(
            post_id = post_id,
            index = index,
            hash = %hex::encode(fragment_hash),
            "Declaring holding for post fragment"
        );
        
        // Note: Full implementation would submit extrinsic:
        // let tx = anarchy::tx().storage().declare_holding(fragment_hash);
        // let progress = self.api.tx().sign_and_submit_then_watch_default(&tx, &self.signer).await?;
        // progress.wait_for_finalized_success().await?;
        
        // Stub: Log and return success
        info!(
            post_id = post_id,
            index = index,
            hash = %hex::encode(fragment_hash),
            "Would declare holding for post fragment (stub)"
        );
        Ok(())
    }

    /// Get holding info for a fragment hash
    /// Returns (post_id, index) if tracked
    pub async fn get_holding_info(&self, fragment_hash: &FragmentId) -> Option<(u64, u32)> {
        let map = self.holding_map.lock().await;
        map.get(fragment_hash).copied()
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

    /// Submit KZG holding proof to chain (T083)
    /// 
    /// Submits `prove_holding_kzg` extrinsic with the proof data.
    /// Called by ChallengeMonitor after generating a valid proof.
    pub async fn submit_holding_proof(
        &self,
        content_hash: [u8; 32],
        share_index: u8,
        share_value: [u8; 32],
        proof: [u8; 48],
    ) -> Result<()> {
        debug!(
            content_hash = %hex::encode(content_hash),
            share_index = share_index,
            "Submitting KZG holding proof to chain"
        );
        
        // Note: Full implementation would use subxt/PAPI:
        // let tx = anarchy::tx().storage().prove_holding_kzg(
        //     content_hash,
        //     share_index,
        //     share_value,
        //     proof,
        // );
        // let progress = self.api.tx().sign_and_submit_then_watch_default(&tx, &self.signer).await?;
        // progress.wait_for_finalized_success().await?;
        
        // Alternative: HTTP JSON-RPC (matches register_with_blockchain pattern)
        #[derive(serde::Serialize)]
        struct ProveHoldingParams {
            content_hash: String,
            share_index: u8,
            share_value: String,
            proof: String,
        }
        
        #[derive(serde::Serialize)]
        struct RpcRequest {
            jsonrpc: &'static str,
            id: u32,
            method: &'static str,
            params: ProveHoldingParams,
        }
        
        #[derive(serde::Deserialize)]
        struct RpcResponse {
            result: Option<bool>,
            error: Option<RpcError>,
        }
        
        #[derive(serde::Deserialize)]
        struct RpcError {
            message: String,
        }
        
        let client = reqwest::Client::new();
        let request = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "storage_proveHoldingKzg",
            params: ProveHoldingParams {
                content_hash: hex::encode(content_hash),
                share_index,
                share_value: hex::encode(share_value),
                proof: hex::encode(proof),
            },
        };
        
        // Retry loop for failover (max 3 attempts)
        const MAX_RETRIES: u32 = 3;
        let mut last_error = None;
        
        for attempt in 0..MAX_RETRIES {
            let endpoint_url = match self.active_endpoint().await {
                Some(url) => url,
                None => {
                    return Err(anyhow::anyhow!("No primary endpoint available"));
                }
            };
            
            let http_endpoint = endpoint_url
                .replace("ws://", "http://")
                .replace("wss://", "https://");
            
            if attempt > 0 {
                info!(
                    blockchain = %http_endpoint,
                    attempt = attempt + 1,
                    "Retrying proof submission after failover"
                );
            } else {
                info!(
                    blockchain = %http_endpoint,
                    content_hash = %hex::encode(content_hash),
                    "Submitting KZG holding proof"
                );
            }
            
            match client
                .post(&http_endpoint)
                .json(&request)
                .send()
                .await
            {
                Ok(resp) => {
                    self.report_success().await;
                    
                    let rpc_response: RpcResponse = resp
                        .json()
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;
                    
                    if let Some(error) = rpc_response.error {
                        bail!("Blockchain rejected proof: {}", error.message);
                    }
                    
                    if rpc_response.result == Some(true) {
                        info!(
                            content_hash = %hex::encode(content_hash),
                            "Successfully submitted KZG holding proof"
                        );
                    }
                    
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    if let Some(new_primary) = self.report_failure().await {
                        info!(new_primary = %new_primary, "Failover triggered, will retry with new primary");
                        continue;
                    }
                    break;
                }
            }
        }
        
        Err(anyhow::anyhow!(
            "Failed to submit proof after {} attempts: {}",
            MAX_RETRIES,
            last_error.map(|e| e.to_string()).unwrap_or_default()
        ))
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

    /// Register this Storage Node with the blockchain node
    /// 
    /// Calls the `storage_registerEndpoint` RPC to register our HTTP endpoint.
    /// This allows the blockchain node to forward fragment requests to us.
    /// Uses failover manager to handle primary node failures (FR-510, FR-511).
    pub async fn register_with_blockchain(&self, our_rpc_url: &str) -> Result<()> {
        #[derive(serde::Serialize)]
        struct RpcRequest<'a> {
            jsonrpc: &'static str,
            id: u32,
            method: &'static str,
            params: [&'a str; 1],
        }
        
        #[derive(serde::Deserialize)]
        struct RpcResponse {
            result: Option<bool>,
            error: Option<RpcError>,
        }
        
        #[derive(serde::Deserialize)]
        struct RpcError {
            message: String,
        }
        
        let client = reqwest::Client::new();
        let request = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "storage_registerEndpoint",
            params: [our_rpc_url],
        };
        
        // Retry loop for failover (max 3 attempts)
        const MAX_RETRIES: u32 = 3;
        let mut last_error = None;
        
        for attempt in 0..MAX_RETRIES {
            // Get current primary endpoint from failover manager
            let endpoint_url = match self.active_endpoint().await {
                Some(url) => url,
                None => {
                    return Err(anyhow::anyhow!("No primary endpoint available"));
                }
            };
            
            // Convert ws:// to http:// for JSON-RPC
            let http_endpoint = endpoint_url
                .replace("ws://", "http://")
                .replace("wss://", "https://");
            
            if attempt > 0 {
                info!(
                    blockchain = %http_endpoint,
                    attempt = attempt + 1,
                    "Retrying registration after failover"
                );
            } else {
                info!(
                    blockchain = %http_endpoint,
                    storage_node = %our_rpc_url,
                    "Registering Storage Node with blockchain"
                );
            }
            
            match client
                .post(&http_endpoint)
                .json(&request)
                .send()
                .await
            {
                Ok(resp) => {
                    self.report_success().await;
                    
                    let rpc_response: RpcResponse = resp
                        .json()
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;
                    
                    if let Some(error) = rpc_response.error {
                        bail!("Blockchain node rejected registration: {}", error.message);
                    }
                    
                    if rpc_response.result == Some(true) {
                        info!("Successfully registered with blockchain node");
                    }
                    
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    // Report failure to failover manager
                    if let Some(new_primary) = self.report_failure().await {
                        info!(new_primary = %new_primary, "Failover triggered, will retry with new primary");
                        continue;
                    }
                    // No failover possible, break and return error
                    break;
                }
            }
        }
        
        Err(anyhow::anyhow!(
            "Failed to connect to blockchain node after {} attempts: {}",
            MAX_RETRIES,
            last_error.map(|e| e.to_string()).unwrap_or_default()
        ))
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

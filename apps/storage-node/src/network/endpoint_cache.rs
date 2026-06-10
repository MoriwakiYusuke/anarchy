//! Blockchain endpoint cache with TTL
//!
//! Caches blockchain RPC endpoints discovered via Gossipsub (FR-506, FR-508).
//! Entries expire after TTL and are garbage collected periodically.
//!
//! ## TTL
//!
//! Default TTL is 300 seconds (5 minutes). Endpoints are re-verified before
//! removal during garbage collection.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Default TTL for cached endpoints (5 minutes)
pub const DEFAULT_TTL_SECS: u32 = 300;

/// Garbage collection interval (1 minute)
pub const GC_INTERVAL: Duration = Duration::from_secs(60);

/// Blockchain endpoint information (shared via Gossipsub)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainEndpoint {
    /// WebSocket RPC URL (max 256 bytes)
    pub url: String,
    
    /// Chain genesis hash (for verification)
    pub chain_id: [u8; 32],
    
    /// Last successful health check timestamp (Unix seconds)
    pub last_verified: u64,
    
    /// Average latency in milliseconds
    pub latency_ms: u32,
    
    /// Time-to-live in seconds
    pub ttl_secs: u32,
}

impl BlockchainEndpoint {
    /// Check if this endpoint has expired
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        now.saturating_sub(self.last_verified) > self.ttl_secs as u64
    }
    
    /// Update the last_verified timestamp to now
    pub fn refresh(&mut self) {
        self.last_verified = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

/// Cached entry with local metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    endpoint: BlockchainEndpoint,
    #[allow(dead_code)] // For potential future TTL-based eviction
    added_at: Instant,
    verified_locally: bool,
}

/// Endpoint cache with TTL-based expiration
pub struct EndpointCache {
    /// URL -> CacheEntry
    entries: RwLock<HashMap<String, CacheEntry>>,
    
    /// Expected chain ID (for verification)
    expected_chain_id: [u8; 32],
}

impl EndpointCache {
    /// Create a new endpoint cache
    pub fn new(expected_chain_id: [u8; 32]) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            expected_chain_id,
        }
    }
    
    /// Add or update an endpoint in the cache
    ///
    /// Returns false if the URL fails validation or chain_id doesn't match (FR-507)
    pub async fn insert(&self, endpoint: BlockchainEndpoint) -> bool {
        // (#31-H-5 と同等) gossip 経由の URL を検証する。failover 時に
        // ここの URL へサーバ側から接続する (subxt / HTTP JSON-RPC) ため、
        // private IP / localhost 等を許すと SSRF になる。また dev の
        // chain_id は周知の [0u8; 32] なので chain_id チェックだけでは
        // 偽エンドポイントの混入 (failover リスト汚染) を防げない。
        // genesis hash の実照合は ChainClient::refresh_standbys_from_cache
        // が昇格前に行う。
        if let Err(reason) = crate::network::storage_node_cache::validate_gossip_url(
            &endpoint.url,
            &["ws", "wss"],
        ) {
            warn!(
                "Rejecting blockchain endpoint URL {}: {}",
                endpoint.url, reason
            );
            return false;
        }

        // Verify chain ID matches
        if endpoint.chain_id != self.expected_chain_id {
            warn!(
                "Rejecting endpoint {} with mismatched chain_id",
                endpoint.url
            );
            return false;
        }

        let entry = CacheEntry {
            endpoint: endpoint.clone(),
            added_at: Instant::now(),
            verified_locally: false,
        };
        
        let mut entries = self.entries.write().await;
        let url_key = endpoint.url.clone();
        entries.insert(url_key.clone(), entry);
        
        debug!("Added endpoint to cache: {}", url_key);
        true
    }
    
    /// Get an endpoint by URL
    pub async fn get(&self, url: &str) -> Option<BlockchainEndpoint> {
        let entries = self.entries.read().await;
        entries.get(url).map(|e| e.endpoint.clone())
    }
    
    /// Get all non-expired endpoints sorted by latency
    pub async fn get_all_by_latency(&self) -> Vec<BlockchainEndpoint> {
        let entries = self.entries.read().await;
        let mut endpoints: Vec<_> = entries
            .values()
            .filter(|e| !e.endpoint.is_expired())
            .map(|e| e.endpoint.clone())
            .collect();
        
        endpoints.sort_by_key(|e| e.latency_ms);
        endpoints
    }
    
    /// Remove an endpoint from the cache
    pub async fn remove(&self, url: &str) {
        let mut entries = self.entries.write().await;
        entries.remove(url);
        debug!("Removed endpoint from cache: {}", url);
    }
    
    /// Run garbage collection (remove expired entries)
    ///
    /// Returns the number of entries removed
    pub async fn gc(&self) -> usize {
        let mut entries = self.entries.write().await;
        let before = entries.len();
        
        entries.retain(|url, entry| {
            let keep = !entry.endpoint.is_expired();
            if !keep {
                debug!("GC: removing expired endpoint {}", url);
            }
            keep
        });
        
        before - entries.len()
    }
    
    /// Get the number of cached endpoints
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }
    
    /// Check if the cache is empty
    pub async fn is_empty(&self) -> bool {
        self.entries.read().await.is_empty()
    }
    
    /// Mark an endpoint as locally verified
    pub async fn mark_verified(&self, url: &str, latency_ms: u32) {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(url) {
            entry.verified_locally = true;
            entry.endpoint.latency_ms = latency_ms;
            entry.endpoint.refresh();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_endpoint(url: &str) -> BlockchainEndpoint {
        BlockchainEndpoint {
            url: url.to_string(),
            chain_id: [0u8; 32],
            last_verified: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            latency_ms: 100,
            ttl_secs: DEFAULT_TTL_SECS,
        }
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let cache = EndpointCache::new([0u8; 32]);
        let endpoint = make_endpoint("ws://node1.example.com:9944");

        assert!(cache.insert(endpoint.clone()).await);

        let retrieved = cache.get("ws://node1.example.com:9944").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().url, "ws://node1.example.com:9944");
    }

    #[tokio::test]
    async fn test_reject_mismatched_chain_id() {
        let cache = EndpointCache::new([1u8; 32]);
        // URL は valid だが chain_id が不一致のケース
        let endpoint = make_endpoint("ws://node1.example.com:9944");

        // Endpoint has chain_id [0; 32], cache expects [1; 32]
        assert!(!cache.insert(endpoint).await);
        assert!(cache.is_empty().await);
    }

    /// (#31-H-5 と同等) gossip 経由 URL のバリデーション: SSRF 対象に
    /// なり得る URL や不正スキームは chain_id が一致していても拒否する。
    #[tokio::test]
    async fn test_reject_invalid_endpoint_urls() {
        let cache = EndpointCache::new([0u8; 32]);
        let bad_urls = [
            "ws://localhost:9944",          // localhost
            "ws://127.0.0.1:9944",          // loopback IP
            "ws://192.168.1.10:9944",       // private IPv4
            "ws://10.0.0.1:9944",           // private IPv4
            "ws://169.254.0.1:9944",        // link-local IPv4
            "ws://[::1]:9944",              // IPv6 loopback
            "ws://mynode.local:9944",       // mDNS
            "http://node.example.com:9944", // ws/wss 以外のスキーム
            "file:///etc/passwd",           // 論外スキーム
            "not a url",                    // パース不能
        ];
        for url in bad_urls {
            let endpoint = make_endpoint(url);
            assert!(
                !cache.insert(endpoint).await,
                "should reject endpoint URL: {}",
                url
            );
        }
        assert!(cache.is_empty().await);

        // 正常系: 公開ホスト名 + ws/wss は受理される
        assert!(cache.insert(make_endpoint("ws://node.example.com:9944")).await);
        assert!(cache.insert(make_endpoint("wss://node2.example.com:9944")).await);
        assert_eq!(cache.len().await, 2);
    }

    #[tokio::test]
    async fn test_sorted_by_latency() {
        let cache = EndpointCache::new([0u8; 32]);
        
        let mut e1 = make_endpoint("ws://slow:9944");
        e1.latency_ms = 500;
        
        let mut e2 = make_endpoint("ws://fast:9944");
        e2.latency_ms = 50;
        
        cache.insert(e1).await;
        cache.insert(e2).await;
        
        let sorted = cache.get_all_by_latency().await;
        assert_eq!(sorted[0].url, "ws://fast:9944");
        assert_eq!(sorted[1].url, "ws://slow:9944");
    }

    #[test]
    fn test_endpoint_expired() {
        let mut endpoint = make_endpoint("ws://test:9944");
        endpoint.last_verified = 0; // Very old
        endpoint.ttl_secs = 1;
        
        assert!(endpoint.is_expired());
    }

    // T082: Test endpoint TTL expiration
    #[tokio::test]
    async fn t082_endpoint_ttl_expiration() {
        let cache = EndpointCache::new([0u8; 32]);
        
        // Create an endpoint that's already expired
        let expired_endpoint = BlockchainEndpoint {
            url: "ws://expired:9944".to_string(),
            chain_id: [0u8; 32],
            last_verified: 0, // Unix epoch = very old
            latency_ms: 100,
            ttl_secs: 1, // 1 second TTL - definitely expired
        };
        
        cache.insert(expired_endpoint.clone()).await;
        
        // Expired endpoints should not appear in get_all_by_latency
        let all = cache.get_all_by_latency().await;
        assert!(all.is_empty(), "Expired endpoint should not be returned");
        
        // Test GC removes expired entries
        let removed = cache.gc().await;
        assert_eq!(removed, 1, "GC should remove 1 expired entry");
        
        // Cache should now be empty
        assert!(cache.is_empty().await);
    }

    // Test refresh updates last_verified
    #[test]
    fn test_endpoint_refresh() {
        let mut endpoint = BlockchainEndpoint {
            url: "ws://test:9944".to_string(),
            chain_id: [0u8; 32],
            last_verified: 1000, // Old timestamp
            latency_ms: 100,
            ttl_secs: 300,
        };
        
        let old_verified = endpoint.last_verified;
        endpoint.refresh();
        
        // Should be updated to current time (much larger)
        assert!(endpoint.last_verified > old_verified);
    }
}

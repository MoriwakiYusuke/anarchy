//! Storage node endpoint cache with TTL
//!
//! Caches storage node HTTP RPC endpoints discovered via Gossipsub (FR-515, FR-516).
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

/// Maximum length of an accepted storage-node URL.
const MAX_URL_LEN: usize = 256;

/// (#31-H-5) Validate an incoming gossip URL before it is cached / health-checked.
///
/// Rejects:
/// - Schemes not in `allowed_schemes`
/// - URLs longer than [`MAX_URL_LEN`]
/// - Malformed URLs (`url::Url::parse` failure)
/// - URLs whose host is missing, an IP literal in a private / loopback /
///   link-local range, the literal `localhost`, or an mDNS `*.local` name
///
/// This is a server-side fetch target (see `health_check`, and the
/// blockchain endpoint failover path), so the rejection list mirrors
/// classic SSRF defences. Public hostnames / public IPv4 / global IPv6
/// are allowed.
///
/// `EndpointCache` (blockchain endpoints, ws/wss) からも再利用するため
/// スキーム集合をパラメータ化している。
pub(crate) fn validate_gossip_url(url: &str, allowed_schemes: &[&str]) -> Result<(), &'static str> {
    if url.len() > MAX_URL_LEN {
        return Err("URL too long");
    }
    let parsed = url::Url::parse(url).map_err(|_| "URL parse failed")?;
    if !allowed_schemes.contains(&parsed.scheme()) {
        return Err("URL scheme not allowed");
    }
    // NOTE: `host_str()` は IPv6 リテラルをブラケット付き ("[::1]") で返すため
    // `parse::<IpAddr>()` が失敗して IP チェックをすり抜けていた。
    // `host()` の Host enum で分岐して確実に IP 判定する。
    match parsed.host().ok_or("URL has no host")? {
        url::Host::Domain(domain) => {
            if domain.eq_ignore_ascii_case("localhost")
                || domain.to_ascii_lowercase().ends_with(".local")
            {
                return Err("host is localhost or .local");
            }
        }
        url::Host::Ipv4(v4) => {
            if v4.is_loopback() || v4.is_private() || v4.is_link_local()
                || v4.is_broadcast() || v4.is_unspecified() || v4.is_multicast()
            {
                return Err("IPv4 address is private/loopback/link-local");
            }
        }
        url::Host::Ipv6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return Err("IPv6 address is loopback/unspecified/multicast");
            }
            // Reject ULA (fc00::/7). Stable IpAddr doesn't expose
            // is_unique_local on stable, so detect manually.
            let segs = v6.segments();
            if (segs[0] & 0xfe00) == 0xfc00 {
                return Err("IPv6 ULA (fc00::/7) rejected");
            }
            // Reject link-local fe80::/10
            if (segs[0] & 0xffc0) == 0xfe80 {
                return Err("IPv6 link-local rejected");
            }
        }
    }
    Ok(())
}

/// (#31-H-5) Storage node HTTP endpoint 用のバリデーション (http/https のみ)。
fn validate_endpoint_url(url: &str) -> Result<(), &'static str> {
    validate_gossip_url(url, &["http", "https"])
}

/// Default TTL for cached storage node endpoints (5 minutes)
pub const DEFAULT_STORAGE_NODE_TTL_SECS: u32 = 300;

/// Garbage collection interval (1 minute)
pub const STORAGE_NODE_GC_INTERVAL: Duration = Duration::from_secs(60);

/// Storage node endpoint information (shared via Gossipsub)
/// 
/// FR-515: Storage node HTTP RPC endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageNodeEndpoint {
    /// HTTP RPC URL (e.g., "http://test-node:3030")
    pub url: String,
    
    /// Last successful health check timestamp (Unix seconds)
    pub last_verified: u64,
    
    /// Average latency in milliseconds
    pub latency_ms: u32,
    
    /// Time-to-live in seconds (FR-516: default 300s)
    pub ttl_secs: u32,
}

impl StorageNodeEndpoint {
    /// Create a new storage node endpoint with default TTL
    pub fn new(url: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Self {
            url,
            last_verified: now,
            latency_ms: 0,
            ttl_secs: DEFAULT_STORAGE_NODE_TTL_SECS,
        }
    }
    
    /// Check if this endpoint has expired (FR-516)
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
    
    /// Update latency measurement
    pub fn update_latency(&mut self, latency_ms: u32) {
        self.latency_ms = latency_ms;
        self.refresh();
    }
}

/// Cached entry with local metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    endpoint: StorageNodeEndpoint,
    #[allow(dead_code)] // For potential future TTL-based eviction
    added_at: Instant,
    health_checked: bool,
}

/// Storage node endpoint cache with TTL-based expiration (FR-515, FR-516)
pub struct StorageNodeCache {
    /// URL -> CacheEntry
    entries: RwLock<HashMap<String, CacheEntry>>,
}

impl StorageNodeCache {
    /// Create a new storage node cache
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }
    
    /// Add or update a storage node endpoint in the cache
    ///
    /// Returns true if the endpoint was added/updated
    pub async fn insert(&self, endpoint: StorageNodeEndpoint) -> bool {
        // (#31-H-5) Validate URL to prevent SSRF — `health_check()` actually
        // fetches `{url}/health` server-side, so an untrusted gossip peer
        // could otherwise probe internal services (Redis, metadata endpoints,
        // etc.) by gossipping crafted URLs.
        if let Err(reason) = validate_endpoint_url(&endpoint.url) {
            warn!(
                "Rejecting storage node endpoint URL {}: {}",
                endpoint.url, reason
            );
            return false;
        }

        let entry = CacheEntry {
            endpoint: endpoint.clone(),
            added_at: Instant::now(),
            health_checked: false,
        };
        
        let mut entries = self.entries.write().await;
        let url_key = endpoint.url.clone();
        entries.insert(url_key.clone(), entry);
        
        debug!("Added storage node endpoint to cache: {}", url_key);
        true
    }
    
    /// Mark endpoint as health-checked
    pub async fn mark_health_checked(&self, url: &str) {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(url) {
            entry.health_checked = true;
            entry.endpoint.refresh();
            debug!("Marked storage node endpoint as health-checked: {}", url);
        }
    }
    
    /// Get an endpoint by URL
    pub async fn get(&self, url: &str) -> Option<StorageNodeEndpoint> {
        let entries = self.entries.read().await;
        entries.get(url).map(|e| e.endpoint.clone())
    }
    
    /// Get all non-expired endpoints
    pub async fn get_all(&self) -> Vec<StorageNodeEndpoint> {
        let entries = self.entries.read().await;
        entries
            .values()
            .filter(|e| !e.endpoint.is_expired())
            .map(|e| e.endpoint.clone())
            .collect()
    }
    
    /// Get all non-expired, health-checked endpoints sorted by latency
    pub async fn get_healthy_by_latency(&self) -> Vec<StorageNodeEndpoint> {
        let entries = self.entries.read().await;
        let mut endpoints: Vec<_> = entries
            .values()
            .filter(|e| !e.endpoint.is_expired() && e.health_checked)
            .map(|e| e.endpoint.clone())
            .collect();
        
        endpoints.sort_by_key(|e| e.latency_ms);
        endpoints
    }
    
    /// Remove an endpoint from the cache
    pub async fn remove(&self, url: &str) {
        let mut entries = self.entries.write().await;
        entries.remove(url);
        debug!("Removed storage node endpoint from cache: {}", url);
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
                debug!("GC removing expired storage node endpoint: {}", url);
            }
            keep
        });
        
        let removed = before - entries.len();
        if removed > 0 {
            debug!("Storage node cache GC removed {} entries", removed);
        }
        removed
    }
    
    /// Get the number of cached entries
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }
    
    /// Check if cache is empty
    pub async fn is_empty(&self) -> bool {
        self.entries.read().await.is_empty()
    }
    
    /// Clear all entries
    pub async fn clear(&self) {
        self.entries.write().await.clear();
        debug!("Storage node cache cleared");
    }
}

impl Default for StorageNodeCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Health check timeout (FR-518)
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// Perform health check on a storage node endpoint (FR-518)
/// 
/// Calls the /health endpoint and returns the latency in milliseconds if successful.
/// Returns None if the health check fails.
pub async fn health_check(url: &str) -> Option<u32> {
    use std::time::Instant;
    
    let health_url = format!("{}/health", url.trim_end_matches('/'));
    let start = Instant::now();
    
    let client = match reqwest::Client::builder()
        .timeout(HEALTH_CHECK_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!(url = url, error = %e, "Failed to create HTTP client for health check");
            return None;
        }
    };
    
    match client.get(&health_url).send().await {
        Ok(response) if response.status().is_success() => {
            let latency_ms = start.elapsed().as_millis() as u32;
            debug!(url = url, latency_ms = latency_ms, "Health check passed");
            Some(latency_ms)
        }
        Ok(response) => {
            debug!(url = url, status = %response.status(), "Health check failed: non-success status");
            None
        }
        Err(e) => {
            debug!(url = url, error = %e, "Health check failed: request error");
            None
        }
    }
}

/// Verify a list of storage node endpoints via health checks (FR-518)
/// 
/// Returns only the endpoints that pass the health check, with updated latency.
pub async fn verify_storage_nodes(nodes: Vec<StorageNodeEndpoint>) -> Vec<StorageNodeEndpoint> {
    let mut verified = Vec::with_capacity(nodes.len());
    
    for mut node in nodes {
        if let Some(latency_ms) = health_check(&node.url).await {
            node.update_latency(latency_ms);
            verified.push(node);
        }
    }
    
    verified
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn make_test_endpoint(url: &str) -> StorageNodeEndpoint {
        StorageNodeEndpoint {
            url: url.to_string(),
            last_verified: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            latency_ms: 50,
            ttl_secs: 300,
        }
    }
    
    // T137: Test StorageNodeCache TTL expiration
    #[tokio::test]
    async fn t137_storage_node_cache_ttl_expiration() {
        let cache = StorageNodeCache::new();
        
        // Create an already-expired endpoint
        let mut expired = make_test_endpoint("http://expired:3030");
        expired.last_verified = 0; // Very old timestamp
        expired.ttl_secs = 1; // 1 second TTL
        
        // Create a valid endpoint
        let valid = make_test_endpoint("http://valid:3030");
        
        cache.insert(expired).await;
        cache.insert(valid).await;
        
        assert_eq!(cache.len().await, 2);
        
        // GC should remove the expired one
        let removed = cache.gc().await;
        assert_eq!(removed, 1);
        assert_eq!(cache.len().await, 1);
        
        // Valid endpoint should remain
        assert!(cache.get("http://valid:3030").await.is_some());
        assert!(cache.get("http://expired:3030").await.is_none());
    }
    
    #[tokio::test]
    async fn test_storage_node_endpoint_creation() {
        let endpoint = StorageNodeEndpoint::new("http://test-node:3030".to_string());
        
        assert_eq!(endpoint.url, "http://test-node:3030");
        assert_eq!(endpoint.ttl_secs, DEFAULT_STORAGE_NODE_TTL_SECS);
        assert!(!endpoint.is_expired());
    }
    
    #[tokio::test]
    async fn test_cache_insert_and_get() {
        let cache = StorageNodeCache::new();
        let endpoint = make_test_endpoint("http://test-node:3030");
        
        assert!(cache.insert(endpoint.clone()).await);
        
        let retrieved = cache.get("http://test-node:3030").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().url, "http://test-node:3030");
    }
    
    #[tokio::test]
    async fn test_cache_rejects_invalid_url() {
        let cache = StorageNodeCache::new();
        let invalid = StorageNodeEndpoint {
            url: "not-a-valid-url".to_string(),
            last_verified: 0,
            latency_ms: 0,
            ttl_secs: 300,
        };

        assert!(!cache.insert(invalid).await);
        assert_eq!(cache.len().await, 0);
    }

    /// IPv6 リテラルはブラケット付き host 表記でも確実に拒否されること
    /// （host_str() 経由の旧実装はブラケットで IP 判定をすり抜けていた）。
    #[tokio::test]
    async fn test_cache_rejects_ipv6_loopback_and_private() {
        let cache = StorageNodeCache::new();
        for url in [
            "http://[::1]:3030",          // IPv6 loopback
            "http://[fe80::1]:3030",      // IPv6 link-local
            "http://[fc00::1]:3030",      // IPv6 ULA
            "http://127.0.0.1:3030",      // IPv4 loopback
            "http://192.168.0.5:3030",    // IPv4 private
        ] {
            assert!(
                !cache.insert(make_test_endpoint(url)).await,
                "should reject {}",
                url
            );
        }
        assert!(cache.is_empty().await);
    }
    
    #[tokio::test]
    async fn test_cache_health_check() {
        let cache = StorageNodeCache::new();
        let endpoint = make_test_endpoint("http://test-node:3030");
        
        cache.insert(endpoint).await;
        cache.mark_health_checked("http://test-node:3030").await;
        
        let healthy = cache.get_healthy_by_latency().await;
        assert_eq!(healthy.len(), 1);
    }
    
    #[tokio::test]
    async fn test_get_all_sorted_by_latency() {
        let cache = StorageNodeCache::new();
        
        let mut slow = make_test_endpoint("http://slow:3030");
        slow.latency_ms = 200;
        
        let mut fast = make_test_endpoint("http://fast:3030");
        fast.latency_ms = 10;
        
        let mut medium = make_test_endpoint("http://medium:3030");
        medium.latency_ms = 100;
        
        cache.insert(slow).await;
        cache.insert(fast).await;
        cache.insert(medium).await;
        
        // Mark all as health-checked
        cache.mark_health_checked("http://slow:3030").await;
        cache.mark_health_checked("http://fast:3030").await;
        cache.mark_health_checked("http://medium:3030").await;
        
        let sorted = cache.get_healthy_by_latency().await;
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].url, "http://fast:3030");
        assert_eq!(sorted[1].url, "http://medium:3030");
        assert_eq!(sorted[2].url, "http://slow:3030");
    }
}

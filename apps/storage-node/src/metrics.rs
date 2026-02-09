//! Metrics for storage node observability
//!
//! Provides basic metrics for monitoring storage node health and performance.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Metrics container for storage node
#[derive(Debug, Clone)]
pub struct Metrics {
    inner: Arc<MetricsInner>,
}

#[derive(Debug)]
struct MetricsInner {
    /// Number of fragments stored
    fragment_count: AtomicU64,
    /// Total bytes used for fragment storage
    capacity_used_bytes: AtomicU64,
    /// Total capacity in bytes
    capacity_total_bytes: AtomicU64,
    /// Number of PUT requests handled
    put_requests: AtomicU64,
    /// Number of GET requests handled
    get_requests: AtomicU64,
    /// Number of connected peers
    connected_peers: AtomicU64,
    /// Number of declare_holding calls made
    declare_holding_calls: AtomicU64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                fragment_count: AtomicU64::new(0),
                capacity_used_bytes: AtomicU64::new(0),
                capacity_total_bytes: AtomicU64::new(0),
                put_requests: AtomicU64::new(0),
                get_requests: AtomicU64::new(0),
                connected_peers: AtomicU64::new(0),
                declare_holding_calls: AtomicU64::new(0),
            }),
        }
    }

    /// Set total capacity
    pub fn set_capacity_total(&self, bytes: u64) {
        self.inner.capacity_total_bytes.store(bytes, Ordering::Relaxed);
    }

    /// Update fragment count and used capacity
    pub fn set_storage_stats(&self, fragment_count: u64, used_bytes: u64) {
        self.inner.fragment_count.store(fragment_count, Ordering::Relaxed);
        self.inner.capacity_used_bytes.store(used_bytes, Ordering::Relaxed);
    }

    /// Increment fragment count by delta
    pub fn inc_fragment_count(&self, delta: u64) {
        self.inner.fragment_count.fetch_add(delta, Ordering::Relaxed);
    }

    /// Add to used capacity
    pub fn add_capacity_used(&self, bytes: u64) {
        self.inner.capacity_used_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record a PUT request
    pub fn record_put(&self) {
        self.inner.put_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a GET request
    pub fn record_get(&self) {
        self.inner.get_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Update connected peers count
    pub fn set_connected_peers(&self, count: u64) {
        self.inner.connected_peers.store(count, Ordering::Relaxed);
    }

    /// Increment connected peers
    pub fn inc_connected_peers(&self) {
        self.inner.connected_peers.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement connected peers
    pub fn dec_connected_peers(&self) {
        self.inner.connected_peers.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record a declare_holding call
    pub fn record_declare_holding(&self) {
        self.inner.declare_holding_calls.fetch_add(1, Ordering::Relaxed);
    }

    // === Getters ===

    /// Get fragment count
    pub fn fragment_count(&self) -> u64 {
        self.inner.fragment_count.load(Ordering::Relaxed)
    }

    /// Get used capacity in bytes
    pub fn capacity_used_bytes(&self) -> u64 {
        self.inner.capacity_used_bytes.load(Ordering::Relaxed)
    }

    /// Get total capacity in bytes
    pub fn capacity_total_bytes(&self) -> u64 {
        self.inner.capacity_total_bytes.load(Ordering::Relaxed)
    }

    /// Get PUT request count
    pub fn put_requests(&self) -> u64 {
        self.inner.put_requests.load(Ordering::Relaxed)
    }

    /// Get GET request count
    pub fn get_requests(&self) -> u64 {
        self.inner.get_requests.load(Ordering::Relaxed)
    }

    /// Get connected peers count
    pub fn connected_peers(&self) -> u64 {
        self.inner.connected_peers.load(Ordering::Relaxed)
    }

    /// Get declare_holding call count
    pub fn declare_holding_calls(&self) -> u64 {
        self.inner.declare_holding_calls.load(Ordering::Relaxed)
    }

    /// Get utilization percentage (0.0 - 100.0)
    pub fn utilization_percent(&self) -> f64 {
        let total = self.capacity_total_bytes();
        if total == 0 {
            return 0.0;
        }
        (self.capacity_used_bytes() as f64 / total as f64) * 100.0
    }

    /// Log current metrics at INFO level
    pub fn log_stats(&self) {
        tracing::info!(
            fragment_count = self.fragment_count(),
            capacity_used_bytes = self.capacity_used_bytes(),
            capacity_total_bytes = self.capacity_total_bytes(),
            utilization_percent = format!("{:.2}", self.utilization_percent()),
            put_requests = self.put_requests(),
            get_requests = self.get_requests(),
            connected_peers = self.connected_peers(),
            declare_holding_calls = self.declare_holding_calls(),
            "Storage node metrics"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_default() {
        let metrics = Metrics::new();
        assert_eq!(metrics.fragment_count(), 0);
        assert_eq!(metrics.capacity_used_bytes(), 0);
        assert_eq!(metrics.connected_peers(), 0);
    }

    #[test]
    fn test_metrics_storage_stats() {
        let metrics = Metrics::new();
        metrics.set_storage_stats(100, 1024 * 1024);
        
        assert_eq!(metrics.fragment_count(), 100);
        assert_eq!(metrics.capacity_used_bytes(), 1024 * 1024);
    }

    #[test]
    fn test_metrics_utilization() {
        let metrics = Metrics::new();
        metrics.set_capacity_total(1000);
        metrics.add_capacity_used(250);
        
        assert!((metrics.utilization_percent() - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_metrics_requests() {
        let metrics = Metrics::new();
        
        metrics.record_put();
        metrics.record_put();
        metrics.record_get();
        
        assert_eq!(metrics.put_requests(), 2);
        assert_eq!(metrics.get_requests(), 1);
    }

    #[test]
    fn test_metrics_connected_peers() {
        let metrics = Metrics::new();
        
        metrics.inc_connected_peers();
        metrics.inc_connected_peers();
        assert_eq!(metrics.connected_peers(), 2);
        
        metrics.dec_connected_peers();
        assert_eq!(metrics.connected_peers(), 1);
    }

    #[test]
    fn test_metrics_clone() {
        let metrics = Metrics::new();
        metrics.record_put();
        
        let cloned = metrics.clone();
        metrics.record_put();
        
        // Both point to the same Arc, so both see the update
        assert_eq!(cloned.put_requests(), 2);
    }
}

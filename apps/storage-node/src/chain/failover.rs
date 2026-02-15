//! Active-Standby failover for blockchain connections
//!
//! Implements automatic failover when the primary blockchain node fails (FR-510, FR-511).
//!
//! ## Architecture
//!
//! - Primary: Currently active connection
//! - Hot Standby: Pre-connected backup (handshake complete)
//! - Disconnected: Not currently connected
//!
//! ## Failover Trigger
//!
//! - Liveness check every 2 seconds
//! - Timeout: 2 seconds per check
//! - Failover after 3 consecutive failures (6 seconds total)

use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::super::network::endpoint_cache::BlockchainEndpoint;

/// Liveness check interval
pub const CHECK_INTERVAL: Duration = Duration::from_secs(2);

/// Liveness check timeout
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// Number of consecutive failures before failover
pub const FAILURE_THRESHOLD: u32 = 3;

/// Connection role in the failover system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionRole {
    /// Currently active primary connection
    Primary,
    /// Pre-connected backup ready for immediate promotion
    HotStandby,
    /// Not currently connected
    Disconnected,
}

/// State of a single connection
#[derive(Debug, Clone)]
pub struct ConnectionState {
    /// Endpoint information
    pub endpoint: BlockchainEndpoint,
    
    /// Current role
    pub role: ConnectionRole,
    
    /// Consecutive liveness check failures
    pub failure_count: u32,
    
    /// Time of last successful communication
    pub last_success: Option<Instant>,
    
    /// Is currently attempting to connect
    pub connecting: bool,
}

impl ConnectionState {
    /// Create a new disconnected connection state
    pub fn new(endpoint: BlockchainEndpoint) -> Self {
        Self {
            endpoint,
            role: ConnectionRole::Disconnected,
            failure_count: 0,
            last_success: None,
            connecting: false,
        }
    }
    
    /// Record a successful liveness check
    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.last_success = Some(Instant::now());
    }
    
    /// Record a failed liveness check
    ///
    /// Returns true if failover should be triggered
    pub fn record_failure(&mut self) -> bool {
        self.failure_count = self.failure_count.saturating_add(1);
        self.failure_count >= FAILURE_THRESHOLD
    }
    
    /// Check if this connection has exceeded the failure threshold
    pub fn should_failover(&self) -> bool {
        self.failure_count >= FAILURE_THRESHOLD
    }
    
    /// Promote this connection to Primary
    pub fn promote(&mut self) {
        self.role = ConnectionRole::Primary;
        self.failure_count = 0;
        info!(endpoint = %self.endpoint.url, "Connection promoted to Primary");
    }
    
    /// Demote this connection to Disconnected
    pub fn demote(&mut self) {
        self.role = ConnectionRole::Disconnected;
        warn!(endpoint = %self.endpoint.url, "Connection demoted to Disconnected");
    }
}

/// Failover manager for blockchain connections
pub struct FailoverManager {
    /// Primary connection (if any)
    primary: RwLock<Option<ConnectionState>>,
    
    /// Hot standby connections
    standbys: RwLock<Vec<ConnectionState>>,
}

impl FailoverManager {
    /// Create a new failover manager
    pub fn new() -> Self {
        Self {
            primary: RwLock::new(None),
            standbys: RwLock::new(Vec::new()),
        }
    }
    
    /// Set the primary connection
    pub async fn set_primary(&self, endpoint: BlockchainEndpoint) {
        let mut state = ConnectionState::new(endpoint);
        state.role = ConnectionRole::Primary;
        state.last_success = Some(Instant::now());
        
        let mut primary = self.primary.write().await;
        *primary = Some(state);
    }
    
    /// Add a hot standby connection
    pub async fn add_standby(&self, endpoint: BlockchainEndpoint) {
        let mut state = ConnectionState::new(endpoint);
        state.role = ConnectionRole::HotStandby;
        
        let mut standbys = self.standbys.write().await;
        standbys.push(state);
    }
    
    /// Get the current primary endpoint URL
    pub async fn get_primary_url(&self) -> Option<String> {
        let primary = self.primary.read().await;
        primary.as_ref().map(|s| s.endpoint.url.clone())
    }
    
    /// Record a successful liveness check for the primary
    pub async fn record_primary_success(&self) {
        let mut primary = self.primary.write().await;
        if let Some(ref mut state) = *primary {
            state.record_success();
        }
    }
    
    /// Record a failed liveness check for the primary
    ///
    /// Returns Some(new_primary_url) if failover occurred
    pub async fn record_primary_failure(&self) -> Option<String> {
        let mut primary = self.primary.write().await;
        let mut standbys = self.standbys.write().await;
        
        if let Some(ref mut state) = *primary {
            if state.record_failure() {
                // Trigger failover
                info!(
                    endpoint = %state.endpoint.url,
                    failures = state.failure_count,
                    "Primary connection failed, initiating failover"
                );
                
                // Find best standby to promote
                if let Some(idx) = standbys
                    .iter()
                    .position(|s| s.role == ConnectionRole::HotStandby)
                {
                    let mut new_primary = standbys.remove(idx);
                    new_primary.promote();
                    
                    // Move old primary to standbys as disconnected
                    let mut old_primary = primary.take().unwrap();
                    old_primary.demote();
                    standbys.push(old_primary);
                    
                    let url = new_primary.endpoint.url.clone();
                    *primary = Some(new_primary);
                    
                    return Some(url);
                } else {
                    error!("No hot standby available for failover!");
                }
            }
        }
        
        None
    }
    
    /// Get the number of hot standbys
    pub async fn standby_count(&self) -> usize {
        let standbys = self.standbys.read().await;
        standbys
            .iter()
            .filter(|s| s.role == ConnectionRole::HotStandby)
            .count()
    }
    
    /// Check if there's a primary connection
    pub async fn has_primary(&self) -> bool {
        let primary = self.primary.read().await;
        primary.is_some()
    }
}

impl Default for FailoverManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_endpoint(url: &str) -> BlockchainEndpoint {
        BlockchainEndpoint {
            url: url.to_string(),
            chain_id: [0u8; 32],
            last_verified: 0,
            latency_ms: 100,
            ttl_secs: 300,
        }
    }

    #[test]
    fn test_connection_state_new() {
        let endpoint = make_endpoint("ws://test:9944");
        let state = ConnectionState::new(endpoint);
        
        assert_eq!(state.role, ConnectionRole::Disconnected);
        assert_eq!(state.failure_count, 0);
    }

    #[test]
    fn test_record_success() {
        let endpoint = make_endpoint("ws://test:9944");
        let mut state = ConnectionState::new(endpoint);
        
        state.failure_count = 2;
        state.record_success();
        
        assert_eq!(state.failure_count, 0);
        assert!(state.last_success.is_some());
    }

    #[test]
    fn test_record_failure_threshold() {
        let endpoint = make_endpoint("ws://test:9944");
        let mut state = ConnectionState::new(endpoint);
        
        assert!(!state.record_failure()); // 1
        assert!(!state.record_failure()); // 2
        assert!(state.record_failure());  // 3 - threshold reached
        assert!(state.should_failover());
    }

    #[tokio::test]
    async fn test_failover_manager_basic() {
        let manager = FailoverManager::new();
        
        assert!(!manager.has_primary().await);
        
        manager.set_primary(make_endpoint("ws://primary:9944")).await;
        assert!(manager.has_primary().await);
        
        let url = manager.get_primary_url().await;
        assert_eq!(url, Some("ws://primary:9944".to_string()));
    }

    #[tokio::test]
    async fn test_failover_with_standby() {
        let manager = FailoverManager::new();
        
        manager.set_primary(make_endpoint("ws://primary:9944")).await;
        manager.add_standby(make_endpoint("ws://standby:9944")).await;
        
        // Simulate 3 failures
        assert!(manager.record_primary_failure().await.is_none());
        assert!(manager.record_primary_failure().await.is_none());
        
        // Third failure should trigger failover
        let new_primary = manager.record_primary_failure().await;
        assert_eq!(new_primary, Some("ws://standby:9944".to_string()));
        
        // Verify new primary
        let url = manager.get_primary_url().await;
        assert_eq!(url, Some("ws://standby:9944".to_string()));
    }

    // T084: Test failover state transitions
    #[tokio::test]
    async fn t084_failover_state_transitions() {
        let manager = FailoverManager::new();
        
        // Initial state: no primary
        assert!(!manager.has_primary().await);
        assert_eq!(manager.standby_count().await, 0);
        
        // Add primary
        manager.set_primary(make_endpoint("ws://node1:9944")).await;
        assert!(manager.has_primary().await);
        
        // Add standbys
        manager.add_standby(make_endpoint("ws://node2:9944")).await;
        manager.add_standby(make_endpoint("ws://node3:9944")).await;
        assert_eq!(manager.standby_count().await, 2);
        
        // Success resets failure count
        manager.record_primary_success().await;
        
        // First failover: node1 -> node2
        for _ in 0..FAILURE_THRESHOLD {
            manager.record_primary_failure().await;
        }
        assert_eq!(manager.get_primary_url().await, Some("ws://node2:9944".to_string()));
        
        // After failover: old primary is demoted, standby count decreased
        assert_eq!(manager.standby_count().await, 1); // node3 only
        
        // Second failover: node2 -> node3
        for _ in 0..FAILURE_THRESHOLD {
            manager.record_primary_failure().await;
        }
        assert_eq!(manager.get_primary_url().await, Some("ws://node3:9944".to_string()));
        
        // No more standbys available
        assert_eq!(manager.standby_count().await, 0);
    }

    // T085: Test liveness check timeout behavior
    #[test]
    fn t085_liveness_check_timeout() {
        // Verify constants are set correctly
        assert_eq!(CHECK_INTERVAL, std::time::Duration::from_secs(2));
        assert_eq!(CHECK_TIMEOUT, std::time::Duration::from_secs(2));
        assert_eq!(FAILURE_THRESHOLD, 3);
        
        // Total time to failover = 3 checks × 2s timeout = 6 seconds max
        let max_failover_time = FAILURE_THRESHOLD * CHECK_TIMEOUT.as_secs() as u32;
        assert_eq!(max_failover_time, 6);
        
        // Test failure counting
        let mut state = ConnectionState::new(make_endpoint("ws://test:9944"));
        
        // Initially no failures
        assert!(!state.should_failover());
        
        // After 2 failures, still not failed over
        state.record_failure();
        state.record_failure();
        assert!(!state.should_failover());
        
        // After 3rd failure, should failover
        state.record_failure();
        assert!(state.should_failover());
        assert_eq!(state.failure_count, FAILURE_THRESHOLD);
    }

    // Test no failover without standby
    #[tokio::test]
    async fn test_no_failover_without_standby() {
        let manager = FailoverManager::new();
        manager.set_primary(make_endpoint("ws://lonely:9944")).await;
        
        // No standby available
        assert_eq!(manager.standby_count().await, 0);
        
        // Failures should not result in failover (no standby to promote)
        for _ in 0..5 {
            let result = manager.record_primary_failure().await;
            assert!(result.is_none());
        }
        
        // Primary should remain the same (even though it's failing)
        assert_eq!(manager.get_primary_url().await, Some("ws://lonely:9944".to_string()));
    }
}

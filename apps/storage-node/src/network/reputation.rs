//! Peer reputation tracking system
//!
//! Tracks reputation scores for peers based on message validity (FR-513).
//! Peers with low scores are ignored to prevent malicious information spread.
//!
//! ## Scoring
//!
//! - Initial score: 100
//! - Valid message: +1
//! - Invalid message: -20
//! - Ignore threshold: 50 (messages from peers below this are dropped)

use std::collections::HashMap;
use std::time::Instant;
use libp2p::PeerId;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Initial reputation score for new peers
pub const INITIAL_SCORE: i32 = 100;

/// Penalty for sending invalid information
pub const INVALID_PENALTY: i32 = -20;

/// Reward for sending valid information
pub const VALID_REWARD: i32 = 1;

/// Score threshold below which peers are ignored
pub const IGNORE_THRESHOLD: i32 = 50;

/// Maximum score cap
pub const MAX_SCORE: i32 = 100;

/// Reputation information for a single peer
#[derive(Debug, Clone)]
pub struct PeerReputation {
    /// Peer identifier
    pub peer_id: PeerId,
    
    /// Current reputation score
    pub score: i32,
    
    /// Last update timestamp
    pub last_updated: Instant,
    
    /// Count of invalid messages received
    pub invalid_count: u32,
    
    /// Count of valid messages received
    pub valid_count: u32,
}

impl PeerReputation {
    /// Create a new reputation entry with initial score
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            score: INITIAL_SCORE,
            last_updated: Instant::now(),
            invalid_count: 0,
            valid_count: 0,
        }
    }
    
    /// Check if this peer should be ignored
    pub fn is_ignored(&self) -> bool {
        self.score <= IGNORE_THRESHOLD
    }
    
    /// Apply a penalty for invalid message
    pub fn penalize(&mut self) {
        self.score = self.score.saturating_add(INVALID_PENALTY);
        self.invalid_count = self.invalid_count.saturating_add(1);
        self.last_updated = Instant::now();
        
        if self.is_ignored() {
            warn!(
                peer = %self.peer_id,
                score = self.score,
                "Peer is now ignored due to low reputation"
            );
        }
    }
    
    /// Apply a reward for valid message
    pub fn reward(&mut self) {
        self.score = self.score.saturating_add(VALID_REWARD).min(MAX_SCORE);
        self.valid_count = self.valid_count.saturating_add(1);
        self.last_updated = Instant::now();
    }
}

/// Reputation manager for all known peers
pub struct ReputationManager {
    /// PeerId -> Reputation
    reputations: RwLock<HashMap<PeerId, PeerReputation>>,
}

impl ReputationManager {
    /// Create a new reputation manager
    pub fn new() -> Self {
        Self {
            reputations: RwLock::new(HashMap::new()),
        }
    }
    
    /// Get or create reputation entry for a peer
    pub async fn get_or_create(&self, peer_id: PeerId) -> PeerReputation {
        let reputations = self.reputations.read().await;
        if let Some(rep) = reputations.get(&peer_id) {
            return rep.clone();
        }
        drop(reputations);
        
        let mut reputations = self.reputations.write().await;
        reputations
            .entry(peer_id)
            .or_insert_with(|| PeerReputation::new(peer_id))
            .clone()
    }
    
    /// Check if a peer should be ignored
    pub async fn should_ignore(&self, peer_id: &PeerId) -> bool {
        let reputations = self.reputations.read().await;
        reputations
            .get(peer_id)
            .map(|r| r.is_ignored())
            .unwrap_or(false)
    }
    
    /// Record a valid message from a peer
    pub async fn record_valid(&self, peer_id: PeerId) {
        let mut reputations = self.reputations.write().await;
        let rep = reputations
            .entry(peer_id)
            .or_insert_with(|| PeerReputation::new(peer_id));
        rep.reward();
        
        debug!(
            peer = %peer_id,
            score = rep.score,
            "Recorded valid message from peer"
        );
    }
    
    /// Record an invalid message from a peer
    pub async fn record_invalid(&self, peer_id: PeerId) {
        let mut reputations = self.reputations.write().await;
        let rep = reputations
            .entry(peer_id)
            .or_insert_with(|| PeerReputation::new(peer_id));
        rep.penalize();
        
        warn!(
            peer = %peer_id,
            score = rep.score,
            invalid_count = rep.invalid_count,
            "Recorded invalid message from peer"
        );
    }
    
    /// Get the current score for a peer
    pub async fn get_score(&self, peer_id: &PeerId) -> Option<i32> {
        let reputations = self.reputations.read().await;
        reputations.get(peer_id).map(|r| r.score)
    }
    
    /// Get all peers currently being tracked
    pub async fn get_all(&self) -> Vec<PeerReputation> {
        let reputations = self.reputations.read().await;
        reputations.values().cloned().collect()
    }
    
    /// Remove old entries that haven't been updated recently
    pub async fn cleanup(&self, max_age: std::time::Duration) -> usize {
        let mut reputations = self.reputations.write().await;
        let before = reputations.len();
        
        reputations.retain(|_, rep| rep.last_updated.elapsed() < max_age);
        
        before - reputations.len()
    }
}

impl Default for ReputationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity::Keypair;

    fn make_peer_id() -> PeerId {
        Keypair::generate_ed25519().public().to_peer_id()
    }

    #[test]
    fn test_new_reputation() {
        let peer_id = make_peer_id();
        let rep = PeerReputation::new(peer_id);
        
        assert_eq!(rep.score, INITIAL_SCORE);
        assert!(!rep.is_ignored());
    }

    #[test]
    fn test_penalize() {
        let peer_id = make_peer_id();
        let mut rep = PeerReputation::new(peer_id);
        
        rep.penalize();
        assert_eq!(rep.score, 80);
        assert_eq!(rep.invalid_count, 1);
        assert!(!rep.is_ignored());
    }

    #[test]
    fn test_ignore_after_penalties() {
        let peer_id = make_peer_id();
        let mut rep = PeerReputation::new(peer_id);
        
        // 3 penalties: 100 - 60 = 40 (below threshold)
        rep.penalize();
        rep.penalize();
        rep.penalize();
        
        assert!(rep.is_ignored());
        assert_eq!(rep.invalid_count, 3);
    }

    #[test]
    fn test_reward_capped() {
        let peer_id = make_peer_id();
        let mut rep = PeerReputation::new(peer_id);
        
        // Should stay at 100
        rep.reward();
        rep.reward();
        
        assert_eq!(rep.score, MAX_SCORE);
    }

    #[tokio::test]
    async fn test_manager_record_valid() {
        let manager = ReputationManager::new();
        let peer_id = make_peer_id();
        
        manager.record_valid(peer_id).await;
        
        let score = manager.get_score(&peer_id).await;
        assert_eq!(score, Some(INITIAL_SCORE)); // Stays at 100 (capped)
    }

    #[tokio::test]
    async fn test_manager_should_ignore() {
        let manager = ReputationManager::new();
        let peer_id = make_peer_id();
        
        // 3 invalid messages
        manager.record_invalid(peer_id).await;
        manager.record_invalid(peer_id).await;
        manager.record_invalid(peer_id).await;
        
        assert!(manager.should_ignore(&peer_id).await);
    }
}

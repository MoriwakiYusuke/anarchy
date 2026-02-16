//! Garbage Collection Module
//!
//! Score-based GC for forgetting candidates with grace period (T057-T058).
//! Also supports reward-pool-based GC: when pool is depleted, data can be deleted.
//!
//! Implements:
//! - FR-203: Score-based GC logic
//! - FR-204: 7-day grace period before GC
//! - FR-XXX: Reward pool depletion GC

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Grace period before garbage collection (7 days in seconds)
pub const GC_GRACE_PERIOD_SECS: u64 = 7 * 24 * 60 * 60; // 604800 seconds

/// Development mode grace period (10 minutes for testing)
pub const GC_GRACE_PERIOD_DEV_SECS: u64 = 10 * 60;

/// Reward pool balance threshold for GC (1 MORAL = 10^12 units)
/// When pool balance falls below this, nodes MAY delete data
pub const GC_REWARD_POOL_THRESHOLD: u128 = 1_000_000_000_000; // 1 MORAL

/// Reward pool check interval (5 minutes)
pub const GC_POOL_CHECK_INTERVAL_SECS: u64 = 5 * 60;

/// GC candidate entry
#[derive(Debug, Clone)]
pub struct GcCandidate {
    /// Content hash
    pub content_hash: [u8; 32],
    /// When the content became a forgetting candidate
    pub marked_at: Instant,
    /// Current score (cached)
    pub score: u64,
}

/// Garbage collector for storage node
pub struct GarbageCollector {
    /// Pending GC candidates
    candidates: HashMap<[u8; 32], GcCandidate>,
    /// Grace period duration
    grace_period: Duration,
    /// Last known reward pool balance
    last_pool_balance: Option<u128>,
    /// Whether pool-based GC is active (pool below threshold)
    pool_gc_active: bool,
    /// Reward pool check interval
    pool_check_interval: Duration,
    /// Last pool check time
    last_pool_check: Option<Instant>,
}

impl GarbageCollector {
    /// Create new garbage collector
    pub fn new(dev_mode: bool) -> Self {
        let grace_period = if dev_mode {
            Duration::from_secs(GC_GRACE_PERIOD_DEV_SECS)
        } else {
            Duration::from_secs(GC_GRACE_PERIOD_SECS)
        };

        Self {
            candidates: HashMap::new(),
            grace_period,
            last_pool_balance: None,
            pool_gc_active: false,
            pool_check_interval: Duration::from_secs(GC_POOL_CHECK_INTERVAL_SECS),
            last_pool_check: None,
        }
    }

    /// Check if reward pool should be rechecked
    pub fn should_check_pool(&self) -> bool {
        match self.last_pool_check {
            None => true,
            Some(last) => Instant::now().duration_since(last) >= self.pool_check_interval,
        }
    }

    /// Update reward pool balance and determine GC activation
    ///
    /// Returns true if GC mode changed (activated or deactivated)
    pub fn update_pool_balance(&mut self, balance: u128) -> bool {
        self.last_pool_check = Some(Instant::now());
        self.last_pool_balance = Some(balance);
        
        let was_active = self.pool_gc_active;
        self.pool_gc_active = balance < GC_REWARD_POOL_THRESHOLD;
        
        if self.pool_gc_active && !was_active {
            warn!(
                balance = balance,
                threshold = GC_REWARD_POOL_THRESHOLD,
                "GC: Reward pool depleted! Nodes may delete physical data."
            );
        } else if !self.pool_gc_active && was_active {
            info!(
                balance = balance,
                threshold = GC_REWARD_POOL_THRESHOLD,
                "GC: Reward pool recovered. GC deactivated."
            );
        }
        
        was_active != self.pool_gc_active
    }

    /// Check if pool-based GC is currently active
    pub fn is_pool_gc_active(&self) -> bool {
        self.pool_gc_active
    }

    /// Get last known pool balance
    pub fn last_pool_balance(&self) -> Option<u128> {
        self.last_pool_balance
    }

    /// Mark content as forgetting candidate (called when on-chain event received)
    pub fn mark_forgetting_candidate(&mut self, content_hash: [u8; 32], score: u64) {
        if !self.candidates.contains_key(&content_hash) {
            info!(
                "GC: Marking content {:?} as forgetting candidate (score: {})",
                hex::encode(&content_hash[..8]),
                score
            );
            
            self.candidates.insert(content_hash, GcCandidate {
                content_hash,
                marked_at: Instant::now(),
                score,
            });
        }
    }

    /// Remove content from forgetting candidates (score recovered)
    pub fn unmark_forgetting_candidate(&mut self, content_hash: &[u8; 32]) {
        if self.candidates.remove(content_hash).is_some() {
            info!(
                "GC: Content {:?} score recovered, removing from GC candidates",
                hex::encode(&content_hash[..8])
            );
        }
    }

    /// Get list of content ready for garbage collection (grace period expired)
    pub fn get_gc_ready(&self) -> Vec<[u8; 32]> {
        let now = Instant::now();
        
        self.candidates
            .iter()
            .filter(|(_, candidate)| {
                now.duration_since(candidate.marked_at) >= self.grace_period
            })
            .map(|(hash, _)| *hash)
            .collect()
    }

    /// Execute garbage collection for given content
    ///
    /// Returns true if GC was performed
    pub fn execute_gc(&mut self, content_hash: &[u8; 32]) -> bool {
        if let Some(candidate) = self.candidates.get(content_hash) {
            let elapsed = Instant::now().duration_since(candidate.marked_at);
            
            if elapsed >= self.grace_period {
                info!(
                    "GC: Executing garbage collection for {:?} (grace period expired: {:?})",
                    hex::encode(&content_hash[..8]),
                    elapsed
                );
                
                self.candidates.remove(content_hash);
                return true;
            } else {
                debug!(
                    "GC: Content {:?} not ready for GC (remaining: {:?})",
                    hex::encode(&content_hash[..8]),
                    self.grace_period - elapsed
                );
            }
        }
        
        false
    }

    /// Get number of pending GC candidates
    pub fn pending_count(&self) -> usize {
        self.candidates.len()
    }

    /// Get all pending candidates (for status reporting)
    pub fn get_all_candidates(&self) -> Vec<&GcCandidate> {
        self.candidates.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_mark_forgetting_candidate() {
        let mut gc = GarbageCollector::new(true);
        let content_hash = [1u8; 32];

        gc.mark_forgetting_candidate(content_hash, 50);
        
        assert_eq!(gc.pending_count(), 1);
        assert!(gc.candidates.contains_key(&content_hash));
    }

    #[test]
    fn test_unmark_forgetting_candidate() {
        let mut gc = GarbageCollector::new(true);
        let content_hash = [1u8; 32];

        gc.mark_forgetting_candidate(content_hash, 50);
        gc.unmark_forgetting_candidate(&content_hash);
        
        assert_eq!(gc.pending_count(), 0);
    }

    #[test]
    fn test_gc_ready_respects_grace_period() {
        let mut gc = GarbageCollector::new(true);
        gc.grace_period = Duration::from_millis(10); // Short for testing
        
        let content_hash = [1u8; 32];
        gc.mark_forgetting_candidate(content_hash, 50);

        // Not ready immediately
        assert!(gc.get_gc_ready().is_empty());

        // Wait for grace period
        thread::sleep(Duration::from_millis(15));

        // Now ready
        let ready = gc.get_gc_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], content_hash);
    }

    #[test]
    fn test_execute_gc() {
        let mut gc = GarbageCollector::new(true);
        gc.grace_period = Duration::from_millis(10);
        
        let content_hash = [1u8; 32];
        gc.mark_forgetting_candidate(content_hash, 50);

        // Not ready immediately
        assert!(!gc.execute_gc(&content_hash));

        // Wait for grace period
        thread::sleep(Duration::from_millis(15));

        // Now executes
        assert!(gc.execute_gc(&content_hash));
        assert_eq!(gc.pending_count(), 0);
    }
}

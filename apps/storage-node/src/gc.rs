//! Garbage Collection Module
//!
//! Score-based GC for forgetting candidates with grace period (T057-T058).
//!
//! Implements:
//! - FR-203: Score-based GC logic
//! - FR-204: 7-day grace period before GC

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Grace period before garbage collection (7 days in seconds)
pub const GC_GRACE_PERIOD_SECS: u64 = 7 * 24 * 60 * 60; // 604800 seconds

/// Development mode grace period (10 minutes for testing)
pub const GC_GRACE_PERIOD_DEV_SECS: u64 = 10 * 60;

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
    /// Whether in development mode
    dev_mode: bool,
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
            dev_mode,
        }
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

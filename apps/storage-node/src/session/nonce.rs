//! Nonce cache for replay attack prevention
//!
//! Maintains a cache of recently used nonces to prevent replay attacks.
//! Nonces are expired after 5 minutes (matching timestamp drift + buffer).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;

/// How long to keep nonces in the cache (5 minutes)
const NONCE_TTL: Duration = Duration::from_secs(300);

/// Maximum number of nonces to keep (prevent memory exhaustion)
const MAX_NONCES: usize = 10_000;

/// Cache of used nonces to prevent replay attacks
#[derive(Clone)]
pub struct NonceCache {
    inner: Arc<RwLock<NonceCacheInner>>,
}

struct NonceCacheInner {
    /// nonce -> expiry time
    nonces: HashMap<String, Instant>,
    /// cleanup counter (cleanup every N insertions)
    insert_counter: usize,
}

impl NonceCache {
    /// Create a new empty nonce cache
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(NonceCacheInner {
                nonces: HashMap::new(),
                insert_counter: 0,
            })),
        }
    }

    /// Check if a nonce has been used and mark it as used if not.
    /// Returns true if the nonce was already used (replay attack detected).
    /// Returns false if the nonce is fresh (OK to proceed).
    pub fn check_and_mark(&self, nonce: &str) -> bool {
        let mut inner = self.inner.write();
        let now = Instant::now();

        // Periodic cleanup (every 100 insertions)
        inner.insert_counter += 1;
        if inner.insert_counter >= 100 {
            inner.insert_counter = 0;
            inner.nonces.retain(|_, &mut expiry| expiry > now);
        }

        // Check if nonce exists and hasn't expired
        if let Some(&expiry) = inner.nonces.get(nonce) {
            if expiry > now {
                // Nonce is still valid = replay attack
                return true;
            }
        }

        // Enforce size limit by removing oldest entries if needed
        if inner.nonces.len() >= MAX_NONCES {
            // Remove 10% of oldest entries
            let mut entries: Vec<_> = inner.nonces.iter()
                .map(|(k, &v)| (k.clone(), v))
                .collect();
            entries.sort_by_key(|(_, expiry)| *expiry);
            let remove_count = MAX_NONCES / 10;
            for (key, _) in entries.into_iter().take(remove_count) {
                inner.nonces.remove(&key);
            }
        }

        // Mark nonce as used
        inner.nonces.insert(nonce.to_string(), now + NONCE_TTL);
        false
    }

    /// Check if a nonce has been used (without marking)
    pub fn is_used(&self, nonce: &str) -> bool {
        let inner = self.inner.read();
        let now = Instant::now();
        
        if let Some(&expiry) = inner.nonces.get(nonce) {
            expiry > now
        } else {
            false
        }
    }

    /// Get the number of cached nonces
    pub fn len(&self) -> usize {
        self.inner.read().nonces.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.inner.read().nonces.is_empty()
    }

    /// Clear all cached nonces
    pub fn clear(&self) {
        self.inner.write().nonces.clear();
    }
}

impl Default for NonceCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fresh_nonce() {
        let cache = NonceCache::new();
        assert!(!cache.check_and_mark("nonce1"));
        assert!(!cache.is_empty());
    }

    #[test]
    fn test_replay_detection() {
        let cache = NonceCache::new();
        assert!(!cache.check_and_mark("nonce1"));
        // Same nonce should be rejected
        assert!(cache.check_and_mark("nonce1"));
    }

    #[test]
    fn test_different_nonces() {
        let cache = NonceCache::new();
        assert!(!cache.check_and_mark("nonce1"));
        assert!(!cache.check_and_mark("nonce2"));
        assert!(!cache.check_and_mark("nonce3"));
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn test_is_used() {
        let cache = NonceCache::new();
        assert!(!cache.is_used("nonce1"));
        cache.check_and_mark("nonce1");
        assert!(cache.is_used("nonce1"));
    }

    #[test]
    fn test_clear() {
        let cache = NonceCache::new();
        cache.check_and_mark("nonce1");
        cache.check_and_mark("nonce2");
        assert_eq!(cache.len(), 2);
        cache.clear();
        assert!(cache.is_empty());
    }
}

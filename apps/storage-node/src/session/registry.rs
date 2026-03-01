//! Session registry for managing active sessions

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use libp2p::PeerId;
use parking_lot::RwLock;
use tracing::{debug, info};

use super::token::{SessionToken, SessionInfo};

/// Default session TTL: 24 hours
pub const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(86400);

/// Default idle timeout: 1 hour
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(3600);

/// Default cleanup interval: 5 minutes (for future background cleanup task)
#[allow(dead_code)]
pub const DEFAULT_CLEANUP_INTERVAL: Duration = Duration::from_secs(300);

/// Session registry for managing token-to-session mappings
#[derive(Clone)]
pub struct SessionRegistry {
    inner: Arc<SessionRegistryInner>,
}

struct SessionRegistryInner {
    /// Token -> SessionInfo mapping
    sessions: RwLock<HashMap<SessionToken, SessionInfo>>,
    /// PeerId -> Token mapping (for revoking existing sessions on re-auth)
    peer_tokens: RwLock<HashMap<PeerId, SessionToken>>,
    /// Token TTL
    ttl: Duration,
    /// Idle timeout
    idle_timeout: Duration,
}

impl SessionRegistry {
    /// Create a new session registry with default settings
    pub fn new() -> Self {
        Self::with_config(DEFAULT_SESSION_TTL, DEFAULT_IDLE_TIMEOUT)
    }

    /// Create a new session registry with custom settings
    pub fn with_config(ttl: Duration, idle_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(SessionRegistryInner {
                sessions: RwLock::new(HashMap::new()),
                peer_tokens: RwLock::new(HashMap::new()),
                ttl,
                idle_timeout,
            }),
        }
    }

    /// Create a new session for a peer, revoking any existing session
    pub fn create_session(&self, peer_id: PeerId) -> SessionToken {
        // Revoke existing session for this peer
        self.revoke_for_peer(&peer_id);

        // Generate new token
        let token = SessionToken::generate();
        let info = SessionInfo::new(peer_id, self.inner.ttl);

        // Store the session
        {
            let mut sessions = self.inner.sessions.write();
            sessions.insert(token.clone(), info);
        }
        {
            let mut peer_tokens = self.inner.peer_tokens.write();
            peer_tokens.insert(peer_id, token.clone());
        }

        debug!(
            peer_id = %peer_id,
            token = %token,
            "Created new session"
        );

        token
    }

    /// Validate token and return the associated peer_id if valid
    /// Also updates the last_access timestamp
    pub fn validate(&self, token: &str) -> Option<PeerId> {
        let token = SessionToken::from_hex(token)?;

        let mut sessions = self.inner.sessions.write();
        let info = sessions.get_mut(&token)?;

        // Check expiry
        if info.is_expired() {
            drop(sessions);
            self.revoke_token(&token);
            return None;
        }

        // Check idle timeout
        if info.is_idle(self.inner.idle_timeout) {
            drop(sessions);
            self.revoke_token(&token);
            return None;
        }

        // Update last access
        info.touch();

        Some(info.peer_id)
    }

    /// Renew a session, returning a new token
    /// Only allowed within 1 hour of expiry
    pub fn renew_session(&self, old_token: &str) -> Option<SessionToken> {
        let old_token = SessionToken::from_hex(old_token)?;

        let sessions = self.inner.sessions.read();
        let info = sessions.get(&old_token)?;

        // Check if renewal is allowed
        if !info.can_renew() {
            return None;
        }

        let peer_id = info.peer_id;
        drop(sessions);

        // Create new session (this revokes the old one)
        Some(self.create_session(peer_id))
    }

    /// Revoke a specific token
    pub fn revoke_token(&self, token: &SessionToken) {
        let mut sessions = self.inner.sessions.write();
        if let Some(info) = sessions.remove(token) {
            let mut peer_tokens = self.inner.peer_tokens.write();
            peer_tokens.remove(&info.peer_id);
            debug!(
                peer_id = %info.peer_id,
                "Revoked session"
            );
        }
    }

    /// Revoke a session by token string (hex)
    /// Returns true if the token was found and revoked, false otherwise
    pub fn revoke_by_token(&self, token_str: &str) -> bool {
        if let Some(token) = SessionToken::from_hex(token_str) {
            let sessions = self.inner.sessions.read();
            if sessions.contains_key(&token) {
                drop(sessions);
                self.revoke_token(&token);
                return true;
            }
        }
        false
    }

    /// Revoke all sessions for a peer
    pub fn revoke_for_peer(&self, peer_id: &PeerId) {
        let peer_tokens = self.inner.peer_tokens.read();
        if let Some(token) = peer_tokens.get(peer_id).cloned() {
            drop(peer_tokens);
            self.revoke_token(&token);
        }
    }

    /// Clean up expired and idle sessions
    pub fn cleanup_expired(&self) {
        let mut sessions = self.inner.sessions.write();
        let mut peer_tokens = self.inner.peer_tokens.write();

        let expired: Vec<_> = sessions
            .iter()
            .filter(|(_, info)| {
                info.is_expired() || info.is_idle(self.inner.idle_timeout)
            })
            .map(|(token, info)| (token.clone(), info.peer_id))
            .collect();

        for (token, peer_id) in expired {
            sessions.remove(&token);
            peer_tokens.remove(&peer_id);
            debug!(peer_id = %peer_id, "Cleaned up expired session");
        }

        if !sessions.is_empty() {
            info!(
                active_sessions = sessions.len(),
                "Session cleanup completed"
            );
        }
    }

    /// Get the number of active sessions
    pub fn session_count(&self) -> usize {
        self.inner.sessions.read().len()
    }

    /// Get the configured TTL
    pub fn ttl(&self) -> Duration {
        self.inner.ttl
    }

    /// Get session expiration time for a token (Unix timestamp)
    pub fn get_expiration(&self, token: &str) -> Option<u64> {
        let token = SessionToken::from_hex(token)?;
        let sessions = self.inner.sessions.read();
        let info = sessions.get(&token)?;

        // Convert Instant to approximate Unix timestamp
        let remaining = info.expires_at.saturating_duration_since(std::time::Instant::now());
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Some(now_unix + remaining.as_secs())
    }

    /// Spawn a background cleanup task (018-storage-node-auth T012)
    /// 
    /// This spawns a tokio task that periodically calls cleanup_expired().
    /// The task runs indefinitely until the runtime is dropped.
    /// 
    /// # Arguments
    /// * `interval` - How often to run cleanup (e.g., 5 minutes)
    /// 
    /// # Returns
    /// A JoinHandle that can be used to abort the task if needed.
    pub fn spawn_cleanup_task(
        &self,
        interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        let registry = self.clone();
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval_timer.tick().await;
                registry.cleanup_expired();
            }
        })
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_validate_session() {
        let registry = SessionRegistry::new();
        let peer_id = PeerId::random();

        // Create session
        let token = registry.create_session(peer_id);

        // Should validate successfully
        let validated_peer = registry.validate(token.as_str());
        assert_eq!(validated_peer, Some(peer_id));

        // Invalid token should fail
        let invalid = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(registry.validate(invalid).is_none());
    }

    #[test]
    fn test_session_revocation() {
        let registry = SessionRegistry::new();
        let peer_id = PeerId::random();

        // Create session
        let token = registry.create_session(peer_id);
        assert!(registry.validate(token.as_str()).is_some());

        // Revoke
        registry.revoke_for_peer(&peer_id);
        assert!(registry.validate(token.as_str()).is_none());
    }

    #[test]
    fn test_session_replacement() {
        let registry = SessionRegistry::new();
        let peer_id = PeerId::random();

        // Create first session
        let token1 = registry.create_session(peer_id);

        // Create second session (should revoke first)
        let token2 = registry.create_session(peer_id);

        // First token should be invalid
        assert!(registry.validate(token1.as_str()).is_none());

        // Second token should be valid
        assert!(registry.validate(token2.as_str()).is_some());
    }

    #[test]
    fn test_session_count() {
        let registry = SessionRegistry::new();

        assert_eq!(registry.session_count(), 0);

        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        registry.create_session(peer1);
        assert_eq!(registry.session_count(), 1);

        registry.create_session(peer2);
        assert_eq!(registry.session_count(), 2);

        registry.revoke_for_peer(&peer1);
        assert_eq!(registry.session_count(), 1);
    }
}

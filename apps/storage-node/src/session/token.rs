//! Session token types

use std::time::Instant;
use libp2p::PeerId;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// 256-bit session token (hex-encoded string, 64 characters)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionToken(String);

impl SessionToken {
    /// Generate a new cryptographically secure random token
    pub fn generate() -> Self {
        let bytes: [u8; 32] = rand::rngs::OsRng.gen();
        Self(hex::encode(bytes))
    }

    /// Create from hex string (for parsing from headers)
    pub fn from_hex(s: &str) -> Option<Self> {
        // Validate: must be exactly 64 hex characters
        if s.len() != 64 {
            return None;
        }
        if !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self(s.to_lowercase()))
    }

    /// Get the hex string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for SessionToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Session information associated with a token
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Associated peer ID
    pub peer_id: PeerId,
    /// Token issuance timestamp
    pub issued_at: Instant,
    /// Token expiration timestamp
    pub expires_at: Instant,
    /// Last access timestamp (for idle timeout)
    pub last_access: Instant,
}

impl SessionInfo {
    /// Create new session info with default TTL (24 hours)
    pub fn new(peer_id: PeerId, ttl: std::time::Duration) -> Self {
        let now = Instant::now();
        Self {
            peer_id,
            issued_at: now,
            expires_at: now + ttl,
            last_access: now,
        }
    }

    /// Check if the session has expired
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    /// Check if the session is idle (no access for specified duration)
    pub fn is_idle(&self, idle_timeout: std::time::Duration) -> bool {
        Instant::now() - self.last_access > idle_timeout
    }

    /// Check if renewal is allowed (within 1 hour of expiry)
    pub fn can_renew(&self) -> bool {
        let remaining = self.expires_at.saturating_duration_since(Instant::now());
        remaining < std::time::Duration::from_secs(3600)
    }

    /// Update last access time
    pub fn touch(&mut self) {
        self.last_access = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation() {
        let token1 = SessionToken::generate();
        let token2 = SessionToken::generate();

        // Tokens should be 64 hex characters
        assert_eq!(token1.as_str().len(), 64);
        assert_eq!(token2.as_str().len(), 64);

        // Tokens should be unique
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_token_from_hex() {
        // Valid token
        let valid = "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2";
        assert!(SessionToken::from_hex(valid).is_some());

        // Invalid: too short
        assert!(SessionToken::from_hex("a1b2c3").is_none());

        // Invalid: non-hex characters
        assert!(SessionToken::from_hex("g1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2").is_none());
    }

    #[test]
    fn test_session_info_expiry() {
        use std::time::Duration;

        // Create session with 1 second TTL
        let peer_id = PeerId::random();
        let session = SessionInfo::new(peer_id, Duration::from_millis(50));

        // Should not be expired immediately
        assert!(!session.is_expired());

        // Wait and check expiry
        std::thread::sleep(Duration::from_millis(100));
        assert!(session.is_expired());
    }
}

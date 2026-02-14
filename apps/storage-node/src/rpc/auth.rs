//! HTTP request authentication middleware
//!
//! Implements signature-based authentication for write operations (FR-201-207).
//!
//! ## Authentication Flow
//!
//! 1. Client creates SignedRequest with timestamp, nonce, payload hash
//! 2. Client signs with Sr25519 key (via WebAuthn/Secure Enclave)
//! 3. Server validates: timestamp within 5 min, nonce not reused, signature valid
//! 4. On failure: 401 (missing auth) or 403 (invalid auth)

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use axum::http::{header::HeaderMap, StatusCode};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Signature validity period (5 minutes)
pub const SIGNATURE_VALIDITY_SECS: u64 = 300;

/// Nonce cache TTL (same as signature validity)
pub const NONCE_TTL_SECS: u64 = SIGNATURE_VALIDITY_SECS;

/// HTTP header name for authentication
pub const AUTH_HEADER: &str = "X-Anarchy-Auth";

/// Signed request structure (sent in X-Anarchy-Auth header as JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedRequest {
    /// Sr25519 public key (AccountId), hex-encoded 32 bytes
    pub account_id: String,
    
    /// Unix timestamp in seconds
    pub timestamp: u64,
    
    /// 128-bit random nonce, hex-encoded 16 bytes
    pub nonce: String,
    
    /// Blake2b hash of request body, hex-encoded 32 bytes
    pub payload_hash: String,
    
    /// Sr25519 signature, hex-encoded 64 bytes
    pub signature: String,
}

impl SignedRequest {
    /// Parse account_id from hex string
    pub fn get_account_id_bytes(&self) -> Result<[u8; 32], AuthError> {
        hex::decode(&self.account_id)
            .map_err(|_| AuthError::MalformedRequest)?
            .try_into()
            .map_err(|_| AuthError::MalformedRequest)
    }
    
    /// Parse nonce from hex string
    pub fn get_nonce_bytes(&self) -> Result<[u8; 16], AuthError> {
        hex::decode(&self.nonce)
            .map_err(|_| AuthError::MalformedRequest)?
            .try_into()
            .map_err(|_| AuthError::MalformedRequest)
    }
    
    /// Parse signature from hex string
    pub fn get_signature_bytes(&self) -> Result<[u8; 64], AuthError> {
        hex::decode(&self.signature)
            .map_err(|_| AuthError::MalformedRequest)?
            .try_into()
            .map_err(|_| AuthError::MalformedRequest)
    }
    
    /// Parse payload hash from hex string
    pub fn get_payload_hash_bytes(&self) -> Result<[u8; 32], AuthError> {
        hex::decode(&self.payload_hash)
            .map_err(|_| AuthError::MalformedRequest)?
            .try_into()
            .map_err(|_| AuthError::MalformedRequest)
    }
}

/// Authentication error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    /// Missing authentication header
    MissingAuth,
    /// Malformed request structure
    MalformedRequest,
    /// Invalid signature
    InvalidSignature,
    /// Timestamp expired (older than 5 minutes)
    ExpiredTimestamp,
    /// Nonce already used (replay attack)
    NonceReused,
    /// Payload hash mismatch
    PayloadHashMismatch,
}

impl AuthError {
    /// Get HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            AuthError::MissingAuth => StatusCode::UNAUTHORIZED,
            _ => StatusCode::FORBIDDEN,
        }
    }
    
    /// Get error message
    pub fn message(&self) -> &'static str {
        match self {
            AuthError::MissingAuth => "Authentication required",
            AuthError::MalformedRequest => "Malformed authentication request",
            AuthError::InvalidSignature => "Invalid signature",
            AuthError::ExpiredTimestamp => "Timestamp expired",
            AuthError::NonceReused => "Nonce already used",
            AuthError::PayloadHashMismatch => "Payload hash mismatch",
        }
    }
}

/// Nonce cache entry
struct NonceCacheEntry {
    /// When this nonce was first seen
    inserted_at: u64,
}

/// Cache for used nonces to prevent replay attacks
pub struct NonceCache {
    /// nonce hex string -> entry
    cache: RwLock<HashMap<String, NonceCacheEntry>>,
    /// TTL in seconds
    ttl_secs: u64,
}

impl NonceCache {
    /// Create a new nonce cache
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            ttl_secs,
        }
    }
    
    /// Check if nonce is fresh and add it to the cache
    ///
    /// Returns true if nonce is fresh (not seen before)
    pub fn check_and_insert(&self, nonce: &str) -> bool {
        let now = current_timestamp();
        
        let mut cache = self.cache.write();
        
        // Check if nonce exists and is still valid
        if let Some(entry) = cache.get(nonce) {
            if now.saturating_sub(entry.inserted_at) <= self.ttl_secs {
                return false; // Nonce was recently used
            }
        }
        
        // Insert new nonce
        cache.insert(nonce.to_string(), NonceCacheEntry { inserted_at: now });
        true
    }
    
    /// Run garbage collection (remove expired nonces)
    pub fn gc(&self) {
        let now = current_timestamp();
        let mut cache = self.cache.write();
        
        cache.retain(|_, entry| {
            now.saturating_sub(entry.inserted_at) <= self.ttl_secs
        });
    }
    
    /// Get the number of cached nonces
    pub fn len(&self) -> usize {
        self.cache.read().len()
    }
}

impl Default for NonceCache {
    fn default() -> Self {
        Self::new(NONCE_TTL_SECS)
    }
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Validate a signed request
pub fn validate_request(
    request: &SignedRequest,
    expected_payload_hash: &[u8; 32],
    nonce_cache: &NonceCache,
) -> Result<(), AuthError> {
    // 1. Check timestamp (within 5 minutes)
    let now = current_timestamp();
    let request_time = request.timestamp;
    
    if now.saturating_sub(request_time) > SIGNATURE_VALIDITY_SECS {
        return Err(AuthError::ExpiredTimestamp);
    }
    
    // Also reject future timestamps (more than 30 seconds ahead)
    if request_time.saturating_sub(now) > 30 {
        return Err(AuthError::ExpiredTimestamp);
    }
    
    // 2. Check nonce hasn't been used
    if !nonce_cache.check_and_insert(&request.nonce) {
        return Err(AuthError::NonceReused);
    }
    
    // 3. Verify payload hash matches
    let provided_hash = request.get_payload_hash_bytes()?;
    if &provided_hash != expected_payload_hash {
        return Err(AuthError::PayloadHashMismatch);
    }
    
    // 4. Verify signature
    // TODO: Implement actual Sr25519 verification in T043
    // For now, accept all signatures for scaffolding
    
    Ok(())
}

/// Parse authentication header from request
pub fn parse_auth_header(headers: &HeaderMap) -> Result<SignedRequest, AuthError> {
    let header_value = headers
        .get(AUTH_HEADER)
        .ok_or(AuthError::MissingAuth)?
        .to_str()
        .map_err(|_| AuthError::MalformedRequest)?;
    
    serde_json::from_str(header_value).map_err(|_| AuthError::MalformedRequest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonce_cache_fresh() {
        let cache = NonceCache::new(300);
        
        // First use should succeed
        assert!(cache.check_and_insert("nonce1"));
        
        // Second use should fail
        assert!(!cache.check_and_insert("nonce1"));
        
        // Different nonce should succeed
        assert!(cache.check_and_insert("nonce2"));
    }

    #[test]
    fn test_auth_error_status_codes() {
        assert_eq!(AuthError::MissingAuth.status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(AuthError::InvalidSignature.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(AuthError::ExpiredTimestamp.status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_parse_signed_request() {
        let json = r#"{
            "account_id": "0000000000000000000000000000000000000000000000000000000000000000",
            "timestamp": 1234567890,
            "nonce": "00000000000000000000000000000000",
            "payload_hash": "0000000000000000000000000000000000000000000000000000000000000000",
            "signature": "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
        }"#;
        
        let request: SignedRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.timestamp, 1234567890);
    }

    #[test]
    fn test_get_bytes() {
        let request = SignedRequest {
            account_id: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            timestamp: 0,
            nonce: "00000000000000000000000000000000".to_string(),
            payload_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            signature: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".to_string(),
        };
        
        assert!(request.get_account_id_bytes().is_ok());
        assert!(request.get_nonce_bytes().is_ok());
        assert!(request.get_payload_hash_bytes().is_ok());
        assert!(request.get_signature_bytes().is_ok());
    }
}

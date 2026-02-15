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
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{header::HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use blake2::{Blake2b, Digest};
use blake2::digest::consts::U32;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use schnorrkel::{PublicKey, Signature, signing_context};

/// Maximum request body size for auth hashing (512KB)
const MAX_AUTH_BODY_SIZE: usize = 512 * 1024;

/// Signature validity period (5 minutes)
pub const SIGNATURE_VALIDITY_SECS: u64 = 300;

/// Nonce cache TTL (same as signature validity)
pub const NONCE_TTL_SECS: u64 = SIGNATURE_VALIDITY_SECS;

/// HTTP header name for authentication
pub const AUTH_HEADER: &str = "X-Anarchy-Auth";

/// Sr25519 signing context - must match @polkadot/keyring default
const SIGNING_CONTEXT: &[u8] = b"substrate";

/// Signed request structure (sent in X-Anarchy-Auth header as JSON)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Strip optional "0x" or "0X" prefix from hex string
    fn strip_hex_prefix(s: &str) -> &str {
        s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s)
    }

    /// Parse account_id from hex string (with optional 0x prefix)
    pub fn get_account_id_bytes(&self) -> Result<[u8; 32], AuthError> {
        hex::decode(Self::strip_hex_prefix(&self.account_id))
            .map_err(|_| AuthError::MalformedRequest)?
            .try_into()
            .map_err(|_| AuthError::MalformedRequest)
    }
    
    /// Parse nonce from hex string (with optional 0x prefix)
    pub fn get_nonce_bytes(&self) -> Result<[u8; 16], AuthError> {
        hex::decode(Self::strip_hex_prefix(&self.nonce))
            .map_err(|_| AuthError::MalformedRequest)?
            .try_into()
            .map_err(|_| AuthError::MalformedRequest)
    }
    
    /// Parse signature from hex string (with optional 0x prefix)
    pub fn get_signature_bytes(&self) -> Result<[u8; 64], AuthError> {
        hex::decode(Self::strip_hex_prefix(&self.signature))
            .map_err(|_| AuthError::MalformedRequest)?
            .try_into()
            .map_err(|_| AuthError::MalformedRequest)
    }
    
    /// Parse payload hash from hex string (with optional 0x prefix)
    pub fn get_payload_hash_bytes(&self) -> Result<[u8; 32], AuthError> {
        hex::decode(Self::strip_hex_prefix(&self.payload_hash))
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
    
    /// Check if the nonce cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.read().is_empty()
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

/// Extract params from JSON-RPC request and compute Blake2b-256 hash
/// 
/// Frontend hashes only the params object (without auth field), not the full JSON-RPC wrapper.
/// This function extracts params, removes auth if present, and hashes the result.
fn extract_and_hash_params(body: &[u8]) -> Result<[u8; 32], AuthError> {
    // Parse JSON-RPC request
    let mut json: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| AuthError::InvalidSignature)?;
    
    // Extract params field
    let params = json.get_mut("params")
        .ok_or(AuthError::InvalidSignature)?;
    
    // Remove auth field if present (frontend doesn't include auth in hash)
    if let Some(obj) = params.as_object_mut() {
        obj.remove("auth");
    }
    
    // Serialize params back to JSON string (same format as frontend)
    let params_json = serde_json::to_string(params)
        .map_err(|_| AuthError::InvalidSignature)?;
    
    // Compute Blake2b-256 hash
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(params_json.as_bytes());
    Ok(hasher.finalize().into())
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
    
    // 3. Verify payload hash matches what client signed
    // The blockchain node now strips auth from body, so hash should match
    let provided_hash = request.get_payload_hash_bytes()?;
    if &provided_hash != expected_payload_hash {
        return Err(AuthError::PayloadHashMismatch);
    }
    
    // 4. Verify Sr25519 signature
    let public_key_bytes = request.get_account_id_bytes()?;
    let signature_bytes = request.get_signature_bytes()?;
    
    // Build the message to verify: account_id || timestamp || nonce || payload_hash
    // This matches the frontend signing format
    let mut message = Vec::new();
    message.extend_from_slice(&public_key_bytes);  // account_id (32 bytes)
    message.extend_from_slice(&request.timestamp.to_le_bytes());
    message.extend_from_slice(&request.get_nonce_bytes()?);
    message.extend_from_slice(&provided_hash);
    
    if !verify_sr25519_signature(&public_key_bytes, &message, &signature_bytes) {
        return Err(AuthError::InvalidSignature);
    }
    
    Ok(())
}

/// Verify Sr25519 signature using schnorrkel
fn verify_sr25519_signature(
    public_key_bytes: &[u8; 32],
    message: &[u8],
    signature_bytes: &[u8; 64],
) -> bool {
    // Parse public key
    let public_key = match PublicKey::from_bytes(public_key_bytes) {
        Ok(pk) => pk,
        Err(_) => return false,
    };
    
    // Parse signature
    let signature = match Signature::from_bytes(signature_bytes) {
        Ok(sig) => sig,
        Err(_) => return false,
    };
    
    // Create signing context
    let ctx = signing_context(SIGNING_CONTEXT);
    
    // Verify signature
    public_key.verify(ctx.bytes(message), &signature).is_ok()
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

/// Shared state for authentication middleware
#[derive(Clone)]
pub struct AuthState {
    /// Nonce cache for replay prevention
    pub nonce_cache: Arc<NonceCache>,
    /// Whether authentication is enabled
    pub enabled: bool,
}

impl AuthState {
    /// Create new auth state
    pub fn new(enabled: bool) -> Self {
        Self {
            nonce_cache: Arc::new(NonceCache::default()),
            enabled,
        }
    }
    
    /// Run periodic garbage collection on nonce cache
    pub fn gc(&self) {
        self.nonce_cache.gc();
    }
}

/// Authentication middleware for axum
///
/// Validates X-Anarchy-Auth header on requests.
/// Returns 401 for missing auth, 403 for invalid auth.
///
/// Note: This middleware validates auth when present, but does not require it.
/// Method-specific auth requirements should be checked by the handler.
pub async fn auth_middleware(
    State(auth_state): State<AuthState>,
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Response {
    // If auth is disabled globally, pass through
    if !auth_state.enabled {
        return next.run(request).await;
    }
    
    // Try to parse auth header - if not present, let the request through
    // The handler will decide if auth is required for the specific method
    let auth_header = headers.get(AUTH_HEADER);
    if auth_header.is_none() {
        // No auth header - let handler decide if this is OK
        return next.run(request).await;
    }
    
    // Auth header is present - validate it
    let signed_request = match parse_auth_header(&headers) {
        Ok(sr) => sr,
        Err(e) => {
            return (e.status_code(), e.message()).into_response();
        }
    };
    
    // Extract and buffer the request body to compute actual hash
    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, MAX_AUTH_BODY_SIZE).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large").into_response();
        }
    };
    
    // Compute Blake2b-256 hash of the params field only (not the full JSON-RPC wrapper)
    // Frontend hashes only the params object, not { jsonrpc, id, method, params }
    let actual_payload_hash: [u8; 32] = match extract_and_hash_params(&body_bytes) {
        Ok(hash) => hash,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, "Invalid JSON-RPC request").into_response();
        }
    };
    
    // Validate the request against actual body hash
    if let Err(e) = validate_request(&signed_request, &actual_payload_hash, &auth_state.nonce_cache) {
        return (e.status_code(), e.message()).into_response();
    }
    
    // Reconstruct request with buffered body
    let request = Request::from_parts(parts, Body::from(body_bytes));
    
    // Auth valid - continue with the request
    next.run(request).await
}

/// Require authentication for the request
/// Call this in handlers that need auth
pub fn require_auth(headers: &HeaderMap, auth_enabled: bool) -> Result<(), (StatusCode, &'static str)> {
    if !auth_enabled {
        return Ok(());
    }
    
    if headers.get(AUTH_HEADER).is_none() {
        return Err((StatusCode::UNAUTHORIZED, "Authentication required"));
    }
    
    // If we got here, auth header was already validated by middleware
    Ok(())
}

/// Check if a method requires authentication
pub fn method_requires_auth(method: &str) -> bool {
    // Only write operations require auth
    // Read operations (get_fragment) are public
    matches!(method, "storage_storeFragment" | "storage_deleteFragment")
}

#[cfg(test)]
mod tests {
    use super::*;
    use schnorrkel::Keypair;

    /// Create a valid signed request for testing
    fn create_test_signed_request(keypair: &Keypair, payload_hash: &[u8; 32], timestamp: u64) -> SignedRequest {
        let nonce = [0u8; 16]; // Simple nonce for testing
        
        // Build message: account_id || timestamp || nonce || payload_hash
        let mut message = Vec::new();
        message.extend_from_slice(&keypair.public.to_bytes()); // account_id
        message.extend_from_slice(&timestamp.to_le_bytes());
        message.extend_from_slice(&nonce);
        message.extend_from_slice(payload_hash);
        
        // Sign with schnorrkel
        let ctx = signing_context(SIGNING_CONTEXT);
        let signature = keypair.sign(ctx.bytes(&message));
        
        SignedRequest {
            account_id: hex::encode(keypair.public.to_bytes()),
            timestamp,
            nonce: hex::encode(&nonce),
            payload_hash: hex::encode(payload_hash),
            signature: hex::encode(signature.to_bytes()),
        }
    }

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

    // T052: Test valid signature acceptance
    #[test]
    fn t052_valid_signature_accepted() {
        let keypair = Keypair::generate();
        let payload_hash = [0xab; 32];
        let now = current_timestamp();
        
        let request = create_test_signed_request(&keypair, &payload_hash, now);
        let cache = NonceCache::new(300);
        
        let result = validate_request(&request, &payload_hash, &cache);
        assert!(result.is_ok(), "Valid signature should be accepted: {:?}", result);
    }

    // T053: Test invalid signature rejection (403)
    #[test]
    fn t053_invalid_signature_rejected() {
        let keypair = Keypair::generate();
        let payload_hash = [0xab; 32];
        let now = current_timestamp();
        
        let mut request = create_test_signed_request(&keypair, &payload_hash, now);
        // Corrupt the signature
        request.signature = "ff".repeat(64);
        
        let cache = NonceCache::new(300);
        
        let result = validate_request(&request, &payload_hash, &cache);
        assert_eq!(result, Err(AuthError::InvalidSignature));
        assert_eq!(AuthError::InvalidSignature.status_code(), StatusCode::FORBIDDEN);
    }

    // T054: Test missing signature rejection (401)
    #[test]
    fn t054_missing_signature_rejected() {
        let headers = HeaderMap::new();
        
        let result = parse_auth_header(&headers);
        assert_eq!(result, Err(AuthError::MissingAuth));
        assert_eq!(AuthError::MissingAuth.status_code(), StatusCode::UNAUTHORIZED);
    }

    // T055: Test expired timestamp rejection
    #[test]
    fn t055_expired_timestamp_rejected() {
        let keypair = Keypair::generate();
        let payload_hash = [0xab; 32];
        // 10 minutes ago = expired
        let old_timestamp = current_timestamp().saturating_sub(600);
        
        let request = create_test_signed_request(&keypair, &payload_hash, old_timestamp);
        let cache = NonceCache::new(300);
        
        let result = validate_request(&request, &payload_hash, &cache);
        assert_eq!(result, Err(AuthError::ExpiredTimestamp));
    }

    // T056: Test replay attack prevention (duplicate nonce)
    #[test]
    fn t056_replay_attack_prevented() {
        let keypair = Keypair::generate();
        let payload_hash = [0xab; 32];
        let now = current_timestamp();
        
        let request = create_test_signed_request(&keypair, &payload_hash, now);
        let cache = NonceCache::new(300);
        
        // First request should succeed
        let result1 = validate_request(&request, &payload_hash, &cache);
        assert!(result1.is_ok());
        
        // Create new request with same nonce (replay attempt)
        let mut replay_request = create_test_signed_request(&keypair, &payload_hash, now);
        replay_request.nonce = request.nonce.clone(); // Same nonce
        
        let result2 = validate_request(&replay_request, &payload_hash, &cache);
        assert_eq!(result2, Err(AuthError::NonceReused));
    }

    // Test that payload hash mismatch is detected
    #[test]
    fn test_payload_hash_mismatch() {
        let keypair = Keypair::generate();
        let payload_hash = [0xab; 32];
        let now = current_timestamp();
        
        let request = create_test_signed_request(&keypair, &payload_hash, now);
        let cache = NonceCache::new(300);
        
        // Different expected hash - should fail validation
        let wrong_hash = [0xcd; 32];
        let result = validate_request(&request, &wrong_hash, &cache);
        assert_eq!(result, Err(AuthError::PayloadHashMismatch));
    }

    // Test method_requires_auth
    #[test]
    fn test_method_requires_auth() {
        assert!(method_requires_auth("storage_storeFragment"));
        assert!(method_requires_auth("storage_deleteFragment"));
        assert!(!method_requires_auth("storage_getFragment"));
        assert!(!method_requires_auth("storage_health"));
    }

    // Test AuthState creation
    #[test]
    fn test_auth_state() {
        let state = AuthState::new(true);
        assert!(state.enabled);
        assert_eq!(state.nonce_cache.len(), 0);
        
        let state_disabled = AuthState::new(false);
        assert!(!state_disabled.enabled);
    }
}

#[cfg(test)]
mod json_format_tests {
    use super::*;
    
    #[test]
    fn test_json_key_order_sorted() {
        // Frontend now sorts keys alphabetically before JSON.stringify
        // This matches serde_json's default behavior
        let frontend_sorted = r#"{"data":"dGVzdA==","index":0,"merkle_root":[1,2,3],"proof":"cHJvb2Y=","total_leaves":5}"#;
        
        // Parse and re-serialize (serde_json sorts alphabetically)
        let parsed: serde_json::Value = serde_json::from_str(frontend_sorted).unwrap();
        let reserialized = serde_json::to_string(&parsed).unwrap();
        
        assert_eq!(frontend_sorted, reserialized, "Sorted JSON should match serde_json output");
    }
    
    #[test]
    fn test_extract_and_hash_params_matches_frontend() {
        // Simulate what frontend does:
        // 1. Create params object (without auth)
        // 2. Sort keys alphabetically
        // 3. JSON.stringify
        // 4. Blake2b-256 hash
        
        // Frontend's sorted params (keys in alphabetical order)
        let frontend_params = r#"{"data":"dGVzdA==","index":0,"merkle_root":[1,2,3],"proof":"cHJvb2Y=","total_leaves":5}"#;
        
        // Compute hash as frontend would
        let mut frontend_hasher = Blake2b::<U32>::new();
        frontend_hasher.update(frontend_params.as_bytes());
        let frontend_hash: [u8; 32] = frontend_hasher.finalize().into();
        
        // Simulate what storage node receives (JSON-RPC wrapper with params)
        // Note: auth might be present but extract_and_hash_params removes it
        let json_rpc_body = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": "storage_storeFragment",
            "params": {
                "merkle_root": [1,2,3],
                "index": 0,
                "data": "dGVzdA==",
                "proof": "cHJvb2Y=",
                "total_leaves": 5
            }
        }"#;
        
        // Extract and hash params as storage node does
        let storage_hash = extract_and_hash_params(json_rpc_body.as_bytes()).unwrap();
        
        assert_eq!(
            frontend_hash, storage_hash,
            "Frontend and storage node hashes must match\nFrontend params: {}\nFrontend hash: {:?}\nStorage hash: {:?}",
            frontend_params, frontend_hash, storage_hash
        );
    }
    
    #[test]
    fn test_extract_and_hash_params_removes_auth() {
        // Even if auth is present in params, it should be removed before hashing
        let params_without_auth = r#"{"data":"dGVzdA==","index":0,"merkle_root":[1,2,3],"proof":"cHJvb2Y=","total_leaves":5}"#;
        
        let mut expected_hasher = Blake2b::<U32>::new();
        expected_hasher.update(params_without_auth.as_bytes());
        let expected_hash: [u8; 32] = expected_hasher.finalize().into();
        
        let json_rpc_with_auth = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": "storage_storeFragment",
            "params": {
                "merkle_root": [1,2,3],
                "index": 0,
                "data": "dGVzdA==",
                "proof": "cHJvb2Y=",
                "total_leaves": 5,
                "auth": {
                    "account_id": "abc",
                    "timestamp": 12345,
                    "nonce": "def",
                    "payload_hash": "ghi",
                    "signature": "jkl"
                }
            }
        }"#;
        
        let actual_hash = extract_and_hash_params(json_rpc_with_auth.as_bytes()).unwrap();
        
        assert_eq!(expected_hash, actual_hash, "Auth should be stripped before hashing");
    }
}

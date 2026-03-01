//! Storage node session client
//!
//! Handles session token acquisition, renewal, and revocation for storage node access.
//! Uses Ed25519 signed HTTP requests to authenticate with storage nodes.

// Allow dead code for future API expansion and optional features
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use parking_lot::RwLock;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sp_core::{ed25519, Pair};
use log::{info, warn, debug};

/// Session token string (64 hex characters)
pub type SessionToken = String;

/// Minimum time before expiry to trigger renewal (1 hour)
const RENEWAL_THRESHOLD_SECS: u64 = 3600;

/// Session request retry interval
const RETRY_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum retries for session request
const MAX_RETRIES: u32 = 3;

/// Session request payload
#[derive(Debug, Clone, Serialize)]
pub struct SessionRequest {
    pub method: String,
    pub params: SessionRequestParams,
    pub id: u32,
}

/// Session request parameters
#[derive(Debug, Clone, Serialize)]
pub struct SessionRequestParams {
    pub public_key: String,
    pub timestamp: u64,
    pub nonce: String,
    pub signature: String,
}

/// Session renew/revoke parameters
#[derive(Debug, Clone, Serialize)]
pub struct SessionTokenParams {
    pub token: String,
}

/// Session response
#[derive(Debug, Clone, Deserialize)]
pub struct SessionResponse {
    pub jsonrpc: String,
    pub id: u32,
    #[serde(default)]
    pub result: Option<SessionResult>,
    #[serde(default)]
    pub error: Option<SessionRpcError>,
}

/// Session result
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SessionResult {
    Session {
        token: String,
        expires_at: u64,
    },
    Revoked {
        revoked: bool,
    },
}

/// Session RPC error
#[derive(Debug, Clone, Deserialize)]
pub struct SessionRpcError {
    pub code: i32,
    pub message: String,
}

/// Session information
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Session token
    pub token: SessionToken,
    /// Expiration timestamp (Unix seconds)
    pub expires_at: u64,
    /// Storage node endpoint URL
    pub endpoint: String,
}

impl SessionInfo {
    /// Check if the session needs renewal
    pub fn needs_renewal(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        self.expires_at.saturating_sub(now) <= RENEWAL_THRESHOLD_SECS
    }

    /// Check if the session is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        now >= self.expires_at
    }
}

/// Storage session client error
#[derive(Debug, Clone)]
pub enum SessionClientError {
    /// HTTP request failed
    RequestFailed(String),
    /// Session request rejected
    Rejected { code: i32, message: String },
    /// Invalid response format
    InvalidResponse(String),
    /// No session for the endpoint
    NoSession,
}

impl std::fmt::Display for SessionClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            Self::Rejected { code, message } => write!(f, "Session rejected: {} ({})", message, code),
            Self::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            Self::NoSession => write!(f, "No session for endpoint"),
        }
    }
}

impl std::error::Error for SessionClientError {}

/// Storage session client for managing sessions across multiple storage nodes
#[derive(Clone)]
pub struct StorageSessionClient {
    /// HTTP client
    client: reqwest::Client,
    /// Ed25519 keypair for signing requests
    keypair: Arc<ed25519::Pair>,
    /// Active sessions: endpoint URL -> SessionInfo
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    /// Request ID counter
    request_id: Arc<RwLock<u32>>,
}

impl StorageSessionClient {
    /// Create a new session client with the given keypair
    pub fn new(keypair: ed25519::Pair) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            keypair: Arc::new(keypair),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            request_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Get the next request ID
    fn next_request_id(&self) -> u32 {
        let mut id = self.request_id.write();
        let current = *id;
        *id = id.wrapping_add(1);
        current
    }

    /// Get the public key as hex string
    fn public_key_hex(&self) -> String {
        hex::encode(self.keypair.public().0)
    }

    /// Sign a session request message
    fn sign_message(&self, message: &str) -> String {
        let signature = self.keypair.sign(message.as_bytes());
        hex::encode(signature.0)
    }

    /// Generate a random nonce (16 bytes = 32 hex chars)
    fn generate_nonce() -> String {
        let nonce: [u8; 16] = rand::rngs::OsRng.gen();
        hex::encode(nonce)
    }

    /// Request a new session from a storage node
    pub async fn request_session(&self, endpoint: &str) -> Result<SessionInfo, SessionClientError> {
        let session_url = format!("{}/session", endpoint.trim_end_matches('/'));

        // Send request with retries - regenerate nonce/signature for each attempt
        let mut last_error = None;
        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                tokio::time::sleep(RETRY_INTERVAL).await;
            }

            // Generate fresh timestamp, nonce, and signature for EACH attempt
            // This prevents "Nonce already used" errors on retries
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let nonce = Self::generate_nonce();
            let message = format!("anarchy-session-request:{}:{}", timestamp, nonce);
            let signature = self.sign_message(&message);

            let request = SessionRequest {
                method: "storage_requestSession".to_string(),
                params: SessionRequestParams {
                    public_key: self.public_key_hex(),
                    timestamp,
                    nonce,
                    signature,
                },
                id: self.next_request_id(),
            };

            match self.send_session_request(&session_url, &request).await {
                Ok(info) => {
                    // Store the session
                    let mut sessions = self.sessions.write();
                    sessions.insert(endpoint.to_string(), info.clone());
                    info!(
                        "Session established with storage node: endpoint={}, expires_at={}",
                        endpoint, info.expires_at
                    );
                    return Ok(info);
                }
                Err(e) => {
                    warn!(
                        "Session request attempt failed: endpoint={}, attempt={}, error={}",
                        endpoint, attempt + 1, e
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or(SessionClientError::RequestFailed("Max retries exceeded".into())))
    }

    /// Send session request and parse response
    async fn send_session_request(
        &self,
        url: &str,
        request: &SessionRequest,
    ) -> Result<SessionInfo, SessionClientError> {
        let response = self.client
            .post(url)
            .json(request)
            .send()
            .await
            .map_err(|e| SessionClientError::RequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(SessionClientError::RequestFailed(format!(
                "HTTP {}", response.status()
            )));
        }

        let session_response: SessionResponse = response
            .json()
            .await
            .map_err(|e| SessionClientError::InvalidResponse(e.to_string()))?;

        if let Some(error) = session_response.error {
            return Err(SessionClientError::Rejected {
                code: error.code,
                message: error.message,
            });
        }

        match session_response.result {
            Some(SessionResult::Session { token, expires_at }) => {
                // Extract endpoint from URL
                let endpoint = url.trim_end_matches("/session").to_string();
                Ok(SessionInfo {
                    token,
                    expires_at,
                    endpoint,
                })
            }
            _ => Err(SessionClientError::InvalidResponse(
                "Expected session result".into(),
            )),
        }
    }

    /// Get session token for an endpoint, requesting one if needed
    pub async fn get_or_request_session(&self, endpoint: &str) -> Result<SessionToken, SessionClientError> {
        // Check existing session
        {
            let sessions = self.sessions.read();
            if let Some(info) = sessions.get(endpoint) {
                if !info.is_expired() {
                    return Ok(info.token.clone());
                }
            }
        }

        // Request new session
        let info = self.request_session(endpoint).await?;
        Ok(info.token)
    }

    /// Get session token for an endpoint (without requesting)
    pub fn get_session(&self, endpoint: &str) -> Option<SessionToken> {
        let sessions = self.sessions.read();
        sessions.get(endpoint).map(|info| info.token.clone())
    }

    /// Check if a session needs renewal
    pub fn needs_renewal(&self, endpoint: &str) -> bool {
        let sessions = self.sessions.read();
        sessions.get(endpoint).map(|info| info.needs_renewal()).unwrap_or(true)
    }

    /// Get all active sessions
    pub fn active_sessions(&self) -> Vec<(String, SessionInfo)> {
        let sessions = self.sessions.read();
        sessions
            .iter()
            .filter(|(_, info)| !info.is_expired())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Remove expired sessions
    pub fn cleanup_expired(&self) {
        let mut sessions = self.sessions.write();
        sessions.retain(|_, info| !info.is_expired());
    }

    /// Spawn background task for session renewal
    pub fn spawn_renewal_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let check_interval = Duration::from_secs(300); // Check every 5 minutes

            loop {
                tokio::time::sleep(check_interval).await;

                // Collect endpoints that need renewal
                let endpoints_to_renew: Vec<String> = {
                    let sessions = self.sessions.read();
                    sessions
                        .iter()
                        .filter(|(_, info)| info.needs_renewal() && !info.is_expired())
                        .map(|(endpoint, _)| endpoint.clone())
                        .collect()
                };

                // Renew sessions
                for endpoint in endpoints_to_renew {
                    debug!("Renewing session: endpoint={}", endpoint);
                    match self.request_session(&endpoint).await {
                        Ok(_) => {
                            info!("Session renewed successfully: endpoint={}", endpoint);
                        }
                        Err(e) => {
                            warn!("Session renewal failed: endpoint={}, error={}", endpoint, e);
                        }
                    }
                }

                // Cleanup expired sessions
                self.cleanup_expired();
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::Pair;

    #[test]
    fn test_session_info_expiry() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Not expired
        let info = SessionInfo {
            token: "test".to_string(),
            expires_at: now + 7200, // 2 hours
            endpoint: "http://localhost:3030".to_string(),
        };
        assert!(!info.is_expired());
        assert!(!info.needs_renewal());

        // Needs renewal (within 1 hour of expiry)
        let info = SessionInfo {
            token: "test".to_string(),
            expires_at: now + 1800, // 30 minutes
            endpoint: "http://localhost:3030".to_string(),
        };
        assert!(!info.is_expired());
        assert!(info.needs_renewal());

        // Expired
        let info = SessionInfo {
            token: "test".to_string(),
            expires_at: now - 100,
            endpoint: "http://localhost:3030".to_string(),
        };
        assert!(info.is_expired());
        assert!(info.needs_renewal());
    }

    #[test]
    fn test_session_client_creation() {
        let (keypair, _) = ed25519::Pair::generate();
        let client = StorageSessionClient::new(keypair);
        
        // Initial state should be empty
        assert_eq!(client.active_sessions().len(), 0);
    }

    #[test]
    fn test_public_key_hex() {
        let (keypair, _) = ed25519::Pair::generate();
        let client = StorageSessionClient::new(keypair);
        
        let hex = client.public_key_hex();
        assert_eq!(hex.len(), 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn test_signature_format() {
        let (keypair, _) = ed25519::Pair::generate();
        let client = StorageSessionClient::new(keypair);
        
        let message = "anarchy-session-request:1234567890";
        let signature = client.sign_message(message);
        assert_eq!(signature.len(), 128); // 64 bytes = 128 hex chars
    }
}

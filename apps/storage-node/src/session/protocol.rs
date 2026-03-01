//! Session protocol for libp2p request-response
//!
//! Protocol ID: `/anarchy/session/1.0.0`
//!
//! This protocol handles session token requests from blockchain nodes.
//! Only peers that are connected via libp2p can request session tokens.

use std::time::{SystemTime, UNIX_EPOCH};
use ed25519_dalek::{Signature, VerifyingKey};
use futures::prelude::*;
use libp2p::{
    request_response::Codec,
    PeerId,
    identity::ed25519::PublicKey as Ed25519PublicKey,
};
use serde::{Deserialize, Serialize};

use super::error::SessionError;

/// Protocol ID for session authentication
pub const SESSION_PROTOCOL: &str = "/anarchy/session/1.0.0";

/// Maximum allowed timestamp drift (±30 seconds)
const MAX_TIMESTAMP_DRIFT_SECS: u64 = 30;

/// Session request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRequest {
    /// Method name ("storage_requestSession", "storage_renewSession", "storage_revokeSession")
    pub method: String,
    /// Request parameters
    pub params: SessionRequestParams,
    /// JSON-RPC ID
    pub id: u32,
}

/// Request parameters variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SessionRequestParams {
    /// Request new session
    Request {
        /// Ed25519 public key (hex, 64 characters)
        public_key: String,
        /// Unix timestamp (seconds)
        timestamp: u64,
        /// Unique nonce (hex, 32 characters) to prevent replay attacks
        nonce: String,
        /// Ed25519 signature (hex, 128 characters)
        signature: String,
    },
    /// Renew existing session
    Renew {
        /// Current session token (hex, 64 characters)
        token: String,
    },
    /// Revoke session
    Revoke {
        /// Session token to revoke (hex, 64 characters)
        token: String,
    },
}

/// Session response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    /// JSON-RPC version
    pub jsonrpc: String,
    /// Request ID
    pub id: u32,
    /// Result (on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<SessionResult>,
    /// Error (on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<SessionRpcError>,
}

/// Session result variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SessionResult {
    /// New session created
    Session {
        /// Session token (hex, 64 characters)
        token: String,
        /// Expiration timestamp (Unix seconds)
        expires_at: u64,
    },
    /// Session revoked
    Revoked { revoked: bool },
}

/// JSON-RPC error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRpcError {
    pub code: i32,
    pub message: String,
}

impl SessionResponse {
    /// Create a success response with a session token
    pub fn success_session(id: u32, token: String, expires_at: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(SessionResult::Session { token, expires_at }),
            error: None,
        }
    }

    /// Create a success response for revocation
    pub fn success_revoked(id: u32) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(SessionResult::Revoked { revoked: true }),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: u32, err: SessionError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(SessionRpcError {
                code: err.rpc_error_code(),
                message: err.to_string(),
            }),
        }
    }
}

impl SessionRequest {
    /// Verify the session request signature and extract peer_id and nonce
    ///
    /// Returns the (PeerId, nonce) derived from the public key if verification succeeds.
    /// The caller MUST check the nonce against a cache to prevent replay attacks.
    pub fn verify_signature(&self) -> Result<(PeerId, String), SessionError> {
        let (public_key_hex, timestamp, nonce_hex, signature_hex) = match &self.params {
            SessionRequestParams::Request {
                public_key,
                timestamp,
                nonce,
                signature,
            } => (public_key, *timestamp, nonce, signature),
            _ => return Err(SessionError::InvalidSignature),
        };

        // Validate nonce format (16 bytes = 32 hex chars)
        if nonce_hex.len() != 32 || !nonce_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(SessionError::InvalidNonce);
        }

        // Validate timestamp (±30 seconds)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SessionError::Internal)?
            .as_secs();

        if timestamp > now + MAX_TIMESTAMP_DRIFT_SECS
            || timestamp < now.saturating_sub(MAX_TIMESTAMP_DRIFT_SECS)
        {
            return Err(SessionError::InvalidTimestamp);
        }

        // Parse public key (32 bytes = 64 hex chars)
        let public_key_bytes: [u8; 32] = hex::decode(public_key_hex)
            .map_err(|_| SessionError::InvalidPublicKey)?
            .try_into()
            .map_err(|_| SessionError::InvalidPublicKey)?;

        // Create ed25519-dalek verifying key
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
            .map_err(|_| SessionError::InvalidPublicKey)?;

        // Parse signature (64 bytes = 128 hex chars)
        let signature_bytes: [u8; 64] = hex::decode(signature_hex)
            .map_err(|_| SessionError::InvalidSignature)?
            .try_into()
            .map_err(|_| SessionError::InvalidSignature)?;

        let signature = Signature::from_bytes(&signature_bytes);

        // Construct the signed message: "anarchy-session-request:{timestamp}:{nonce}"
        let message = format!("anarchy-session-request:{}:{}", timestamp, nonce_hex);

        // Verify signature
        verifying_key
            .verify_strict(message.as_bytes(), &signature)
            .map_err(|_| SessionError::InvalidSignature)?;

        // Derive PeerId from the public key
        let ed25519_pubkey = Ed25519PublicKey::try_from_bytes(&public_key_bytes)
            .map_err(|_| SessionError::InvalidPublicKey)?;
        let peer_id = PeerId::from_public_key(
            &libp2p::identity::PublicKey::from(ed25519_pubkey)
        );

        Ok((peer_id, nonce_hex.clone()))
    }

    /// Get the token from a renewal or revocation request
    pub fn get_token(&self) -> Option<&str> {
        match &self.params {
            SessionRequestParams::Renew { token } => Some(token),
            SessionRequestParams::Revoke { token } => Some(token),
            _ => None,
        }
    }
}

/// Codec for session protocol messages
#[derive(Debug, Clone, Default)]
pub struct SessionProtocolCodec;

#[async_trait::async_trait]
impl Codec for SessionProtocolCodec {
    type Protocol = &'static str;
    type Request = SessionRequest;
    type Response = SessionResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let mut length_buf = [0u8; 4];
        io.read_exact(&mut length_buf).await?;
        let length = u32::from_be_bytes(length_buf) as usize;

        if length > 64 * 1024 {
            // Max 64KB for session requests
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Session request too large",
            ));
        }

        let mut buf = vec![0u8; length];
        io.read_exact(&mut buf).await?;

        serde_json::from_slice(&buf).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let mut length_buf = [0u8; 4];
        io.read_exact(&mut length_buf).await?;
        let length = u32::from_be_bytes(length_buf) as usize;

        if length > 64 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Session response too large",
            ));
        }

        let mut buf = vec![0u8; length];
        io.read_exact(&mut buf).await?;

        serde_json::from_slice(&buf).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        let data = serde_json::to_vec(&req).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        let length = (data.len() as u32).to_be_bytes();
        io.write_all(&length).await?;
        io.write_all(&data).await?;
        io.flush().await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        let data = serde_json::to_vec(&res).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        let length = (data.len() as u32).to_be_bytes();
        io.write_all(&length).await?;
        io.write_all(&data).await?;
        io.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_response_serialization() {
        let response = SessionResponse::success_session(
            1,
            "a123".repeat(16),
            1709251200,
        );

        let json = serde_json::to_string(&response).unwrap();
        let parsed: SessionResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, 1);
        assert!(parsed.result.is_some());
        assert!(parsed.error.is_none());
    }

    #[test]
    fn test_session_error_response() {
        let response = SessionResponse::error(1, SessionError::NotConnected);

        assert!(response.error.is_some());
        assert_eq!(response.error.as_ref().unwrap().code, -32001);
    }
}

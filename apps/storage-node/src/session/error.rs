//! Session error types

use axum::http::StatusCode;
use thiserror::Error;

/// Session-related errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SessionError {
    /// Missing session token header
    #[error("Missing X-Session-Token header")]
    MissingToken,

    /// Invalid or expired session token
    #[error("Invalid or expired session token")]
    InvalidToken,

    /// Peer not connected via P2P
    #[error("Not connected via P2P")]
    NotConnected,

    /// Invalid signature
    #[error("Invalid signature")]
    InvalidSignature,

    /// Invalid timestamp (outside ±30 seconds)
    #[error("Invalid timestamp")]
    InvalidTimestamp,

    /// Invalid public key format
    #[error("Invalid public key format")]
    InvalidPublicKey,

    /// Invalid nonce format
    #[error("Invalid nonce format")]
    InvalidNonce,

    /// Nonce already used (replay attack detected)
    #[error("Nonce already used")]
    NonceReused,

    /// Session renewal not allowed (more than 1 hour until expiry)
    #[error("Session renewal not allowed yet")]
    RenewalNotAllowed,

    /// Internal error
    #[error("Internal session error")]
    Internal,
}

impl SessionError {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            SessionError::MissingToken => StatusCode::UNAUTHORIZED,
            SessionError::InvalidToken => StatusCode::FORBIDDEN,
            SessionError::NotConnected => StatusCode::FORBIDDEN,
            SessionError::InvalidSignature => StatusCode::FORBIDDEN,
            SessionError::InvalidTimestamp => StatusCode::FORBIDDEN,
            SessionError::InvalidPublicKey => StatusCode::BAD_REQUEST,
            SessionError::InvalidNonce => StatusCode::BAD_REQUEST,
            SessionError::NonceReused => StatusCode::FORBIDDEN,
            SessionError::RenewalNotAllowed => StatusCode::FORBIDDEN,
            SessionError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get the JSON-RPC error code
    pub fn rpc_error_code(&self) -> i32 {
        match self {
            SessionError::NotConnected => -32001,
            SessionError::InvalidSignature => -32002,
            SessionError::InvalidTimestamp => -32003,
            SessionError::InvalidPublicKey => -32004,
            SessionError::InvalidToken => -32005,
            SessionError::RenewalNotAllowed => -32006,
            SessionError::InvalidNonce => -32007,
            SessionError::NonceReused => -32008,
            SessionError::MissingToken => -32000,
            SessionError::Internal => -32099,
        }
    }
}

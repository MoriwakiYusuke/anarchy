//! Gossipsub implementation for endpoint sharing
//!
//! Implements Gossipsub protocol for sharing blockchain endpoint information
//! between storage nodes (FR-502, FR-512, FR-514).
//!
//! ## Protocol
//!
//! - Topic: `/anarchy/endpoints/1.0.0`
//! - Message: EndpointMessage (max 4KB)
//! - Signing: Ed25519 (required for all messages)
//! - Broadcast interval: 60 seconds

use std::time::Duration;
use libp2p::{
    gossipsub::{self, ConfigBuilder, ValidationMode},
    identity::Keypair,
    PeerId,
};
use serde::{Deserialize, Serialize};

use super::endpoint_cache::BlockchainEndpoint;

/// Gossipsub topic for endpoint sharing
pub const ENDPOINT_TOPIC: &str = "/anarchy/endpoints/1.0.0";

/// Maximum message size (4KB as per FR-514)
pub const MAX_MESSAGE_SIZE: usize = 4096;

/// Maximum endpoints per message
pub const MAX_ENDPOINTS_PER_MESSAGE: usize = 20;

/// Broadcast interval for endpoint updates
pub const BROADCAST_INTERVAL: Duration = Duration::from_secs(60);

/// Message structure for Gossipsub endpoint sharing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointMessage {
    /// List of known endpoints (max 20)
    pub endpoints: Vec<BlockchainEndpoint>,
    
    /// Sender's PeerID (base58 encoded)
    pub sender_peer_id: String,
    
    /// Message timestamp (Unix seconds)
    pub timestamp: u64,
    
    /// Ed25519 signature of (sender_peer_id || timestamp || hash(endpoints)), hex-encoded
    pub signature: String,
}

impl EndpointMessage {
    /// Create a new endpoint message with signature
    pub fn new(
        endpoints: Vec<BlockchainEndpoint>,
        sender_peer_id: PeerId,
        _keypair: &Keypair,
    ) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let sender_peer_id_str = sender_peer_id.to_base58();
        
        // TODO: Implement actual signing in T058
        let signature = "0".repeat(128); // 64 bytes hex = 128 chars
        
        Self {
            endpoints,
            sender_peer_id: sender_peer_id_str,
            timestamp,
            signature,
        }
    }
    
    /// Verify the message signature
    pub fn verify_signature(&self) -> bool {
        // TODO: Implement actual verification in T059
        true
    }
    
    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
    
    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }
    
    /// Check if message size is within limit
    pub fn is_size_valid(&self) -> bool {
        match self.to_bytes() {
            Ok(bytes) => bytes.len() <= MAX_MESSAGE_SIZE,
            Err(_) => false,
        }
    }
}

/// Result of message validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageValidation {
    /// Message is valid
    Valid,
    /// Message is too large
    TooLarge,
    /// Invalid signature
    InvalidSignature,
    /// Message is expired (too old)
    Expired,
    /// Malformed message
    Malformed,
}

/// Validate an incoming endpoint message
pub fn validate_message(data: &[u8]) -> MessageValidation {
    // Check size limit
    if data.len() > MAX_MESSAGE_SIZE {
        return MessageValidation::TooLarge;
    }
    
    // Parse message
    let message = match EndpointMessage::from_bytes(data) {
        Ok(m) => m,
        Err(_) => return MessageValidation::Malformed,
    };
    
    // Verify signature
    if !message.verify_signature() {
        return MessageValidation::InvalidSignature;
    }
    
    // Check timestamp (allow 5 minute skew)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let age = now.saturating_sub(message.timestamp);
    if age > 300 {
        return MessageValidation::Expired;
    }
    
    MessageValidation::Valid
}

/// Build Gossipsub configuration for endpoint sharing
pub fn build_gossipsub_config() -> gossipsub::Config {
    ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(ValidationMode::Strict)
        .max_transmit_size(MAX_MESSAGE_SIZE)
        .build()
        .expect("Valid gossipsub config")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_message_serialization() {
        let msg = EndpointMessage {
            endpoints: vec![],
            sender_peer_id: "12D3KooWtest".to_string(),
            timestamp: 1234567890,
            signature: "0".repeat(128),
        };
        
        let bytes = msg.to_bytes().unwrap();
        let decoded = EndpointMessage::from_bytes(&bytes).unwrap();
        
        assert_eq!(decoded.sender_peer_id, msg.sender_peer_id);
        assert_eq!(decoded.timestamp, msg.timestamp);
    }

    #[test]
    fn test_message_size_limit() {
        let msg = EndpointMessage {
            endpoints: vec![],
            sender_peer_id: "test".to_string(),
            timestamp: 0,
            signature: "0".repeat(128),
        };
        
        assert!(msg.is_size_valid());
    }

    #[test]
    fn test_validate_message_too_large() {
        let data = vec![0u8; MAX_MESSAGE_SIZE + 1];
        assert_eq!(validate_message(&data), MessageValidation::TooLarge);
    }

    #[test]
    fn test_validate_message_malformed() {
        let data = b"not valid json";
        assert_eq!(validate_message(data), MessageValidation::Malformed);
    }
}

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
use blake2::{Blake2b, Digest};
use blake2::digest::consts::U32;

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
        keypair: &Keypair,
    ) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let sender_peer_id_str = sender_peer_id.to_base58();
        
        // Build message bytes for signing: peer_id || timestamp || hash(endpoints)
        let endpoints_hash = hash_endpoints(&endpoints);
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(sender_peer_id_str.as_bytes());
        sign_data.extend_from_slice(&timestamp.to_le_bytes());
        sign_data.extend_from_slice(&endpoints_hash);
        
        // Sign with Ed25519
        let signature = match keypair.sign(&sign_data) {
            Ok(sig) => hex::encode(sig),
            Err(_) => "0".repeat(128), // Fallback for non-Ed25519 keys
        };
        
        Self {
            endpoints,
            sender_peer_id: sender_peer_id_str,
            timestamp,
            signature,
        }
    }
    
    /// Verify the message signature
    pub fn verify_signature(&self) -> bool {
        // Decode signature
        let sig_bytes = match hex::decode(&self.signature) {
            Ok(b) => b,
            Err(_) => return false,
        };
        
        // Parse PeerId from string
        let peer_id = match self.sender_peer_id.parse::<PeerId>() {
            Ok(id) => id,
            Err(_) => return false,
        };
        
        // Extract public key from PeerId (works for Ed25519)
        let public_key = match libp2p::identity::PublicKey::try_decode_protobuf(
            &peer_id.to_bytes()[2..] // Skip multicodec bytes
        ) {
            Ok(pk) => pk,
            Err(_) => {
                // Old format, or can't extract pubkey - use a different approach
                // For modern PeerIds, the pubkey is embedded
                return false;
            }
        };
        
        // Rebuild message bytes for verification
        let endpoints_hash = hash_endpoints(&self.endpoints);
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(self.sender_peer_id.as_bytes());
        sign_data.extend_from_slice(&self.timestamp.to_le_bytes());
        sign_data.extend_from_slice(&endpoints_hash);
        
        // Verify signature
        public_key.verify(&sign_data, &sig_bytes)
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

/// Hash endpoints list for signing
fn hash_endpoints(endpoints: &[BlockchainEndpoint]) -> [u8; 32] {
    let mut hasher = Blake2b::<U32>::new();
    for ep in endpoints {
        hasher.update(ep.url.as_bytes());
        hasher.update(&ep.chain_id);
        hasher.update(&ep.latency_ms.to_le_bytes());
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_endpoint() -> BlockchainEndpoint {
        BlockchainEndpoint {
            url: "ws://localhost:9944".to_string(),
            chain_id: [0u8; 32],
            last_verified: 1234567890,
            latency_ms: 50,
            ttl_secs: 300,
        }
    }

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

    // T079: Test Gossipsub message serialization/deserialization
    #[test]
    fn t079_gossipsub_serialization() {
        let endpoints = vec![make_test_endpoint(), make_test_endpoint()];
        let msg = EndpointMessage {
            endpoints: endpoints.clone(),
            sender_peer_id: "12D3KooWTest123".to_string(),
            timestamp: 1700000000,
            signature: "ab".repeat(64),
        };
        
        let bytes = msg.to_bytes().expect("Should serialize");
        let decoded = EndpointMessage::from_bytes(&bytes).expect("Should deserialize");
        
        assert_eq!(decoded.endpoints.len(), 2);
        assert_eq!(decoded.sender_peer_id, msg.sender_peer_id);
        assert_eq!(decoded.timestamp, msg.timestamp);
        assert_eq!(decoded.signature, msg.signature);
        assert_eq!(decoded.endpoints[0].url, "ws://localhost:9944");
    }

    // T080: Test invalid signature message rejection
    #[test]
    fn t080_invalid_signature_rejected() {
        // Create a message with invalid signature
        let msg = EndpointMessage {
            endpoints: vec![make_test_endpoint()],
            sender_peer_id: "12D3KooWInvalid".to_string(),  // Not a real PeerId
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: "ff".repeat(64), // Invalid signature
        };
        
        // Should fail verification (PeerId is not a proper one)
        assert!(!msg.verify_signature());
    }

    // T081: Test 4KB message size limit
    #[test]
    fn t081_message_size_limit() {
        // Empty message should be within limit
        let small_msg = EndpointMessage {
            endpoints: vec![],
            sender_peer_id: "test".to_string(),
            timestamp: 0,
            signature: "0".repeat(128),
        };
        assert!(small_msg.is_size_valid());
        
        // Create a message that exceeds 4KB
        let mut large_endpoints = Vec::new();
        for i in 0..100 {
            large_endpoints.push(BlockchainEndpoint {
                url: format!("ws://node{}.example.com:9944/very/long/path/{}", i, "x".repeat(200)),
                chain_id: [i as u8; 32],
                last_verified: 1000000000 + i as u64,
                latency_ms: 50 + i,
                ttl_secs: 300,
            });
        }
        
        let large_msg = EndpointMessage {
            endpoints: large_endpoints,
            sender_peer_id: "test".to_string(),
            timestamp: 0,
            signature: "0".repeat(128),
        };
        
        // Should exceed size limit
        assert!(!large_msg.is_size_valid());
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

    #[test]
    fn test_hash_endpoints_deterministic() {
        let endpoints = vec![make_test_endpoint()];
        let hash1 = hash_endpoints(&endpoints);
        let hash2 = hash_endpoints(&endpoints);
        assert_eq!(hash1, hash2);
        
        // Different endpoints should have different hashes
        let endpoints2 = vec![BlockchainEndpoint {
            url: "ws://different:9944".to_string(),
            ..make_test_endpoint()
        }];
        let hash3 = hash_endpoints(&endpoints2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_gossipsub_config() {
        let config = build_gossipsub_config();
        assert_eq!(config.max_transmit_size(), MAX_MESSAGE_SIZE);
    }
}

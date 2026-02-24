//! Repair protocol types (013-slashing-repair T030)
//!
//! Defines message types for the fragment repair protocol.
//! Uses libp2p request-response pattern for peer communication.

use serde::{Deserialize, Serialize};

/// Content hash type (Blake2-256, 32 bytes)
pub type ContentHash = [u8; 32];

/// KZG commitment type (compressed G1 point, 48 bytes as Vec<u8>)
pub type KzgCommitment = Vec<u8>;

/// KZG proof type (compressed G1 point, 48 bytes as Vec<u8>)
pub type KzgProof = Vec<u8>;

/// Share index type (1-255)
pub type ShareIndex = u8;

/// Expected size of KZG commitment/proof (48 bytes for compressed G1)
pub const KZG_COMMITMENT_SIZE: usize = 48;

/// Expected size of share value (32 bytes for scalar field element)
pub const SHARE_VALUE_SIZE: usize = 32;

/// VSS share data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareData {
    /// Share index (1-based)
    pub index: ShareIndex,
    /// Share value (32 bytes, scalar field element as Vec<u8>)
    pub value: Vec<u8>,
    /// KZG proof for the share
    pub proof: KzgProof,
}

impl ShareData {
    /// Create new ShareData from byte arrays
    pub fn new(index: ShareIndex, value: &[u8; 32], proof: &[u8; 48]) -> Self {
        Self {
            index,
            value: value.to_vec(),
            proof: proof.to_vec(),
        }
    }
    
    /// Get value as fixed-size array if valid
    pub fn value_array(&self) -> Option<[u8; 32]> {
        if self.value.len() == SHARE_VALUE_SIZE {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&self.value);
            Some(arr)
        } else {
            None
        }
    }
    
    /// Get proof as fixed-size array if valid
    pub fn proof_array(&self) -> Option<[u8; 48]> {
        if self.proof.len() == KZG_COMMITMENT_SIZE {
            let mut arr = [0u8; 48];
            arr.copy_from_slice(&self.proof);
            Some(arr)
        } else {
            None
        }
    }
}

// ============ Repair Protocol Messages ============

/// Request types for repair protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RepairRequest {
    /// Coordinator requests a share from donor node
    CollectShare {
        /// Content hash being repaired
        content_hash: ContentHash,
        /// Share index requested (the index the donor holds)
        requested_index: ShareIndex,
        /// Coordinator's peer ID (for response routing)
        coordinator: Vec<u8>,
    },
    
    /// Coordinator pushes regenerated share to new holder
    PushShare {
        /// Content hash being repaired
        content_hash: ContentHash,
        /// The share data being pushed
        share: ShareData,
        /// KZG commitment for verification
        commitment: KzgCommitment,
    },
    
    /// Query a node's health/availability
    Ping {
        /// Optional nonce for request-response matching
        nonce: u64,
    },
}

/// Response types for repair protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RepairResponse {
    /// Donor provides their share
    ShareProvided {
        /// Content hash
        content_hash: ContentHash,
        /// The share data
        share: ShareData,
    },
    
    /// Donor refuses or cannot provide share
    ShareDenied {
        /// Content hash
        content_hash: ContentHash,
        /// Reason for denial
        reason: ShareDenialReason,
    },
    
    /// New holder accepts pushed share
    ShareAccepted {
        /// Content hash
        content_hash: ContentHash,
        /// The share index accepted
        share_index: ShareIndex,
    },
    
    /// New holder rejects pushed share
    ShareRejected {
        /// Content hash
        content_hash: ContentHash,
        /// Reason for rejection
        reason: ShareRejectionReason,
    },
    
    /// Ping response
    Pong {
        /// Echo back the nonce
        nonce: u64,
    },
}

/// Reasons a donor might deny a share request
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ShareDenialReason {
    /// Donor doesn't hold this content
    NotHolder,
    /// Share index doesn't match what donor holds
    WrongIndex,
    /// Content marked for GC/forgetting
    ContentExpired,
    /// Donor is busy/overloaded
    Busy,
    /// Unknown/other error
    Unknown,
}

/// Reasons a receiver might reject a pushed share
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ShareRejectionReason {
    /// Receiver already holds a share for this content
    AlreadyHolder,
    /// KZG proof verification failed
    InvalidProof,
    /// Receiver has no capacity
    NoCapacity,
    /// Receiver is not interested in this content
    NotInterested,
    /// Unknown/other error
    Unknown,
}

// ============ Protocol Codec ============

/// Protocol ID for libp2p
pub const REPAIR_PROTOCOL_ID: &str = "/anarchy/repair/1.0.0";

/// Serialize a repair request to bytes
pub fn encode_request(request: &RepairRequest) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(request)
}

/// Deserialize a repair request from bytes
pub fn decode_request(bytes: &[u8]) -> Result<RepairRequest, serde_json::Error> {
    serde_json::from_slice(bytes)
}

/// Serialize a repair response to bytes
pub fn encode_response(response: &RepairResponse) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(response)
}

/// Deserialize a repair response from bytes
pub fn decode_response(bytes: &[u8]) -> Result<RepairResponse, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_collect_share() {
        let request = RepairRequest::CollectShare {
            content_hash: [1u8; 32],
            requested_index: 3,
            coordinator: b"QmTestPeerId".to_vec(),
        };
        
        let bytes = encode_request(&request).unwrap();
        let decoded = decode_request(&bytes).unwrap();
        
        match decoded {
            RepairRequest::CollectShare { content_hash, requested_index, coordinator } => {
                assert_eq!(content_hash, [1u8; 32]);
                assert_eq!(requested_index, 3);
                assert_eq!(coordinator, b"QmTestPeerId".to_vec());
            }
            _ => panic!("Wrong request type"),
        }
    }

    #[test]
    fn test_encode_decode_share_provided() {
        let response = RepairResponse::ShareProvided {
            content_hash: [2u8; 32],
            share: ShareData::new(5, &[3u8; 32], &[4u8; 48]),
        };
        
        let bytes = encode_response(&response).unwrap();
        let decoded = decode_response(&bytes).unwrap();
        
        match decoded {
            RepairResponse::ShareProvided { content_hash, share } => {
                assert_eq!(content_hash, [2u8; 32]);
                assert_eq!(share.index, 5);
                assert_eq!(share.value_array().unwrap(), [3u8; 32]);
            }
            _ => panic!("Wrong response type"),
        }
    }
}

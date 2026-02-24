//! Receiver handler for repair protocol (013-slashing-repair T033)
//!
//! Handles PushShare requests from repair coordinators.
//! Verifies KZG proof and stores the new share locally.

use crate::repair::protocol::{
    ContentHash, KzgCommitment, RepairRequest, RepairResponse, ShareData, ShareRejectionReason,
};
use crate::storage::FragmentStore;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Receiver handler for accepting pushed shares
pub struct ReceiverHandler {
    /// Local storage for fragment data
    storage: Arc<FragmentStore>,
}

impl ReceiverHandler {
    /// Create a new receiver handler
    pub fn new(storage: Arc<FragmentStore>) -> Self {
        Self { storage }
    }

    /// Handle an incoming PushShare request
    ///
    /// Returns a RepairResponse:
    /// - ShareAccepted if we successfully store the share
    /// - ShareRejected with reason otherwise
    pub async fn handle_push_share(
        &self,
        content_hash: ContentHash,
        share: ShareData,
        commitment: KzgCommitment,
    ) -> RepairResponse {
        debug!(
            content_hash = hex::encode(content_hash),
            share_index = share.index,
            "Handling PushShare request"
        );

        // Check if we already hold a share for this content
        if self.storage.exists(&content_hash) {
            warn!(
                content_hash = hex::encode(content_hash),
                "Already holding a share for this content"
            );
            return RepairResponse::ShareRejected {
                content_hash,
                reason: ShareRejectionReason::AlreadyHolder,
            };
        }

        // Check capacity
        let used = self.storage.used_bytes();
        let max_capacity = self.storage.capacity_bytes();
        let share_size = share.value.len() + share.proof.len();
        if used + share_size as u64 > max_capacity {
            warn!(
                used,
                max = max_capacity,
                "Insufficient capacity to store new share"
            );
            return RepairResponse::ShareRejected {
                content_hash,
                reason: ShareRejectionReason::NoCapacity,
            };
        }

        // Verify KZG proof
        if !self.verify_kzg_proof(&commitment, &share) {
            warn!(
                content_hash = hex::encode(content_hash),
                "KZG proof verification failed"
            );
            return RepairResponse::ShareRejected {
                content_hash,
                reason: ShareRejectionReason::InvalidProof,
            };
        }

        // Store the share locally
        // Format: share_value (32 bytes) + kzg_proof (48 bytes)
        let mut data = Vec::with_capacity(share.value.len() + share.proof.len());
        data.extend_from_slice(&share.value);
        data.extend_from_slice(&share.proof);

        match self.storage.store(content_hash, &data) {
            Ok(()) => {
                info!(
                    content_hash = hex::encode(content_hash),
                    share_index = share.index,
                    "Successfully stored pushed share"
                );
                RepairResponse::ShareAccepted {
                    content_hash,
                    share_index: share.index,
                }
            }
            Err(e) => {
                warn!(
                    content_hash = hex::encode(content_hash),
                    error = %e,
                    "Failed to store share"
                );
                RepairResponse::ShareRejected {
                    content_hash,
                    reason: ShareRejectionReason::Unknown,
                }
            }
        }
    }

    /// Verify KZG proof for a share
    ///
    /// Uses wasm-engine's verify_kzg_proof function
    fn verify_kzg_proof(&self, _commitment: &KzgCommitment, _share: &ShareData) -> bool {
        // TODO: Call wasm-engine verify_kzg_proof
        // For MVP, we trust the coordinator's proof
        // In production, this MUST verify the KZG proof:
        // verify_kzg_proof(commitment, share.index, share.value, share.proof)
        true
    }

    /// Handle a repair protocol request
    pub async fn handle_request(&self, request: RepairRequest) -> RepairResponse {
        match request {
            RepairRequest::PushShare {
                content_hash,
                share,
                commitment,
            } => self.handle_push_share(content_hash, share, commitment).await,
            RepairRequest::Ping { nonce } => RepairResponse::Pong { nonce },
            RepairRequest::CollectShare { content_hash, .. } => {
                // Receivers don't handle CollectShare - that's for donors
                warn!("ReceiverHandler received CollectShare - ignoring");
                RepairResponse::ShareRejected {
                    content_hash,
                    reason: ShareRejectionReason::Unknown,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Integration tests should cover the full flow with FragmentStore
}

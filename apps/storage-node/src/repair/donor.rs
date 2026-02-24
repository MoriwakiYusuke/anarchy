//! Donor handler for repair protocol (013-slashing-repair T031)
//!
//! Handles CollectShare requests from repair coordinators.
//! Responds with the share data if held, or denial reasons.

use crate::repair::protocol::{
    ContentHash, RepairRequest, RepairResponse, ShareData, ShareDenialReason,
};
use crate::storage::FragmentStore;
use std::sync::Arc;
use tracing::{debug, warn};

/// Donor handler for share collection requests
pub struct DonorHandler {
    /// Local storage for fragment data
    storage: Arc<FragmentStore>,
    /// Local share index (which share this node holds)
    share_index: u8,
}

impl DonorHandler {
    /// Create a new donor handler
    pub fn new(storage: Arc<FragmentStore>, share_index: u8) -> Self {
        Self { storage, share_index }
    }

    /// Handle an incoming CollectShare request
    ///
    /// Returns a RepairResponse:
    /// - ShareProvided if we hold the requested share
    /// - ShareDenied with reason otherwise
    pub async fn handle_collect_share(
        &self,
        content_hash: ContentHash,
        requested_index: u8,
        _coordinator: Vec<u8>,
    ) -> RepairResponse {
        debug!(
            content_hash = hex::encode(content_hash),
            requested_index,
            "Handling CollectShare request"
        );

        // Verify the requested index matches what we hold
        if self.share_index != requested_index {
            warn!(
                content_hash = hex::encode(content_hash),
                requested_index,
                actual_index = self.share_index,
                "Requested index doesn't match our share"
            );
            return RepairResponse::ShareDenied {
                content_hash,
                reason: ShareDenialReason::WrongIndex,
            };
        }

        // Check if we hold this content using fragment_id = content_hash
        match self.storage.retrieve(&content_hash) {
            Ok(Some(data)) => {
                debug!(
                    content_hash = hex::encode(content_hash),
                    share_index = self.share_index,
                    data_len = data.len(),
                    "Providing share to coordinator"
                );

                // Parse the stored data as ShareData
                // The stored fragment contains: share_value (32 bytes) + kzg_proof (48 bytes)
                if data.len() < 32 + 48 {
                    warn!(
                        content_hash = hex::encode(content_hash),
                        data_len = data.len(),
                        "Stored fragment too small for share data"
                    );
                    return RepairResponse::ShareDenied {
                        content_hash,
                        reason: ShareDenialReason::Unknown,
                    };
                }

                let share_value = data[..32].to_vec();
                let kzg_proof = data[32..80].to_vec();

                RepairResponse::ShareProvided {
                    content_hash,
                    share: ShareData {
                        index: self.share_index,
                        value: share_value,
                        proof: kzg_proof,
                    },
                }
            }
            Ok(None) => {
                debug!(
                    content_hash = hex::encode(content_hash),
                    "Share not found in local storage"
                );
                RepairResponse::ShareDenied {
                    content_hash,
                    reason: ShareDenialReason::NotHolder,
                }
            }
            Err(e) => {
                warn!(
                    content_hash = hex::encode(content_hash),
                    error = %e,
                    "Error reading share from storage"
                );
                RepairResponse::ShareDenied {
                    content_hash,
                    reason: ShareDenialReason::Unknown,
                }
            }
        }
    }

    /// Handle a repair protocol request
    pub async fn handle_request(&self, request: RepairRequest) -> RepairResponse {
        match request {
            RepairRequest::CollectShare {
                content_hash,
                requested_index,
                coordinator,
            } => {
                self.handle_collect_share(content_hash, requested_index, coordinator)
                    .await
            }
            RepairRequest::Ping { nonce } => RepairResponse::Pong { nonce },
            RepairRequest::PushShare { .. } => {
                // Donors don't handle PushShare - that's for receivers
                warn!("DonorHandler received PushShare - ignoring");
                RepairResponse::ShareDenied {
                    content_hash: [0u8; 32],
                    reason: ShareDenialReason::Unknown,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Integration tests should cover the full flow with FragmentStore
}

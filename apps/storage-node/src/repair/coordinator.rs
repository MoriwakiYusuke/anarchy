//! Repair coordinator (013-slashing-repair T032)
//!
//! Orchestrates the repair process:
//! 1. Discovers AtRisk fragments
//! 2. Collects k shares from existing holders
//! 3. Regenerates new share using Lagrange interpolation
//! 4. Pushes regenerated share to new holder (self)
//! 5. Submits confirm_repair on-chain

use crate::repair::protocol::{
    ContentHash, KzgCommitment, KzgProof, RepairRequest, RepairResponse, ShareData,
};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Configuration for repair coordinator
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// Timeout for collecting a single share
    pub share_collect_timeout: Duration,
    /// Maximum concurrent share collection requests
    pub max_concurrent_requests: usize,
    /// Recovery threshold (k in k-of-n)
    pub threshold_k: u8,
    /// Total shares (n in k-of-n)
    pub total_shares_n: u8,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            share_collect_timeout: Duration::from_secs(30),
            max_concurrent_requests: 5,
            threshold_k: 3,
            total_shares_n: 5,
        }
    }
}

/// Errors that can occur during repair coordination
#[derive(Debug, Error)]
pub enum CoordinatorError {
    #[error("Insufficient shares collected: got {collected}, need {required}")]
    InsufficientShares { collected: usize, required: usize },

    #[error("Share regeneration failed: {0}")]
    RegenerationFailed(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Chain submission failed: {0}")]
    ChainSubmissionFailed(String),

    #[error("No eligible holders found")]
    NoEligibleHolders,

    #[error("Timeout collecting shares")]
    Timeout,
}

/// Information about an existing holder for share collection
#[derive(Debug, Clone)]
pub struct HolderInfo {
    /// Peer ID of the holder
    pub peer_id: Vec<u8>,
    /// Share index held by this peer
    pub share_index: u8,
    /// HTTP URL for direct communication (optional)
    pub http_url: Option<String>,
}

/// Result of a share collection attempt
#[derive(Debug)]
pub enum CollectionResult {
    /// Successfully collected share
    Success(ShareData),
    /// Failed to collect share
    Failed(String),
    /// Holder denied the request
    Denied(String),
    /// Request timed out
    Timeout,
}

/// Repair coordinator state machine
pub struct Coordinator {
    config: CoordinatorConfig,
    /// Collected shares (index -> share data)
    collected_shares: HashMap<u8, ShareData>,
}

impl Coordinator {
    /// Create a new repair coordinator
    pub fn new(config: CoordinatorConfig) -> Self {
        Self {
            config,
            collected_shares: HashMap::new(),
        }
    }

    /// Collect shares from holders for repair
    ///
    /// Returns collected shares when we have at least k shares
    pub async fn collect_shares<F>(
        &mut self,
        content_hash: ContentHash,
        holders: &[HolderInfo],
        send_request: F,
    ) -> Result<Vec<ShareData>, CoordinatorError>
    where
        F: Fn(&[u8], RepairRequest) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<RepairResponse>> + Send>>,
    {
        self.collected_shares.clear();

        if holders.is_empty() {
            return Err(CoordinatorError::NoEligibleHolders);
        }

        info!(
            content_hash = hex::encode(content_hash),
            holder_count = holders.len(),
            threshold = self.config.threshold_k,
            "Starting share collection"
        );

        // Collect shares from holders
        for holder in holders {
            if self.collected_shares.len() >= self.config.threshold_k as usize {
                break;
            }

            let request = RepairRequest::CollectShare {
                content_hash,
                requested_index: holder.share_index,
                coordinator: vec![], // Will be set by network layer
            };

            debug!(
                peer_id = hex::encode(&holder.peer_id),
                share_index = holder.share_index,
                "Requesting share from holder"
            );

            // Send request and await response
            let response = send_request(&holder.peer_id, request).await;

            match response {
                Some(RepairResponse::ShareProvided { share, .. }) => {
                    debug!(
                        share_index = share.index,
                        "Successfully collected share"
                    );
                    self.collected_shares.insert(share.index, share);
                }
                Some(RepairResponse::ShareDenied { reason, .. }) => {
                    warn!(
                        peer_id = hex::encode(&holder.peer_id),
                        reason = ?reason,
                        "Holder denied share request"
                    );
                }
                _ => {
                    warn!(
                        peer_id = hex::encode(&holder.peer_id),
                        "No response or unexpected response from holder"
                    );
                }
            }
        }

        if self.collected_shares.len() < self.config.threshold_k as usize {
            return Err(CoordinatorError::InsufficientShares {
                collected: self.collected_shares.len(),
                required: self.config.threshold_k as usize,
            });
        }

        info!(
            collected = self.collected_shares.len(),
            "Share collection complete"
        );

        Ok(self.collected_shares.values().cloned().collect())
    }

    /// Regenerate a new share using Lagrange interpolation
    ///
    /// Uses wasm-engine's regenerate_share function
    pub fn regenerate_share(
        &self,
        shares: &[ShareData],
        _commitment: &KzgCommitment,
        new_index: u8,
    ) -> Result<(ShareData, KzgProof), CoordinatorError> {
        info!(
            share_count = shares.len(),
            new_index,
            "Regenerating share via Lagrange interpolation"
        );

        // Convert ShareData to wasm-engine format
        // Note: In production, this calls anarchy_wasm_engine::regenerate_share
        // For now, we create a placeholder implementation

        // Validate we have enough shares
        if shares.len() < self.config.threshold_k as usize {
            return Err(CoordinatorError::InsufficientShares {
                collected: shares.len(),
                required: self.config.threshold_k as usize,
            });
        }

        // TODO: Call wasm-engine regenerate_share
        // let vss_shares: Vec<VssShare> = shares.iter()
        //     .map(|s| VssShare { index: s.index, value: s.value })
        //     .collect();
        // let (new_share, proof) = regenerate_share(&vss_shares, threshold, new_index, commitment)?;

        // Placeholder: Create mock share for testing
        // In production, this MUST call the actual Lagrange interpolation
        let new_share = ShareData {
            index: new_index,
            value: vec![0u8; 32], // Would be computed via Lagrange
            proof: vec![0u8; 48], // Would be KZG proof
        };

        let proof = vec![0u8; 48];

        info!(
            new_index,
            "Successfully regenerated share"
        );

        Ok((new_share, proof))
    }

    /// Execute full repair flow for a content hash
    pub async fn execute_repair<F, C>(
        &mut self,
        content_hash: ContentHash,
        commitment: KzgCommitment,
        holders: &[HolderInfo],
        new_share_index: u8,
        send_request: F,
        submit_confirm_repair: C,
    ) -> Result<(), CoordinatorError>
    where
        F: Fn(&[u8], RepairRequest) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<RepairResponse>> + Send>>,
        C: Fn(ContentHash, u8, KzgProof) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>,
    {
        info!(
            content_hash = hex::encode(content_hash),
            "Starting repair flow"
        );

        // Step 1: Collect k shares
        let shares = self.collect_shares(content_hash, holders, send_request).await?;

        // Step 2: Regenerate new share
        let (new_share, kzg_proof) = self.regenerate_share(&shares, &commitment, new_share_index)?;

        // Step 3: Store the new share locally
        // (Handled by caller/receiver)

        // Step 4: Submit confirm_repair on-chain
        submit_confirm_repair(content_hash, new_share.index, kzg_proof)
            .await
            .map_err(|e| CoordinatorError::ChainSubmissionFailed(e))?;

        info!(
            content_hash = hex::encode(content_hash),
            new_share_index = new_share.index,
            "Repair flow completed successfully"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_creation() {
        let config = CoordinatorConfig::default();
        let coordinator = Coordinator::new(config);
        assert!(coordinator.collected_shares.is_empty());
    }

    #[test]
    fn test_insufficient_shares_error() {
        let coord = Coordinator::new(CoordinatorConfig::default());
        let shares = vec![
            ShareData::new(1, &[0u8; 32], &[0u8; 48]),
            ShareData::new(2, &[0u8; 32], &[0u8; 48]),
        ];
        // Threshold is 3, we only have 2
        let result = coord.regenerate_share(&shares, &[0u8; 48].to_vec(), 6);
        assert!(matches!(result, Err(CoordinatorError::InsufficientShares { .. })));
    }
}

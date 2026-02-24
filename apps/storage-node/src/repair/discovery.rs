//! Fragment discovery for repair (013-slashing-repair T034)
//!
//! Queries the blockchain for AtRisk fragments and determines
//! which fragments need repair and who can help.

use crate::repair::coordinator::HolderInfo;
use crate::repair::protocol::ContentHash;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Configuration for repair discovery
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// How often to check for AtRisk fragments
    pub check_interval: Duration,
    /// RPC endpoint for blockchain queries
    pub chain_rpc_url: String,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(60),
            chain_rpc_url: "ws://127.0.0.1:9944".to_string(),
        }
    }
}

/// Errors that can occur during discovery
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("Chain query failed: {0}")]
    ChainQueryFailed(String),

    #[error("RPC connection error: {0}")]
    RpcError(String),

    #[error("Decoding error: {0}")]
    DecodingError(String),
}

/// Information about an AtRisk fragment
#[derive(Debug, Clone)]
pub struct AtRiskFragment {
    /// Content hash
    pub content_hash: ContentHash,
    /// KZG commitment
    pub commitment: [u8; 48],
    /// Current holder count
    pub holder_count: u8,
    /// Recovery threshold (k)
    pub threshold: u8,
    /// Total shares (n)
    pub fragment_count: u8,
    /// List of current holders
    pub holders: Vec<HolderInfo>,
}

/// Fragment discovery service
pub struct DiscoveryService {
    config: DiscoveryConfig,
}

impl DiscoveryService {
    /// Create a new discovery service
    pub fn new(config: DiscoveryConfig) -> Self {
        Self { config }
    }

    /// Query blockchain for all AtRisk fragments
    ///
    /// Uses the get_at_risk_fragments Runtime API
    pub async fn get_at_risk_fragments(&self) -> Result<Vec<ContentHash>, DiscoveryError> {
        debug!(
            rpc_url = %self.config.chain_rpc_url,
            "Querying AtRisk fragments from chain"
        );

        // TODO: Implement actual RPC call using polkadot-api (PAPI)
        // 
        // Example using PAPI:
        // ```
        // let client = createClient(getWsProvider(&self.config.chain_rpc_url));
        // let api = client.getUnsafeApi();
        // let fragments = api.call.storageApi.getAtRiskFragments();
        // ```
        //
        // For now, return empty list (will be populated by integration)

        info!("AtRisk fragment query complete (placeholder)");
        Ok(vec![])
    }

    /// Get detailed fragment info including holders
    ///
    /// Uses the get_kzg_fragment Runtime API
    pub async fn get_fragment_info(
        &self,
        content_hash: &ContentHash,
    ) -> Result<Option<AtRiskFragment>, DiscoveryError> {
        debug!(
            content_hash = hex::encode(content_hash),
            "Querying fragment info from chain"
        );

        // TODO: Implement actual RPC call
        //
        // 1. Get KzgFragment via get_kzg_fragment API
        // 2. Get FragmentState via get_fragment_state API
        // 3. Map holder account IDs to peer IDs via StorageNodes
        //
        // For now, return None (will be populated by integration)

        Ok(None)
    }

    /// Get holder information for a fragment
    ///
    /// Queries the chain for each holder's peer ID and HTTP URL
    pub async fn get_holder_info(
        &self,
        holder_accounts: &[[u8; 32]],
    ) -> Result<Vec<HolderInfo>, DiscoveryError> {
        let holders = Vec::new();

        for account in holder_accounts {
            // TODO: Query StorageNodes to get peer_id and http_url for this operator
            // For now, skip
            debug!(
                account = hex::encode(account),
                "Looking up holder info (placeholder)"
            );
        }

        Ok(holders)
    }

    /// Find a new share index for repair
    ///
    /// Returns the next available share index (typically fragment_count + repair_count)
    pub fn allocate_share_index(&self, fragment_count: u8, existing_indices: &[u8]) -> Option<u8> {
        // Start from fragment_count + 1 and find first unused
        for candidate in (fragment_count + 1)..=255 {
            if !existing_indices.contains(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    /// Check if we should participate in repairing a fragment
    ///
    /// Returns true if:
    /// - We are not already a holder
    /// - We have sufficient capacity
    /// - Fragment is worth repairing (enough holders remain for k)
    pub fn should_participate(
        &self,
        fragment: &AtRiskFragment,
        our_account: &[u8; 32],
        our_capacity: u64,
        share_size: u64,
    ) -> bool {
        // Check we're not already a holder
        for holder in &fragment.holders {
            // Compare account IDs properly
            // For now, assume peer_id contains the account (simplified)
            if holder.peer_id.len() >= 32 && holder.peer_id[..32] == our_account[..] {
                debug!(
                    content_hash = hex::encode(fragment.content_hash),
                    "Already a holder, skipping"
                );
                return false;
            }
        }

        // Check capacity
        if our_capacity < share_size {
            debug!(
                content_hash = hex::encode(fragment.content_hash),
                "Insufficient capacity for share"
            );
            return false;
        }

        // Check if fragment is repairable (enough holders for k)
        if fragment.holder_count < fragment.threshold {
            warn!(
                content_hash = hex::encode(fragment.content_hash),
                holder_count = fragment.holder_count,
                threshold = fragment.threshold,
                "Fragment cannot be repaired - below threshold"
            );
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_share_index() {
        let service = DiscoveryService::new(DiscoveryConfig::default());

        // With fragment_count=5, should allocate 6
        assert_eq!(service.allocate_share_index(5, &[1, 2, 3, 4, 5]), Some(6));

        // With 6 already used, should allocate 7
        assert_eq!(service.allocate_share_index(5, &[1, 2, 3, 4, 5, 6]), Some(7));

        // With gaps, still allocate next after fragment_count
        assert_eq!(service.allocate_share_index(5, &[1, 3, 5]), Some(6));
    }

    #[test]
    fn test_should_participate_capacity() {
        let service = DiscoveryService::new(DiscoveryConfig::default());
        let fragment = AtRiskFragment {
            content_hash: [1u8; 32],
            commitment: [0u8; 48],
            holder_count: 4,
            threshold: 3,
            fragment_count: 5,
            holders: vec![],
        };

        let our_account = [2u8; 32];

        // Sufficient capacity
        assert!(service.should_participate(&fragment, &our_account, 1000, 100));

        // Insufficient capacity
        assert!(!service.should_participate(&fragment, &our_account, 50, 100));
    }
}

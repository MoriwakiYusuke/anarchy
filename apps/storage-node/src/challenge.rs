//! Challenge Monitoring and Response Module
//!
//! Monitors blockchain for ChallengeIssued events and automatically
//! responds with KZG proofs using the prover module.
//!
//! T041: Implements automatic proof submission via chain client.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, debug, error};
use anyhow::Result;

use crate::chain::ChainClient;
use crate::prover::KzgProver;

/// Challenge data received from blockchain
#[derive(Debug, Clone)]
pub struct Challenge {
    /// Content hash being challenged
    pub content_hash: [u8; 32],
    /// Share index to prove (1-based)
    pub share_index: u8,
    /// Target node account
    pub target_node: [u8; 32],
    /// Deadline block number
    pub deadline: u64,
    /// Block number when issued
    pub issued_at: u64,
}

/// Pending challenge state
#[derive(Debug, Clone)]
pub struct PendingChallenge {
    /// The challenge details
    pub challenge: Challenge,
    /// Number of proof submission attempts
    pub attempts: u32,
    /// Whether proof was successfully submitted
    pub submitted: bool,
}

/// Challenge monitor for responding to holding challenges
pub struct ChallengeMonitor {
    /// Our account ID (for filtering challenges)
    our_account: [u8; 32],
    /// KZG prover instance
    prover: Arc<KzgProver>,
    /// Chain client for submitting proofs (T083)
    chain_client: Option<Arc<ChainClient>>,
    /// Pending challenges awaiting response
    pending: Mutex<HashMap<([u8; 32], u8), PendingChallenge>>,
    /// Maximum proof submission attempts
    max_attempts: u32,
}

impl ChallengeMonitor {
    /// Create a new challenge monitor (without chain client - for testing)
    pub fn new(our_account: [u8; 32], prover: Arc<KzgProver>) -> Self {
        Self {
            our_account,
            prover,
            chain_client: None,
            pending: Mutex::new(HashMap::new()),
            max_attempts: 3,
        }
    }

    /// Create a new challenge monitor with chain client (T083, T041)
    pub fn with_chain_client(
        our_account: [u8; 32],
        prover: Arc<KzgProver>,
        chain_client: Arc<ChainClient>,
    ) -> Self {
        Self {
            our_account,
            prover,
            chain_client: Some(chain_client),
            pending: Mutex::new(HashMap::new()),
            max_attempts: 3,
        }
    }

    /// Process a ChallengeIssued event
    pub async fn on_challenge_issued(&self, challenge: Challenge) -> Result<()> {
        // Check if this challenge is for us
        if challenge.target_node != self.our_account {
            debug!(
                content_hash = hex::encode(challenge.content_hash),
                target = hex::encode(challenge.target_node),
                "Ignoring challenge for another node"
            );
            return Ok(());
        }

        info!(
            content_hash = hex::encode(challenge.content_hash),
            share_index = challenge.share_index,
            deadline = challenge.deadline,
            "Received challenge, queueing proof generation"
        );

        // Add to pending challenges
        let key = (challenge.content_hash, challenge.share_index);
        let pending_challenge = PendingChallenge {
            challenge: challenge.clone(),
            attempts: 0,
            submitted: false,
        };

        {
            let mut pending = self.pending.lock().await;
            pending.insert(key, pending_challenge);
        }

        // Attempt to generate and submit proof immediately
        self.try_submit_proof(challenge.content_hash, challenge.share_index).await
    }

    /// Try to generate and submit a proof for a pending challenge
    async fn try_submit_proof(&self, content_hash: [u8; 32], share_index: u8) -> Result<()> {
        let key = (content_hash, share_index);

        // Get pending challenge (currently unused until chain submission is implemented)
        let _pending_challenge = {
            let mut pending = self.pending.lock().await;
            match pending.get_mut(&key) {
                Some(pc) => {
                    if pc.submitted {
                        debug!("Proof already submitted for this challenge");
                        return Ok(());
                    }
                    if pc.attempts >= self.max_attempts {
                        warn!(
                            attempts = pc.attempts,
                            "Max proof submission attempts reached"
                        );
                        return Ok(());
                    }
                    pc.attempts += 1;
                    pc.challenge.clone()
                }
                None => {
                    warn!("Challenge not found in pending map");
                    return Ok(());
                }
            }
        }; // Drop lock before async operations

        // Generate proof
        match self.prover.generate_proof_for_challenge(
            &content_hash,
            share_index,
        ).await {
            Ok((share_value, proof)) => {
                info!(
                    content_hash = hex::encode(content_hash),
                    share_index = share_index,
                    "Generated KZG proof, submitting to chain"
                );

                // T041: Submit proof to chain via prove_holding_kzg extrinsic
                let submission_result = if let Some(ref chain_client) = self.chain_client {
                    chain_client.submit_holding_proof(
                        content_hash,
                        share_index,
                        share_value,
                        proof,
                    ).await
                } else {
                    // No chain client - mark as submitted (stub for testing)
                    warn!("No chain client configured, proof submission skipped");
                    Ok(())
                };

                match submission_result {
                    Ok(()) => {
                        let mut pending = self.pending.lock().await;
                        if let Some(pc) = pending.get_mut(&key) {
                            pc.submitted = true;
                        }
                        info!(
                            content_hash = hex::encode(content_hash),
                            "Proof submission successful"
                        );
                        Ok(())
                    }
                    Err(e) => {
                        error!(
                            content_hash = hex::encode(content_hash),
                            error = %e,
                            "Failed to submit proof to chain"
                        );
                        Err(e)
                    }
                }
            }
            Err(e) => {
                error!(
                    content_hash = hex::encode(content_hash),
                    share_index = share_index,
                    error = %e,
                    "Failed to generate KZG proof"
                );
                Err(e)
            }
        }
    }

    /// Get count of pending challenges
    pub async fn pending_count(&self) -> usize {
        self.pending.lock().await.len()
    }

    /// Get count of unanswered challenges (submitted = false)
    pub async fn unanswered_count(&self) -> usize {
        self.pending.lock().await
            .values()
            .filter(|pc| !pc.submitted)
            .count()
    }

    /// Clear submitted challenges older than given block
    pub async fn cleanup_old_challenges(&self, current_block: u64) {
        let mut pending = self.pending.lock().await;
        pending.retain(|_, pc| {
            // Keep if not submitted or if deadline hasn't passed
            !pc.submitted && pc.challenge.deadline > current_block
        });
    }

    /// Process pending challenges that haven't been submitted yet (T047)
    /// 
    /// Retries proof generation and submission for any pending challenges
    /// that haven't exceeded max_attempts.
    pub async fn process_pending(&self) -> Result<()> {
        // Get list of pending challenges that need retry
        let pending_keys: Vec<([u8; 32], u8)> = {
            let pending = self.pending.lock().await;
            pending.iter()
                .filter(|(_, pc)| !pc.submitted && pc.attempts < self.max_attempts)
                .map(|(key, _)| key.clone())
                .collect()
        };

        // Retry each pending challenge
        for (content_hash, share_index) in pending_keys {
            if let Err(e) = self.try_submit_proof(content_hash, share_index).await {
                debug!(
                    content_hash = hex::encode(content_hash),
                    share_index = share_index,
                    error = %e,
                    "Retry proof submission failed"
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_challenge() -> Challenge {
        Challenge {
            content_hash: [1u8; 32],
            share_index: 1,
            target_node: [42u8; 32],
            deadline: 200,
            issued_at: 100,
        }
    }

    #[tokio::test]
    async fn test_ignores_challenge_for_other_node() {
        let prover = Arc::new(KzgProver::new());
        let monitor = ChallengeMonitor::new([99u8; 32], prover); // Different account

        let challenge = make_test_challenge();
        monitor.on_challenge_issued(challenge).await.unwrap();

        // Should not be added to pending
        assert_eq!(monitor.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_accepts_challenge_for_our_node() {
        let prover = Arc::new(KzgProver::new());
        let monitor = ChallengeMonitor::new([42u8; 32], prover); // Our account

        let challenge = make_test_challenge();
        // Note: This will fail at proof generation since we don't have the share data
        let _ = monitor.on_challenge_issued(challenge).await;

        // Should be added to pending (even if proof generation fails)
        assert_eq!(monitor.pending_count().await, 1);
    }
}

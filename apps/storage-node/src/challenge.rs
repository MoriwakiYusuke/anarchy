//! Challenge Monitoring and Response Module
//!
//! Monitors blockchain for ChallengeIssued events and automatically
//! responds with KZG proofs using the prover module.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, debug, error};
use anyhow::Result;

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
    /// Pending challenges awaiting response
    pending: Mutex<HashMap<([u8; 32], u8), PendingChallenge>>,
    /// Maximum proof submission attempts
    max_attempts: u32,
}

impl ChallengeMonitor {
    /// Create a new challenge monitor
    pub fn new(our_account: [u8; 32], prover: Arc<KzgProver>) -> Self {
        Self {
            our_account,
            prover,
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
            Ok((_share_value, _proof)) => {
                info!(
                    content_hash = hex::encode(content_hash),
                    share_index = share_index,
                    "Generated KZG proof, submitting to chain"
                );

                // TODO (T041): Submit proof to chain via prove_holding_kzg extrinsic
                // For now, mark as submitted (stub)
                let mut pending = self.pending.lock().await;
                if let Some(pc) = pending.get_mut(&key) {
                    pc.submitted = true;
                }

                info!("Proof submission successful (stub)");
                Ok(())
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

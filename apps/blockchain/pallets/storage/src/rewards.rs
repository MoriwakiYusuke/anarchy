//! Reward Calculation and Distribution Module
//!
//! Handles reward calculation based on data size and distribution to storage nodes.

/// Minimum score threshold for rewards (FR-109)
/// Content below this score receives 0 rewards
pub const SCORE_THRESHOLD: u64 = 100;

/// Calculate reward for a successful holding proof.
///
/// Formula: base_reward_per_byte × data_size
///
/// Note: If `score` is less than [`SCORE_THRESHOLD`], this function returns `0`
/// and no rewards are distributed. This is used by the GC system to mark
/// low-score content as forgetting candidates.
///
/// # Arguments
/// * `data_size` - Size of the content in bytes
/// * `base_reward_per_byte` - Base reward per byte configured in pallet
/// * `score` - Current content score (from external scorer)
///
/// # Returns
/// * Reward amount (0 if `score` is below [`SCORE_THRESHOLD`])
pub fn calculate_reward(data_size: u32, base_reward_per_byte: u128, score: u64) -> u128 {
    calculate_reward_with_threshold(data_size, base_reward_per_byte, score, SCORE_THRESHOLD)
}

/// Calculate reward with configurable threshold (T059).
///
/// # Arguments
/// * `data_size` - Size of the content in bytes
/// * `base_reward_per_byte` - Base reward per byte configured in pallet
/// * `score` - Current content score (from external scorer)
/// * `threshold` - Minimum score for reward eligibility
///
/// # Returns
/// * Reward amount (0 if score below threshold)
pub fn calculate_reward_with_threshold(
    data_size: u32,
    base_reward_per_byte: u128,
    score: u64,
    threshold: u64,
) -> u128 {
    // Check score threshold
    if score < threshold {
        return 0;
    }

    // Calculate reward: base_reward_per_byte × data_size
    base_reward_per_byte.saturating_mul(data_size as u128)
}

/// Calculate pro-rata distribution when pool is exhausted.
///
/// # Arguments
/// * `pending_amounts` - Vector of (account, pending_reward)
/// * `pool_balance` - Available pool balance
///
/// # Returns
/// * Vector of (account, actual_payout)
#[cfg(test)]
pub fn calculate_pro_rata<AccountId: Clone>(
    pending_amounts: &[(AccountId, u128)],
    pool_balance: u128,
) -> Vec<(AccountId, u128)> {
    let total_pending: u128 = pending_amounts
        .iter()
        .map(|(_, amount)| *amount)
        .sum();

    if total_pending == 0 {
        return vec![];
    }

    if pool_balance >= total_pending {
        // Pool has enough - pay full amounts
        return pending_amounts.to_vec();
    }

    // Pro-rata distribution
    pending_amounts
        .iter()
        .map(|(account, amount)| {
            let share = (*amount).saturating_mul(pool_balance) / total_pending;
            (account.clone(), share)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_reward_above_threshold() {
        let data_size = 1024; // 1KB
        let base_reward_per_byte = 1_000_000; // 1 micro-MORAL per byte
        let score = 150; // Above threshold

        let reward = calculate_reward(data_size, base_reward_per_byte, score);
        assert_eq!(reward, 1024 * 1_000_000); // 1024 micro-MORAL
    }

    #[test]
    fn test_calculate_reward_below_threshold() {
        let data_size = 1024;
        let base_reward_per_byte = 1_000_000;
        let score = 50; // Below threshold

        let reward = calculate_reward(data_size, base_reward_per_byte, score);
        assert_eq!(reward, 0);
    }

    #[test]
    fn test_calculate_reward_at_threshold() {
        let data_size = 1024;
        let base_reward_per_byte = 1_000_000;
        let score = 100; // At threshold

        let reward = calculate_reward(data_size, base_reward_per_byte, score);
        assert_eq!(reward, 1024 * 1_000_000); // Should get reward
    }

    #[test]
    fn test_pro_rata_full_pool() {
        let pending = vec![
            (1u64, 100u128),
            (2u64, 200u128),
        ];
        let pool = 500; // More than enough

        let result = calculate_pro_rata(&pending, pool);
        assert_eq!(result, vec![(1, 100), (2, 200)]);
    }

    #[test]
    fn test_pro_rata_exhausted_pool() {
        let pending = vec![
            (1u64, 60u128),
            (2u64, 60u128),
        ];
        let pool = 100; // Less than total (120)

        let result = calculate_pro_rata(&pending, pool);
        // Each should get 60/120 * 100 = 50
        assert_eq!(result, vec![(1, 50), (2, 50)]);
    }

    #[test]
    fn test_pro_rata_empty() {
        let pending: Vec<(u64, u128)> = vec![];
        let pool = 100;

        let result = calculate_pro_rata(&pending, pool);
        assert!(result.is_empty());
    }
}

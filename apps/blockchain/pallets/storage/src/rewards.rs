//! Reward Calculation and Distribution Module
//!
//! Handles reward calculation based on data size and distribution to storage nodes.

/// Minimum score threshold for rewards (FR-109).
/// Content below this score receives 0 rewards.
///
/// Note: This constant serves as the default fallback value. The pallet's `Config::ScoreThreshold`
/// type should be used for runtime configuration. This constant is provided for use in tests
/// and contexts where the runtime config is not available.
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

/// Calculate reward with TSTS v1 dynamics (P3): pool ratio decay.
///
/// 旧 `calculate_reward_with_threshold` を `pool_balance / pool_target` で線形に減衰させる。
/// プール残高がターゲット以上なら 100% 支払い、半分なら 50% など。
/// プール枯渇時に支払いが急に途切れて怠惰行動が支配戦略にならないようにする。
///
/// **将来の P4 Storage stake で `√(bond_share)` 補正をここに掛ける**。bond_share は
/// `(node_bond / total_active_bond)^0.5` で、quadratic Sybil resistance を実現する。
///
/// # Arguments
/// * `data_size` - Size of the content in bytes
/// * `base_reward_per_byte` - Base reward per byte configured in pallet
/// * `score` - Current content score (from external scorer)
/// * `threshold` - Minimum score for reward eligibility
/// * `pool_balance` - Current σ_storage balance (u128 units)
/// * `pool_target` - Target σ_storage balance (governance-tunable)
///
/// # Returns
/// * Reward amount (0 if score below threshold or pool empty)
pub fn calculate_reward_v2(
    data_size: u32,
    base_reward_per_byte: u128,
    score: u64,
    threshold: u64,
    pool_balance: u128,
    pool_target: u128,
) -> u128 {
    if score < threshold {
        return 0;
    }
    let base = base_reward_per_byte.saturating_mul(data_size as u128);

    // pool_target=0 を「補正無効 (旧挙動互換)」と解釈する。
    if pool_target == 0 {
        return base;
    }

    // 線形 decay: pool_ratio_ppm = min(1_000_000, pool_balance × 1e6 / pool_target)
    let pool_ratio_ppm = if pool_balance >= pool_target {
        1_000_000u128
    } else {
        pool_balance.saturating_mul(1_000_000) / pool_target.max(1)
    };

    // base × pool_ratio_ppm / 1e6 (saturating)
    base.saturating_mul(pool_ratio_ppm) / 1_000_000
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
    fn calculate_reward_v2_full_pool_returns_base() {
        // pool_balance >= pool_target → 100% payout
        let r = calculate_reward_v2(1024, 1_000_000, 150, 100, 1_000_000, 500_000);
        assert_eq!(r, 1024 * 1_000_000);
    }

    #[test]
    fn calculate_reward_v2_half_pool_halves_payout() {
        // pool_balance = pool_target / 2 → 50% payout
        let r = calculate_reward_v2(1024, 1_000_000, 150, 100, 250_000, 500_000);
        assert_eq!(r, 1024 * 1_000_000 / 2);
    }

    #[test]
    fn calculate_reward_v2_empty_pool_zero_payout() {
        // pool_balance = 0 → 0 payout
        let r = calculate_reward_v2(1024, 1_000_000, 150, 100, 0, 500_000);
        assert_eq!(r, 0);
    }

    #[test]
    fn calculate_reward_v2_below_threshold_zero() {
        // score < threshold → 0 (pool ratio 関係なし)
        let r = calculate_reward_v2(1024, 1_000_000, 50, 100, 1_000_000, 500_000);
        assert_eq!(r, 0);
    }

    #[test]
    fn calculate_reward_v2_zero_target_disables_decay() {
        // pool_target = 0 → 補正無効、旧挙動と同じ
        let r = calculate_reward_v2(1024, 1_000_000, 150, 100, 0, 0);
        assert_eq!(r, 1024 * 1_000_000);
    }

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

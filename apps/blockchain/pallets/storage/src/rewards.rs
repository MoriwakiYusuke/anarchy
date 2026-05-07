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

/// Calculate reward with TSTS v1 dynamics (P3): pool ratio decay (bond-share 補正なし).
///
/// 後続 (TSTS F1) で bond_share √ 補正を入れた `calculate_reward_v3` も追加した。
/// 互換性のため v2 はそのまま残し、bond 機能を使わない既存テストはこちらを呼べる。
///
/// `calculate_reward_v3(..., 0, 0)` と等価。
pub fn calculate_reward_v2(
    data_size: u32,
    base_reward_per_byte: u128,
    score: u64,
    threshold: u64,
    pool_balance: u128,
    pool_target: u128,
) -> u128 {
    calculate_reward_v3(
        data_size,
        base_reward_per_byte,
        score,
        threshold,
        pool_balance,
        pool_target,
        0,  // node_bond = 0 → bond 補正無効
        0,  // total_active_bond = 0 → 同上
    )
}

/// Calculate reward with TSTS F1 full dynamics: pool ratio decay × √(bond_share).
///
/// `√(bond_share)` の意味: ノードの bond が全 active bond のうち占める割合の平方根。
/// quadratic Sybil resistance を実現する。N 個の Sybil ノードに分散しても
/// `Σ √(b_i) ≤ √(Σ b_i)` (Jensen) なので 1 ノード集中より報酬総和が **減る**。
/// 巨大プレイヤー独占の場合も √ で薄まるため寡占を抑制する。
///
/// `total_active_bond == 0` のときは「bond 機能未確立」と解釈し、bond 補正を 1.0 にする
/// (= pool_ratio のみで動く、TSTS P3 互換挙動)。
///
/// # Arguments
/// * `data_size` - Size of the content in bytes
/// * `base_reward_per_byte` - Base reward per byte configured in pallet
/// * `score` - Current content score (from external scorer)
/// * `threshold` - Minimum score for reward eligibility
/// * `pool_balance` - Current σ_storage balance (u128 units)
/// * `pool_target` - Target σ_storage balance (governance-tunable)
/// * `node_bond` - Storage node の bond 額 (u128 units)
/// * `total_active_bond` - 全ノードの bond 合計 (u128 units)
///
/// # Returns
/// * Reward amount (0 if score below threshold or pool empty)
pub fn calculate_reward_v3(
    data_size: u32,
    base_reward_per_byte: u128,
    score: u64,
    threshold: u64,
    pool_balance: u128,
    pool_target: u128,
    node_bond: u128,
    total_active_bond: u128,
) -> u128 {
    if score < threshold {
        return 0;
    }
    let mut reward = base_reward_per_byte.saturating_mul(data_size as u128);

    // (1) pool_ratio decay (TSTS P3): pool_target=0 を「補正無効」と解釈
    if pool_target > 0 {
        let pool_ratio_ppm = if pool_balance >= pool_target {
            1_000_000u128
        } else {
            pool_balance.saturating_mul(1_000_000) / pool_target.max(1)
        };
        reward = reward.saturating_mul(pool_ratio_ppm) / 1_000_000;
    }

    // (2) √(bond_share) 補正 (TSTS F1): total_active_bond=0 を「補正無効」と解釈
    if total_active_bond > 0 && node_bond > 0 {
        // bond_share_ppm = node_bond / total_active_bond × 1_000_000
        let bond_share_ppm = node_bond
            .saturating_mul(1_000_000)
            / total_active_bond.max(1);
        // sqrt_ppm: x_ppm の平方根を ppm 単位で返す
        let sqrt_share_ppm = sqrt_ppm(bond_share_ppm);
        reward = reward.saturating_mul(sqrt_share_ppm) / 1_000_000;
    }

    reward
}

/// Newton iteration による整数 sqrt for ppm representation.
///
/// 入力: x_ppm = x × 1e6 として、戻り値は sqrt(x) × 1e6.
/// 例: x_ppm=1_000_000 (= x=1.0) → 戻り値 1_000_000.
///     x_ppm=  250_000 (= x=0.25) → 戻り値 500_000 (= sqrt(0.25)).
pub fn sqrt_ppm(x_ppm: u128) -> u128 {
    let scaled = x_ppm.saturating_mul(1_000_000);
    if scaled == 0 {
        return 0;
    }
    let mut z = (scaled + 1) / 2;
    let mut y = scaled;
    // Newton method: 高々 ~64 反復で収束
    while z < y {
        y = z;
        z = (scaled / z + z) / 2;
    }
    y
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

    // ─── TSTS F1: calculate_reward_v3 (with √(bond_share)) ─────────────────────

    #[test]
    fn calculate_reward_v3_sole_node_full_share() {
        // node_bond = total_active_bond → bond_share = 1.0 → sqrt = 1.0 → 100%
        let r = calculate_reward_v3(1024, 1_000_000, 150, 100, 1_000_000, 500_000, 1000, 1000);
        assert_eq!(r, 1024 * 1_000_000);
    }

    #[test]
    fn calculate_reward_v3_quarter_bond_returns_half() {
        // bond_share = 0.25 → sqrt = 0.5 → 50% payout
        let r = calculate_reward_v3(1024, 1_000_000, 150, 100, 1_000_000, 500_000, 250, 1000);
        // 1024 × 1_000_000 = 1_024_000_000 base; × 0.5 = 512_000_000 (整数 sqrt の誤差 ±1)
        let expected = 512_000_000u128;
        assert!((r as i128 - expected as i128).abs() <= 2, "got {}", r);
    }

    #[test]
    fn calculate_reward_v3_zero_total_bond_disables_correction() {
        // total_active_bond = 0 → bond 機能未確立、pool_ratio のみで動く
        let r = calculate_reward_v3(1024, 1_000_000, 150, 100, 1_000_000, 500_000, 0, 0);
        assert_eq!(r, 1024 * 1_000_000); // pool_ratio = 1.0 (full pool) なのでそのまま
    }

    #[test]
    fn calculate_reward_v3_combines_pool_and_bond() {
        // pool_ratio = 0.5, bond_share = 0.25 → 0.5 × sqrt(0.25) = 0.5 × 0.5 = 0.25
        let r = calculate_reward_v3(1024, 1_000_000, 150, 100, 250_000, 500_000, 250, 1000);
        let expected = 1024 * 1_000_000 / 4; // 256_000_000
        assert!((r as i128 - expected as i128).abs() <= 4, "got {}", r);
    }

    #[test]
    fn sqrt_ppm_basic_cases() {
        assert_eq!(sqrt_ppm(0), 0);
        assert_eq!(sqrt_ppm(1_000_000), 1_000_000);                    // sqrt(1) = 1
        let r25 = sqrt_ppm(250_000);
        assert!((r25 as i128 - 500_000).abs() <= 1, "got {}", r25);    // sqrt(0.25) = 0.5
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

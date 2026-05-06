//! LWMA-3 difficulty adjustment algorithm (Monero / Kulupu 流派)。
//!
//! 参考: https://github.com/zawy12/difficulty-algorithms/issues/3
//!
//! 計算式 (window 長 N):
//!   weight_i      = i               (i = 1..=N)
//!   solve_time_i  = clamp(ts_i - ts_{i-1}, 1, 6 * target)
//!   weighted_solve_sum = Σ (weight_i * solve_time_i)
//!   weighted_target_sum = N * (N+1) / 2 * target
//!   harmonic_mean_diff = N / Σ (1 / diff_i)
//!   next_diff = harmonic_mean_diff * weighted_target_sum / weighted_solve_sum

use sp_core::U256;

/// `window` は `(difficulty, timestamp_ms)` の昇順 (古→新) スライス。
/// 長さは N >= 2 を前提 (N == 1 は呼び出し側でガード)。
pub fn lwma3_next_difficulty<T>(window: &[(U256, T)], target_ms: u64) -> U256
where
    T: Copy + TryInto<u64>,
{
    let n = window.len();
    if n < 2 {
        return window.last().map(|(d, _)| *d).unwrap_or(U256::one());
    }

    let target = U256::from(target_ms);
    let max_solve = 6u64.saturating_mul(target_ms);

    let mut weighted_solve_sum: U256 = U256::zero();
    let mut sum_inverse_diff: U256 = U256::zero();

    for i in 1..n {
        let (_, prev_ts) = window[i - 1];
        let (diff_i, ts_i) = window[i];
        let prev_ms: u64 = prev_ts.try_into().ok().unwrap_or(0);
        let cur_ms: u64 = ts_i.try_into().ok().unwrap_or(0);
        let raw_solve = cur_ms.saturating_sub(prev_ms).max(1);
        let solve = raw_solve.min(max_solve);
        let weight = U256::from(i as u64);
        weighted_solve_sum = weighted_solve_sum.saturating_add(weight * U256::from(solve));
        if !diff_i.is_zero() {
            sum_inverse_diff =
                sum_inverse_diff.saturating_add(U256::MAX / diff_i / U256::from(n as u64));
        }
    }

    if sum_inverse_diff.is_zero() {
        return window.last().map(|(d, _)| *d).unwrap_or(U256::one());
    }
    let harmonic_mean = U256::MAX / sum_inverse_diff;

    let weighted_target_sum = target * U256::from((n as u64) * ((n as u64) + 1) / 2);
    if weighted_solve_sum.is_zero() {
        return harmonic_mean;
    }
    harmonic_mean.saturating_mul(weighted_target_sum) / weighted_solve_sum
}

//! Pure relative-decay function — easy to unit-test in isolation.

use sp_runtime::Permill;

/// Apply `score *= decay_rate ^ delta_blocks`, clamped to `max_steps` exponent.
///
/// `decay_rate` is a `Permill` (out of 1_000_000). `999_950` ≈ 0.99995 per block.
///
/// Implementation: exponentiation by squaring in u128 fixed-point with denominator
/// `BASE = 10^18`. Runtime is O(log steps) instead of O(steps), so even pathologically
/// large `delta_blocks` values stay cheap (≤ 32 u128 mul/divs for any u32 exponent).
pub fn apply(score: u64, delta_blocks: u32, decay_rate: Permill, max_steps: u32) -> u64 {
    let steps = delta_blocks.min(max_steps);
    if steps == 0 || score == 0 {
        return score;
    }
    let rate_parts = decay_rate.deconstruct();
    // Permill::one() = 1_000_000 → no decay; Permill::zero() → instant zero.
    if rate_parts == 1_000_000 {
        return score;
    }
    if rate_parts == 0 {
        return 0;
    }

    // Fixed-point base: 10^18 ≈ 2^60. Squaring stays under 2^120 (u128-safe).
    const BASE: u128 = 1_000_000_000_000_000_000;
    const SCALE: u128 = 1_000_000_000_000; // BASE / 1_000_000

    let mut base = (rate_parts as u128) * SCALE; // rate in BASE-fixed-point
    let mut result: u128 = BASE;                  // 1.0 in BASE-fixed-point
    let mut n = steps;

    while n > 0 {
        if n & 1 == 1 {
            result = result.saturating_mul(base) / BASE;
            if result == 0 {
                return 0;
            }
        }
        n >>= 1;
        if n > 0 {
            base = base.saturating_mul(base) / BASE;
            // If `base` truncates to 0 while bits remain in `n`, at least one future
            // iteration will multiply `result` by 0 — answer collapses to 0.
            if base == 0 {
                return 0;
            }
        }
    }

    let new_score = (score as u128).saturating_mul(result) / BASE;
    new_score as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_zero_returns_score_unchanged() {
        assert_eq!(apply(123, 0, Permill::from_parts(999_950), 1_000_000), 123);
    }

    #[test]
    fn score_zero_returns_zero() {
        assert_eq!(apply(0, 100, Permill::from_parts(999_950), 1_000_000), 0);
    }

    #[test]
    fn known_decay_roughly_matches_expectation() {
        // 100_000 * 0.99995^200 ≈ 99_004.9 (decays ~1% over 200 blocks)
        let result = apply(100_000, 200, Permill::from_parts(999_950), 1_000_000);
        assert!(result >= 98_900 && result <= 99_100, "got {}", result);
    }

    #[test]
    fn delta_clamped_by_max_steps() {
        // Big delta but max_steps=10 caps the iteration.
        let with_clamp = apply(1_000_000, 1_000_000, Permill::from_parts(999_950), 10);
        // 0.99995^10 ≈ 0.9995
        assert!(with_clamp >= 999_400 && with_clamp <= 999_600, "got {}", with_clamp);
    }

    #[test]
    fn very_long_delta_drives_score_to_zero() {
        // With rate 0.99995 and 500_000 blocks, score should be near zero.
        let r = apply(100_000, 500_000, Permill::from_parts(999_950), 1_000_000);
        assert!(r < 100, "got {}", r);
    }

    #[test]
    fn max_score_does_not_overflow() {
        // u64::MAX through one tick must not panic
        let _ = apply(u64::MAX, 1, Permill::from_parts(999_950), 1_000_000);
    }
}

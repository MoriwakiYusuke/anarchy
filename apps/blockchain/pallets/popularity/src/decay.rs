//! Pure relative-decay function — easy to unit-test in isolation.

use sp_runtime::Permill;

/// Apply `score *= decay_rate ^ delta_blocks`, clamped to `max_steps` iterations.
///
/// `decay_rate` is a `Permill` (out of 1_000_000). `999_950` ≈ 0.99995 per block.
pub fn apply(score: u64, delta_blocks: u32, decay_rate: Permill, max_steps: u32) -> u64 {
    let steps = delta_blocks.min(max_steps);
    if steps == 0 || score == 0 {
        return score;
    }
    let rate = decay_rate.deconstruct() as u128;
    let mut s = score as u128;
    for _ in 0..steps {
        s = s.saturating_mul(rate) / 1_000_000;
        if s == 0 {
            return 0;
        }
    }
    s as u64
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

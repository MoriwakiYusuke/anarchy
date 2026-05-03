//! Unit tests for pallet-popularity score logic.

use crate::{
    mock::*,
    pallet::{DeletionQueue, PostScores},
    PopularityInterface, PopularityReactionType,
};

// `mock::*` already brings in `deleted_posts`, `released_hashes`, `reset_deletion_trackers`
// via the glob import above.

#[test]
fn on_post_created_inserts_initial_score() {
    new_test_ext().execute_with(|| {
        run_to_block(5);
        Popularity::on_post_created(42);
        let p = PostScores::<Test>::get(42).expect("entry");
        assert_eq!(p.stored_score, 10_000);
        assert_eq!(p.last_touched, 5);
        assert_eq!(p.like_count, 0);
        assert_eq!(p.dislike_count, 0);
        assert!(p.marked_for_deletion_at.is_none());
    });
}

#[test]
fn on_reaction_like_bumps_score_and_count() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        Popularity::on_post_created(7);
        run_to_block(1); // delta=0, no decay
        Popularity::on_reaction(7, PopularityReactionType::Like);
        let p = PostScores::<Test>::get(7).expect("entry");
        assert_eq!(p.like_count, 1);
        assert_eq!(p.dislike_count, 0);
        assert_eq!(p.stored_score, 10_000 + 100);
    });
}

#[test]
fn on_reaction_dislike_bumps_score_and_count() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        Popularity::on_post_created(7);
        Popularity::on_reaction(7, PopularityReactionType::Dislike);
        let p = PostScores::<Test>::get(7).expect("entry");
        assert_eq!(p.like_count, 0);
        assert_eq!(p.dislike_count, 1);
        assert_eq!(p.stored_score, 10_000 + 50);
    });
}

#[test]
fn on_reaction_applies_decay_before_adding_delta() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        Popularity::on_post_created(7);
        run_to_block(101); // 100 blocks elapsed
        Popularity::on_reaction(7, PopularityReactionType::Like);
        let p = PostScores::<Test>::get(7).expect("entry");
        // 10_000 * 0.999^100 ≈ 9_047.9, then + 100 = ~9_147
        assert!(p.stored_score < 10_000);
        assert!(p.stored_score >= 9_000);
        assert_eq!(p.last_touched, 101);
    });
}

#[test]
fn on_reaction_unmarks_when_above_recovery_threshold() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        Popularity::on_post_created(7);
        // Manually mark and queue, simulating prior on_finalize.
        PostScores::<Test>::mutate(7, |e| {
            let p = e.as_mut().unwrap();
            p.stored_score = 800; // below threshold (1000)
            p.marked_for_deletion_at = Some(1);
        });
        DeletionQueue::<Test>::insert(7u64, 11u64);

        // A flurry of likes pushes score above threshold + hysteresis (1500).
        run_to_block(2);
        for _ in 0..10 {
            // 800 + 10*100 = 1800 > 1500
            Popularity::on_reaction(7, PopularityReactionType::Like);
        }
        let p = PostScores::<Test>::get(7).unwrap();
        assert!(p.marked_for_deletion_at.is_none());
        assert!(DeletionQueue::<Test>::get(7).is_none());
    });
}

#[test]
fn on_reaction_keeps_mark_when_below_recovery() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        Popularity::on_post_created(7);
        PostScores::<Test>::mutate(7, |e| {
            let p = e.as_mut().unwrap();
            p.stored_score = 800;
            p.marked_for_deletion_at = Some(1);
        });
        DeletionQueue::<Test>::insert(7u64, 11u64);

        // Just one like → 900, still below 1500 recovery.
        Popularity::on_reaction(7, PopularityReactionType::Like);
        let p = PostScores::<Test>::get(7).unwrap();
        assert!(p.marked_for_deletion_at.is_some());
        assert!(DeletionQueue::<Test>::get(7).is_some());
    });
}

use crate::pallet::ScanCursor;

#[test]
fn on_finalize_marks_post_below_threshold() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        Popularity::on_post_created(0);
        set_max_post_id(1);
        // Push score artificially below threshold (1000)
        crate::pallet::PostScores::<Test>::mutate(0, |e| {
            e.as_mut().unwrap().stored_score = 500;
        });

        Popularity::run_scan_pass(2);

        let p = crate::pallet::PostScores::<Test>::get(0).unwrap();
        assert_eq!(p.marked_for_deletion_at, Some(2));
        assert_eq!(crate::pallet::DeletionQueue::<Test>::get(0), Some(2 + 10));
    });
}

#[test]
fn on_finalize_unmarks_post_above_recovery() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        Popularity::on_post_created(0);
        set_max_post_id(1);
        crate::pallet::PostScores::<Test>::mutate(0, |e| {
            let p = e.as_mut().unwrap();
            p.stored_score = 800;
            p.marked_for_deletion_at = Some(1);
        });
        crate::pallet::DeletionQueue::<Test>::insert(0u64, 11u64);

        // Score climbs above recovery (1500)
        crate::pallet::PostScores::<Test>::mutate(0, |e| {
            e.as_mut().unwrap().stored_score = 2_000;
        });

        Popularity::run_scan_pass(2);
        let p = crate::pallet::PostScores::<Test>::get(0).unwrap();
        assert!(p.marked_for_deletion_at.is_none());
        assert!(crate::pallet::DeletionQueue::<Test>::get(0).is_none());
    });
}

#[test]
fn on_finalize_does_not_unmark_within_hysteresis_band() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        Popularity::on_post_created(0);
        set_max_post_id(1);
        crate::pallet::PostScores::<Test>::mutate(0, |e| {
            let p = e.as_mut().unwrap();
            p.stored_score = 1_200; // > threshold (1000) but < recovery (1500)
            p.marked_for_deletion_at = Some(1);
        });
        crate::pallet::DeletionQueue::<Test>::insert(0u64, 11u64);

        Popularity::run_scan_pass(2);
        let p = crate::pallet::PostScores::<Test>::get(0).unwrap();
        assert!(p.marked_for_deletion_at.is_some(), "should remain marked in hysteresis band");
    });
}

#[test]
fn on_finalize_respects_max_posts_scanned() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        for id in 0..10u64 {
            Popularity::on_post_created(id);
        }
        set_max_post_id(10);

        Popularity::run_scan_pass(2);
        // Mock has MaxPostsScannedPerBlock = 4, so cursor should be 4.
        assert_eq!(ScanCursor::<Test>::get(), 4);
    });
}

#[test]
fn on_finalize_cursor_wraps_around() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        for id in 0..3u64 {
            Popularity::on_post_created(id);
        }
        set_max_post_id(3);
        ScanCursor::<Test>::put(2u64);

        Popularity::run_scan_pass(2);
        // The property: cursor stays in [0, max_post_id) after the pass.
        let cursor = ScanCursor::<Test>::get();
        assert!(cursor < 3, "cursor should wrap, got {}", cursor);
    });
}

#[test]
fn deletion_pass_removes_eligible_posts() {
    new_test_ext().execute_with(|| {
        reset_deletion_trackers();
        run_to_block(1);
        Popularity::on_post_created(0);
        set_max_post_id(1);

        // Manually mark + queue with eligible_at = 5
        crate::pallet::PostScores::<Test>::mutate(0, |e| {
            e.as_mut().unwrap().marked_for_deletion_at = Some(1);
        });
        crate::pallet::DeletionQueue::<Test>::insert(0u64, 5u64);

        run_to_block(5);
        Popularity::run_deletion_pass(5);

        assert!(crate::pallet::PostScores::<Test>::get(0).is_none());
        assert!(crate::pallet::DeletionQueue::<Test>::get(0).is_none());
        assert_eq!(deleted_posts(), vec![0]);
        assert_eq!(released_hashes().len(), 1);
    });
}

#[test]
fn deletion_pass_skips_posts_within_grace_period() {
    new_test_ext().execute_with(|| {
        reset_deletion_trackers();
        run_to_block(1);
        Popularity::on_post_created(0);
        set_max_post_id(1);
        crate::pallet::DeletionQueue::<Test>::insert(0u64, 100u64);

        Popularity::run_deletion_pass(50);
        assert!(crate::pallet::PostScores::<Test>::get(0).is_some());
        assert_eq!(crate::pallet::DeletionQueue::<Test>::get(0), Some(100));
        assert!(deleted_posts().is_empty());
    });
}

#[test]
fn deletion_pass_respects_max_deletions_per_block() {
    new_test_ext().execute_with(|| {
        reset_deletion_trackers();
        run_to_block(1);
        for id in 0..5u64 {
            Popularity::on_post_created(id);
            crate::pallet::DeletionQueue::<Test>::insert(id, 5u64);
        }
        set_max_post_id(5);

        run_to_block(5);
        Popularity::run_deletion_pass(5);
        // Mock has MaxDeletionsPerBlock = 2.
        assert_eq!(deleted_posts().len(), 2);
    });
}

// ===========================================================================
// Edge cases (review-pass M5 + I3)
// ===========================================================================

#[test]
fn scan_pass_handles_score_already_zero() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        Popularity::on_post_created(0);
        set_max_post_id(1);
        // Force the stored_score to 0 (simulating a post that fully decayed already).
        crate::pallet::PostScores::<Test>::mutate(0, |e| {
            e.as_mut().unwrap().stored_score = 0;
        });

        // Should not panic, must mark for deletion (0 < threshold=1000).
        Popularity::run_scan_pass(2);

        let p = crate::pallet::PostScores::<Test>::get(0).unwrap();
        assert_eq!(p.stored_score, 0, "decay of 0 stays 0");
        assert_eq!(p.marked_for_deletion_at, Some(2));
        assert_eq!(crate::pallet::DeletionQueue::<Test>::get(0), Some(2 + 10));
    });
}

#[test]
fn on_post_created_twice_resets_to_initial_state() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        Popularity::on_post_created(7);

        // Mutate to a non-default state.
        crate::pallet::PostScores::<Test>::mutate(7, |e| {
            let p = e.as_mut().unwrap();
            p.stored_score = 500;
            p.like_count = 5;
            p.dislike_count = 3;
            p.marked_for_deletion_at = Some(1);
        });

        // Defensive re-creation. Production won't hit this (post_id is unique),
        // but if it ever does we want a clean reset, not a stale record.
        run_to_block(10);
        Popularity::on_post_created(7);

        let p = crate::pallet::PostScores::<Test>::get(7).expect("entry");
        assert_eq!(p.stored_score, 10_000, "InitialScore reset");
        assert_eq!(p.like_count, 0);
        assert_eq!(p.dislike_count, 0);
        assert_eq!(p.last_touched, 10);
        assert!(p.marked_for_deletion_at.is_none());
    });
}

#[test]
fn deletion_pass_drops_queue_entry_when_post_already_gone() {
    new_test_ext().execute_with(|| {
        reset_deletion_trackers();
        run_to_block(1);
        Popularity::on_post_created(0);
        Popularity::on_post_created(1);
        set_max_post_id(2);

        // Both posts are eligible, but post 0 will fail delete (race: post already removed).
        crate::pallet::PostScores::<Test>::mutate(0, |e| {
            e.as_mut().unwrap().marked_for_deletion_at = Some(1);
        });
        crate::pallet::PostScores::<Test>::mutate(1, |e| {
            e.as_mut().unwrap().marked_for_deletion_at = Some(1);
        });
        crate::pallet::DeletionQueue::<Test>::insert(0u64, 5u64);
        crate::pallet::DeletionQueue::<Test>::insert(1u64, 5u64);

        fail_delete_for(0);

        run_to_block(5);
        Popularity::run_deletion_pass(5);

        // Post 0: PostMutator returned Err → queue entry must be dropped, no event,
        //         no PostScores prune (mutator owns Posts; popularity keeps score
        //         until next scan re-evaluates).
        assert!(crate::pallet::DeletionQueue::<Test>::get(0).is_none(), "queue entry dropped on Err");
        assert!(crate::pallet::PostScores::<Test>::get(0).is_some(), "score not pruned on Err (no double-effect)");
        assert!(!deleted_posts().contains(&0), "no successful delete for post 0");

        // Post 1: succeeded normally.
        assert!(crate::pallet::PostScores::<Test>::get(1).is_none());
        assert!(crate::pallet::DeletionQueue::<Test>::get(1).is_none());
        assert!(deleted_posts().contains(&1));
    });
}

#[test]
fn decay_apply_clamps_at_production_max_decay_steps() {
    use sp_runtime::Permill;
    // The production runtime uses MaxDecaySteps = 1_000_000. With rate 999_950
    // (production's DecayRatePermill), 1M ticks decays a 100k score essentially
    // to 0. Verify the clamp prevents pathological loop runtime even when
    // delta_blocks far exceeds MaxDecaySteps — the function returns the
    // result of MaxDecaySteps iterations, not delta_blocks iterations.
    let production_rate = Permill::from_parts(999_950);
    let production_cap = 1_000_000u32;

    // delta_blocks = u32::MAX, but cap should kick in at 1M iterations.
    let result = crate::decay::apply(100_000, u32::MAX, production_rate, production_cap);
    // After 1M iterations of *0.99995, score should be ~0 (since 0.99995^1M ≈ 1.9e-22).
    assert!(result < 100, "expected near-zero after 1M iterations, got {}", result);

    // Below the cap, behavior is unchanged.
    let unclamped = crate::decay::apply(100_000, 500, production_rate, production_cap);
    // 100_000 * 0.99995^500 ≈ 97_530
    assert!(unclamped >= 97_400 && unclamped <= 97_600, "got {}", unclamped);
}

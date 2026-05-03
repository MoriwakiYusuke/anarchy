//! Unit tests for pallet-popularity score logic.

use crate::{
    mock::*,
    pallet::{DeletionQueue, PostScores},
    PopularityInterface, PopularityReactionType,
};

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

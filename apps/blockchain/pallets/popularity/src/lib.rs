//! # Popularity Pallet
//!
//! 投稿人気度スコア管理。reaction による加点と時間減衰、
//! 閾値割れの mark + 猶予期間後の削除を担当する。
//! 詳細: docs/superpowers/specs/2026-05-03-post-popularity-design.md

#![cfg_attr(not(feature = "std"), no_std)]

pub mod decay;

pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// Reaction kind as observed by the popularity pallet.
/// Independent from `pallet-reaction::ReactionType` to avoid cyclic deps.
#[derive(Clone, Copy, Encode, Decode, TypeInfo, PartialEq, Eq, Debug)]
pub enum PopularityReactionType {
    Like,
    Dislike,
}

/// Trait that callers (post / reaction pallets) use to push popularity events.
pub trait PopularityInterface {
    fn on_post_created(post_id: u64);
    fn on_reaction(post_id: u64, kind: PopularityReactionType);
}

/// No-op implementation — used by mock runtimes that don't wire popularity.
impl PopularityInterface for () {
    fn on_post_created(_post_id: u64) {}
    fn on_reaction(_post_id: u64, _kind: PopularityReactionType) {}
}

/// Implemented by pallet-post (or test mock) so popularity can iterate posts.
pub trait PostCountProvider {
    /// Returns `NextPostId` — the upper bound (exclusive) of the post id space.
    fn next_post_id() -> u64;
}

/// No-op implementation — returns 0. Used by mock runtimes that don't have a real post pallet.
impl PostCountProvider for () {
    fn next_post_id() -> u64 {
        0
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::{traits::Saturating, Permill};

    #[derive(Clone, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq, Eq)]
    #[scale_info(skip_type_params(T))]
    pub struct PostPopularity<BlockNumber> {
        pub stored_score: u64,
        pub last_touched: BlockNumber,
        pub like_count: u32,
        pub dislike_count: u32,
        pub marked_for_deletion_at: Option<BlockNumber>,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Initial score assigned at post creation.
        #[pallet::constant]
        type InitialScore: Get<u64>;

        /// Score delta added when a Like is received.
        #[pallet::constant]
        type LikeWeight: Get<u64>;

        /// Score delta added when a Dislike (Bad) is received.
        #[pallet::constant]
        type DislikeWeight: Get<u64>;

        /// Per-block multiplicative decay rate (out of 1_000_000).
        #[pallet::constant]
        type DecayRatePermill: Get<Permill>;

        /// Effective score below this marks the post for deletion.
        #[pallet::constant]
        type LowPopularityThreshold: Get<u64>;

        /// Margin above threshold required to recover from marked state (anti-flap).
        #[pallet::constant]
        type HysteresisMargin: Get<u64>;

        /// Blocks between mark and actual deletion.
        #[pallet::constant]
        type GracePeriod: Get<BlockNumberFor<Self>>;

        /// Max posts scanned per on_finalize.
        #[pallet::constant]
        type MaxPostsScannedPerBlock: Get<u32>;

        /// Max posts deleted per on_finalize.
        #[pallet::constant]
        type MaxDeletionsPerBlock: Get<u32>;

        /// Decay loop iteration cap (DoS guard for huge `delta_blocks`).
        #[pallet::constant]
        type MaxDecaySteps: Get<u32>;
    }

    #[pallet::storage]
    pub type PostScores<T: Config> = StorageMap<
        _, Blake2_128Concat, u64,
        PostPopularity<BlockNumberFor<T>>, OptionQuery,
    >;

    #[pallet::storage]
    pub type DeletionQueue<T: Config> = StorageMap<
        _, Blake2_128Concat, u64,
        BlockNumberFor<T>, OptionQuery,
    >;

    #[pallet::storage]
    pub type ScanCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PostMarkedForDeletion { post_id: u64, marked_at: BlockNumberFor<T> },
        PostUnmarkedForDeletion { post_id: u64 },
        PostDeleted { post_id: u64 },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Defensive — pallet-popularity does not currently expose call_index entries.
        Unreachable,
    }

    impl<T: Config> Pallet<T> {
        /// Recompute effective score by applying decay since `last_touched`.
        pub(crate) fn effective_score_now(p: &PostPopularity<BlockNumberFor<T>>) -> u64 {
            let now = frame_system::Pallet::<T>::block_number();
            let delta_raw = now.saturating_sub(p.last_touched);
            // BlockNumber → u32 (saturating). For BlockNumber = u32 this is identity.
            let delta = TryInto::<u32>::try_into(delta_raw).unwrap_or(u32::MAX);
            super::decay::apply(p.stored_score, delta, T::DecayRatePermill::get(), T::MaxDecaySteps::get())
        }
    }

    impl<T: Config> super::PopularityInterface for Pallet<T> {
        fn on_post_created(post_id: u64) {
            let now = frame_system::Pallet::<T>::block_number();
            PostScores::<T>::insert(post_id, PostPopularity {
                stored_score: T::InitialScore::get(),
                last_touched: now,
                like_count: 0,
                dislike_count: 0,
                marked_for_deletion_at: None,
            });
        }

        fn on_reaction(post_id: u64, kind: super::PopularityReactionType) {
            use super::PopularityReactionType::*;
            let now = frame_system::Pallet::<T>::block_number();
            PostScores::<T>::mutate(post_id, |entry| {
                let p = entry.get_or_insert_with(|| PostPopularity {
                    stored_score: T::InitialScore::get(),
                    last_touched: now,
                    like_count: 0,
                    dislike_count: 0,
                    marked_for_deletion_at: None,
                });

                // 1. Apply decay up to now and bake it into stored_score.
                p.stored_score = Pallet::<T>::effective_score_now(p);
                p.last_touched = now;

                // 2. Bump counter and add weight.
                let delta = match kind {
                    Like => {
                        p.like_count = p.like_count.saturating_add(1);
                        T::LikeWeight::get()
                    }
                    Dislike => {
                        p.dislike_count = p.dislike_count.saturating_add(1);
                        T::DislikeWeight::get()
                    }
                };
                p.stored_score = p.stored_score.saturating_add(delta);

                // 3. Immediate unmark if recovery threshold met.
                if p.marked_for_deletion_at.is_some() {
                    let recovery = T::LowPopularityThreshold::get()
                        .saturating_add(T::HysteresisMargin::get());
                    if p.stored_score >= recovery {
                        p.marked_for_deletion_at = None;
                        DeletionQueue::<T>::remove(post_id);
                        Pallet::<T>::deposit_event(Event::PostUnmarkedForDeletion { post_id });
                    }
                }
            });
        }
    }
}

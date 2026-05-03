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

/// RPC-shape representation of `PostPopularity` plus derived `net_count`.
/// Returned by the `PopularityApi::get_post_popularity` runtime API.
#[derive(Clone, Encode, Decode, TypeInfo)]
pub struct PostPopularityRpc {
    pub effective_score: u64,
    pub like_count: u32,
    pub dislike_count: u32,
    pub net_count: i64,
    pub marked_for_deletion_at: Option<u32>,
    pub last_touched: u32,
}

sp_api::decl_runtime_apis! {
    pub trait PopularityApi {
        /// Effective (decay-applied) score for a post, computed at the current block.
        fn get_effective_score(post_id: u64) -> Option<u64>;

        /// Net count = like_count - dislike_count, returned as i64.
        fn get_net_count(post_id: u64) -> Option<i64>;

        /// All popularity info bundled for one RPC call.
        fn get_post_popularity(post_id: u64) -> Option<PostPopularityRpc>;
    }
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

/// Mutator implemented by pallet-post; called on policy-driven deletion.
pub trait PostMutator<AccountId> {
    /// Removes the post and all associated metadata (Posts/ContentRefs/MerkleRootToPostId/UserPosts).
    /// Returns the merkle_root so the caller can propagate the deletion to storage.
    fn delete_post(post_id: u64) -> Result<[u8; 32], frame_support::pallet_prelude::DispatchError>;
}

/// No-op for mock runtimes — fails on every call (used when there's no real post pallet).
impl<A> PostMutator<A> for () {
    fn delete_post(_post_id: u64) -> Result<[u8; 32], frame_support::pallet_prelude::DispatchError> {
        Err(frame_support::pallet_prelude::DispatchError::Other("PostMutator not configured"))
    }
}

/// Storage release implemented by pallet-storage; thin shim over `StorageInterface::do_release_fragment`.
pub trait StorageReleaser {
    fn release_fragment(content_hash: [u8; 32]) -> frame_support::pallet_prelude::DispatchResult;
}

/// No-op for mock runtimes.
impl StorageReleaser for () {
    fn release_fragment(_content_hash: [u8; 32]) -> frame_support::pallet_prelude::DispatchResult {
        Ok(())
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

        /// Provider of the current upper bound (`NextPostId`) for the post id space.
        type PostCountProvider: super::PostCountProvider;

        /// Mutator that deletes posts when the policy fires.
        type PostMutator: super::PostMutator<Self::AccountId>;

        /// Storage releaser that drops fragment metadata after deletion.
        type StorageReleaser: super::StorageReleaser;
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

        /// Public wrapper for runtime-API consumers.
        pub fn effective_score_now_public(p: &PostPopularity<BlockNumberFor<T>>) -> u64 {
            Self::effective_score_now(p)
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

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(now: BlockNumberFor<T>) {
            Self::run_scan_pass(now);
            Self::run_deletion_pass(now);
        }
    }

    impl<T: Config> Pallet<T> {
        pub(crate) fn run_scan_pass(now: BlockNumberFor<T>) {
            let max_post_id = T::PostCountProvider::next_post_id();
            if max_post_id == 0 {
                return;
            }
            let scan_limit = T::MaxPostsScannedPerBlock::get();
            let threshold = T::LowPopularityThreshold::get();
            let recovery = threshold.saturating_add(T::HysteresisMargin::get());
            let mut cursor = ScanCursor::<T>::get();
            let mut scanned = 0u32;

            while scanned < scan_limit {
                if cursor >= max_post_id {
                    cursor = 0;
                }
                let id = cursor;
                cursor = cursor.saturating_add(1);
                scanned = scanned.saturating_add(1);

                if let Some(mut p) = PostScores::<T>::get(id) {
                    let eff = Pallet::<T>::effective_score_now(&p);

                    if eff < threshold && p.marked_for_deletion_at.is_none() {
                        p.marked_for_deletion_at = Some(now);
                        let eligible_at = now.saturating_add(T::GracePeriod::get());
                        DeletionQueue::<T>::insert(id, eligible_at);
                        Self::deposit_event(Event::PostMarkedForDeletion { post_id: id, marked_at: now });
                    } else if eff >= recovery && p.marked_for_deletion_at.is_some() {
                        p.marked_for_deletion_at = None;
                        DeletionQueue::<T>::remove(id);
                        Self::deposit_event(Event::PostUnmarkedForDeletion { post_id: id });
                    }

                    p.stored_score = eff;
                    p.last_touched = now;
                    PostScores::<T>::insert(id, p);
                }
            }

            if cursor >= max_post_id {
                cursor = 0;
            }
            ScanCursor::<T>::put(cursor);
        }

        pub(crate) fn run_deletion_pass(now: BlockNumberFor<T>) {
            let limit = T::MaxDeletionsPerBlock::get();

            let candidates: sp_std::vec::Vec<(u64, BlockNumberFor<T>)> = DeletionQueue::<T>::iter()
                .filter(|(_, eligible_at)| now >= *eligible_at)
                .take(limit as usize)
                .collect();

            for (post_id, _) in candidates {
                match T::PostMutator::delete_post(post_id) {
                    Ok(merkle_root) => {
                        // Best-effort — log-only if storage release fails.
                        let _ = T::StorageReleaser::release_fragment(merkle_root);
                        PostScores::<T>::remove(post_id);
                        DeletionQueue::<T>::remove(post_id);
                        Self::deposit_event(Event::PostDeleted { post_id });
                    }
                    Err(_) => {
                        // Post is gone (race). Drop the queue entry.
                        DeletionQueue::<T>::remove(post_id);
                    }
                }
            }
        }
    }
}

//! # Reaction Pallet
//!
//! PoW-based reaction mining for posts.
//! Users can react to posts (Like/Boost/Bad) with PoW proof,
//! post authors receive $moral rewards from the reaction reward pool.
//!
//! ## Features
//! - Like, Boost, Bad reactions with PoW proof
//! - Dynamic difficulty adjustment
//! - Reward distribution from reaction pool
//! - Foreground mining enforcement (via client)

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::traits::fungible::{Inspect, Mutate};
    use frame_system::pallet_prelude::*;
    use parity_scale_codec::DecodeWithMemTracking;
    use primitives_pow::{compute_challenge, verify_proof};
    use sp_runtime::Saturating;

    /// Balance type for $moral token
    pub type BalanceOf<T> =
        <<T as Config>::NativeToken as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

    /// Reaction types
    #[derive(
        Clone, Copy, Encode, Decode, DecodeWithMemTracking, 
        TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq
    )]
    pub enum ReactionType {
        Like,
        Boost,
        Bad,
    }

    impl ReactionType {
        /// Get the weight multiplier for reward calculation
        pub fn weight(&self) -> u128 {
            match self {
                ReactionType::Like => 1,
                ReactionType::Boost => 5,
                ReactionType::Bad => 0,
            }
        }
    }

    /// Reaction record
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
    #[scale_info(skip_type_params(T))]
    pub struct Reaction<T: Config> {
        pub reactor: T::AccountId,
        pub reaction_type: ReactionType,
        pub pow_nonce: u64,
        pub cpu_power: u64,
        pub created_at: BlockNumberFor<T>,
    }

    /// Aggregated reaction statistics per post
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default, PartialEq, Eq)]
    pub struct ReactionStats {
        pub likes: u32,
        pub boosts: u32,
        pub bads: u32,
        pub total_weight: u128,
    }

    /// Difficulty adjustment state
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct DifficultyState<BlockNumber: Default> {
        pub current: u8,
        pub last_adjusted: BlockNumber,
        pub recent_count: u32,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Native token ($moral) for reward payouts
        type NativeToken: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

        /// Base PoW difficulty (number of leading zero bits required)
        #[pallet::constant]
        type BaseDifficulty: Get<u8>;

        /// Minimum PoW difficulty
        #[pallet::constant]
        type MinDifficulty: Get<u8>;

        /// Maximum PoW difficulty
        #[pallet::constant]
        type MaxDifficulty: Get<u8>;

        /// Challenge validity period (in blocks)
        #[pallet::constant]
        type ChallengeValidity: Get<BlockNumberFor<Self>>;

        /// Target reactions per block for difficulty adjustment
        #[pallet::constant]
        type TargetReactionRate: Get<u32>;

        /// Adjustment window (in blocks) for difficulty recalculation
        #[pallet::constant]
        type AdjustmentWindow: Get<BlockNumberFor<Self>>;

        /// Adjustment divisor for smooth difficulty changes
        #[pallet::constant]
        type AdjustmentDivisor: Get<u32>;
    }

    /// Reaction records: (post_id, reactor) -> Reaction
    #[pallet::storage]
    #[pallet::getter(fn reactions)]
    pub type Reactions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        Reaction<T>,
        OptionQuery,
    >;

    /// Reaction statistics per post
    #[pallet::storage]
    #[pallet::getter(fn reaction_stats)]
    pub type ReactionStatsStorage<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        ReactionStats,
        ValueQuery,
    >;

    /// User's total reaction count
    #[pallet::storage]
    #[pallet::getter(fn user_reaction_count)]
    pub type UserReactionCount<T: Config> = StorageMap<
        _,
        Blake2_128Concat, T::AccountId,
        u32,
        ValueQuery,
    >;

    /// Reaction reward pool balance (in planck)
    #[pallet::storage]
    #[pallet::getter(fn reaction_reward_pool)]
    pub type ReactionRewardPool<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// Current PoW difficulty
    #[pallet::storage]
    #[pallet::getter(fn current_difficulty)]
    pub type CurrentDifficulty<T: Config> = StorageValue<_, u8, ValueQuery>;

    /// Reaction count per block (for difficulty adjustment)
    #[pallet::storage]
    #[pallet::getter(fn reaction_history)]
    pub type ReactionHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat, BlockNumberFor<T>,
        u32,
        ValueQuery,
    >;

    /// Total reactions count
    #[pallet::storage]
    #[pallet::getter(fn total_reactions)]
    pub type TotalReactions<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// Initial reaction reward pool balance
        pub initial_reward_pool: u128,
        /// Initial difficulty
        pub initial_difficulty: u8,
        #[serde(skip)]
        pub _marker: core::marker::PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            ReactionRewardPool::<T>::put(self.initial_reward_pool);
            CurrentDifficulty::<T>::put(self.initial_difficulty);
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Reaction created successfully
        ReactionCreated {
            post_id: u64,
            reactor: T::AccountId,
            reaction_type: ReactionType,
            reward_paid: u128,
        },
        /// Reward pool deposit received
        RewardPoolDeposit {
            amount: u128,
        },
        /// Difficulty adjusted
        DifficultyAdjusted {
            old_difficulty: u8,
            new_difficulty: u8,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// User has already reacted to this post
        AlreadyReacted,
        /// The PoW challenge has expired
        ChallengeExpired,
        /// The PoW proof is invalid
        InvalidProof,
        /// The specified block does not exist
        BlockNotFound,
        /// Post not found
        PostNotFound,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Submit a reaction to a post with PoW proof.
        ///
        /// # Arguments
        /// * `post_id` - The post to react to
        /// * `reaction_type` - Type of reaction (Like/Boost/Bad)
        /// * `block_number` - Block number used for challenge generation
        /// * `nonce` - PoW nonce that satisfies difficulty
        /// * `cpu_power` - Reported hashrate (for reward calculation)
        /// * `stealth_recipient` - Optional stealth address for reward
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(5, 5))]
        pub fn react(
            origin: OriginFor<T>,
            post_id: u64,
            reaction_type: ReactionType,
            block_number: BlockNumberFor<T>,
            nonce: u64,
            cpu_power: u64,
            _stealth_recipient: Option<T::AccountId>,
        ) -> DispatchResult {
            let reactor = ensure_signed(origin)?;

            // Check for duplicate reaction
            ensure!(
                !Reactions::<T>::contains_key(post_id, &reactor),
                Error::<T>::AlreadyReacted
            );

            // Validate challenge expiry
            let current_block = frame_system::Pallet::<T>::block_number();
            let validity = T::ChallengeValidity::get();
            let max_valid_block = block_number.saturating_add(validity);
            ensure!(current_block <= max_valid_block, Error::<T>::ChallengeExpired);

            // Get block hash for challenge
            let block_hash = frame_system::Pallet::<T>::block_hash(block_number);
            ensure!(block_hash != Default::default(), Error::<T>::BlockNotFound);

            // Verify PoW proof
            let challenge = compute_challenge(block_hash.as_ref(), &reactor.encode());
            let difficulty = CurrentDifficulty::<T>::get().max(T::MinDifficulty::get());
            ensure!(
                verify_proof(&challenge, nonce, difficulty),
                Error::<T>::InvalidProof
            );

            // Store reaction
            let reaction = Reaction {
                reactor: reactor.clone(),
                reaction_type,
                pow_nonce: nonce,
                cpu_power,
                created_at: current_block,
            };
            Reactions::<T>::insert(post_id, &reactor, reaction);

            // Update statistics
            ReactionStatsStorage::<T>::mutate(post_id, |stats| {
                match reaction_type {
                    ReactionType::Like => stats.likes = stats.likes.saturating_add(1),
                    ReactionType::Boost => stats.boosts = stats.boosts.saturating_add(1),
                    ReactionType::Bad => stats.bads = stats.bads.saturating_add(1),
                }
                stats.total_weight = stats.total_weight.saturating_add(reaction_type.weight());
            });

            // Update user reaction count
            UserReactionCount::<T>::mutate(&reactor, |count| {
                *count = count.saturating_add(1);
            });

            // Update reaction history for this block
            ReactionHistory::<T>::mutate(current_block, |count| {
                *count = count.saturating_add(1);
            });

            // Increment total reactions
            TotalReactions::<T>::mutate(|total| {
                *total = total.saturating_add(1);
            });

            // Calculate and pay reward
            // Reward = Weight × CPUPower × γ
            // γ = ReactionRewardPool / TotalSupply (simplified: use pool balance directly)
            let weight = reaction_type.weight();
            let pool_balance = ReactionRewardPool::<T>::get();
            
            // Simplified reward: base_reward = weight * cpu_power / 1_000_000
            // Capped by available pool
            let base_reward = weight
                .saturating_mul(cpu_power as u128)
                .saturating_div(1_000_000);
            let reward = base_reward.min(pool_balance);

            // Pay reward if pool has funds and reward > 0
            // Note: Post author lookup would require pallet-post integration
            // For now, we emit the event with reward amount
            if reward > 0 {
                ReactionRewardPool::<T>::mutate(|pool| {
                    *pool = pool.saturating_sub(reward);
                });
            }

            Self::deposit_event(Event::ReactionCreated {
                post_id,
                reactor,
                reaction_type,
                reward_paid: reward,
            });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Adjust difficulty based on recent reaction rate.
        /// Called from on_finalize hook.
        pub fn adjust_difficulty() {
            let current_block = frame_system::Pallet::<T>::block_number();
            let window = T::AdjustmentWindow::get();
            let target_rate = T::TargetReactionRate::get();
            let divisor = T::AdjustmentDivisor::get();

            // Count reactions in the adjustment window
            let mut recent_count = 0u32;
            let mut block = current_block;
            let window_start = current_block.saturating_sub(window);
            
            while block > window_start {
                recent_count = recent_count.saturating_add(ReactionHistory::<T>::get(block));
                block = block.saturating_sub(1u32.into());
            }

            let current = CurrentDifficulty::<T>::get();
            let target_total = target_rate.saturating_mul(
                TryInto::<u32>::try_into(window).unwrap_or(1)
            );

            // Calculate adjustment
            let diff = if recent_count > target_total {
                (recent_count.saturating_sub(target_total)) / divisor.max(1)
            } else {
                0
            };

            let decrease = if recent_count < target_total {
                (target_total.saturating_sub(recent_count)) / divisor.max(1)
            } else {
                0
            };

            let new_difficulty = current
                .saturating_add(diff as u8)
                .saturating_sub(decrease as u8)
                .clamp(T::MinDifficulty::get(), T::MaxDifficulty::get());

            if new_difficulty != current {
                CurrentDifficulty::<T>::put(new_difficulty);
                Self::deposit_event(Event::DifficultyAdjusted {
                    old_difficulty: current,
                    new_difficulty,
                });
            }
        }
    }

    /// Interface for other pallets (e.g., pallet-post) to interact with reaction pallet
    pub trait ReactionInterface {
        /// Deposit tokens into the reaction reward pool
        fn do_deposit_to_reaction_pool(amount: u128);

        /// Get reaction counts for a post
        fn get_reaction_counts(post_id: u64) -> Option<(u32, u32, u32)>;

        /// Get bad reaction count for a post
        fn get_bad_count(post_id: u64) -> u32;
    }

    impl<T: Config> ReactionInterface for Pallet<T> {
        fn do_deposit_to_reaction_pool(amount: u128) {
            ReactionRewardPool::<T>::mutate(|pool| {
                *pool = pool.saturating_add(amount);
            });
            Self::deposit_event(Event::RewardPoolDeposit { amount });
        }

        fn get_reaction_counts(post_id: u64) -> Option<(u32, u32, u32)> {
            let stats = ReactionStatsStorage::<T>::get(post_id);
            Some((stats.likes, stats.boosts, stats.bads))
        }

        fn get_bad_count(post_id: u64) -> u32 {
            ReactionStatsStorage::<T>::get(post_id).bads
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_n: BlockNumberFor<T>) {
            // Adjust difficulty every AdjustmentWindow blocks
            let window = T::AdjustmentWindow::get();
            if _n % window == 0u32.into() {
                Self::adjust_difficulty();
            }
        }
    }
}

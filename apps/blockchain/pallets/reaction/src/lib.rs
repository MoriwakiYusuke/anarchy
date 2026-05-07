//! # Reaction Pallet
//!
//! PoW-based reaction mining for posts.
//! Users can react to posts (Like/Bad) with PoW proof,
//! post authors receive $moral rewards from the reaction reward pool.
//!
//! ## Features
//! - Like, Bad reactions with PoW proof
//! - Dynamic difficulty adjustment
//! - Reward distribution from reaction pool
//! - Foreground mining enforcement (via client)

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

/// Trait for getting post author from post pallet
pub trait PostAuthorProvider<AccountId> {
    /// Get the author of a post by post_id
    fn get_post_author(post_id: u64) -> Option<AccountId>;
}

#[frame_support::pallet]
pub mod pallet {
    use super::PostAuthorProvider;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::fungible::{Inspect, Mutate};
    use frame_support::traits::ReservableCurrency;
    use frame_system::pallet_prelude::*;
    use pallet_popularity::{PopularityInterface, PopularityReactionType};
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
        Bad,
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
        pub bads: u32,
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
        /// Native token ($moral) for reward payouts.
        ///
        /// `Inspect + Mutate` は mint_into / burn_from で reaction 報酬の支払いに、
        /// `ReservableCurrency` (Balance = u128) は reactor_lock の reserve / unreserve に使う。
        /// runtime ではどちらも pallet_balances::Balances を渡す。
        type NativeToken: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

        /// Reservable currency (TSTS P5: reactor lock 用).
        /// pallet_balances は ReservableCurrency と fungible::Mutate の両方を実装している。
        type ReservableCurrency: ReservableCurrency<
            Self::AccountId,
            Balance = BalanceOf<Self>,
        >;

        /// Provider for getting post authors
        type PostAuthorProvider: PostAuthorProvider<Self::AccountId>;

        /// Fixed reward per reaction (旧モデル, TSTS では fallback として残す)。
        ///
        /// `GammaMaxPpm = 0` (γ 動的計算を無効化) のときに使用される。
        /// runtime では γ 計算を有効にするため `GammaMaxPpm > 0` を設定し、この値は使われない。
        #[pallet::constant]
        type ReactionReward: Get<BalanceOf<Self>>;

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

        /// Popularity sink — receives push notifications when a reaction is recorded.
        type Popularity: pallet_popularity::PopularityInterface;

        /// Total issuance provider (TSTS P5: γ 動的計算用)。
        ///
        /// `pallet_balances::total_issuance()` を返す。runtime で `BalancesTotalIssuance` adapter を渡す。
        /// 0 を返すと γ 計算は ReactionReward fallback に縮退する。
        type TotalIssuanceProvider: TotalIssuanceProvider;

        /// γ 最大値 (Permill ppm). TSTS P5.
        ///
        /// γ = ReactionPool / TotalIssuance を上限 `GammaMaxPpm / 1_000_000` で clamp する。
        /// 0 を指定すると γ 動的計算を無効化し、固定 ReactionReward に縮退する (旧挙動互換)。
        /// spec §3.2.6 で 10_000 (= 1%) 推奨。
        #[pallet::constant]
        type GammaMaxPpm: Get<u32>;

        /// Reactor decay K (TSTS P5).
        ///
        /// 同一 reactor の n 番目の反応の decay 係数 = `1 / sqrt(1 + n / K)`.
        /// K が大きいほど減衰が緩やか。0 を指定すると decay 無効。spec §3.2.6 で 100 推奨。
        #[pallet::constant]
        type ReactorDecayK: Get<u32>;

        /// Daily payout cap (Permill ppm). TSTS P5.
        ///
        /// 1 ブロックあたりの reaction pool 流出を `pool × DailyPayoutCapPpm / 1_000_000`
        /// 以下に抑える (Sybil 大量攻撃対策)。
        /// spec では 1 day = 2880 blocks に対する 5% を 1 block あたり 5%/2880 で按分。
        /// 0 を指定すると cap 無効。
        #[pallet::constant]
        type PerBlockPayoutCapPpm: Get<u32>;

        /// Reactor lock minimum (TSTS P5).
        ///
        /// 報酬を受け取るには `lock_for_rewards` で `ReactorLockMin` 以上を一定期間ロックする必要がある。
        /// 0 を指定すると lock 不要 (Sybil 防御無効、旧挙動互換)。spec で 0.1 MORAL 推奨。
        #[pallet::constant]
        type ReactorLockMin: Get<BalanceOf<Self>>;

        /// Reactor lock duration (TSTS P5).
        ///
        /// `lock_for_rewards` で lock してから unlock 可能になるまでのブロック数。
        /// spec で 24h ≒ 2880 blocks 推奨。
        #[pallet::constant]
        type ReactorLockDuration: Get<BlockNumberFor<Self>>;
    }

    /// 総発行量を取得する trait (γ 計算用)。
    ///
    /// pallet_balances::Pallet::<Runtime>::total_issuance() の値を u128 に変換して返す。
    /// pallet_reaction が pallet_balances に直接依存しないようにするための抽象。
    pub trait TotalIssuanceProvider {
        fn total_issuance() -> u128;
    }

    impl TotalIssuanceProvider for () {
        fn total_issuance() -> u128 {
            0
        }
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

    /// 各 reactor の累積反応回数 (TSTS P5: decay 計算用)。
    ///
    /// `react` extrinsic 成功時に increment し、payout decay に使う。永続加算で
    /// reactor のキャリアが進むほど 1 反応あたり報酬が `1/sqrt(1+n/K)` で薄まる。
    /// session-key 単位の Anarchy 設計と整合: 鍵を変えれば cumulative count はリセットされるが、
    /// その都度 lock も再構築する必要があるためコストは reactor_lock で担う。
    #[pallet::storage]
    #[pallet::getter(fn reactor_cumulative_count)]
    pub type ReactorCumulativeCount<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

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

    /// Reactor lock 情報 (TSTS P5).
    ///
    /// 各 reactor が報酬を受け取るために lock した残高と、unlock 可能になる block。
    /// `react` で reward を払うときの必須条件として参照する。
    #[pallet::storage]
    #[pallet::getter(fn reactor_locks)]
    pub type ReactorLocks<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        ReactorLock<BlockNumberFor<T>, BalanceOf<T>>,
        OptionQuery,
    >;

    /// 1 ブロックあたりの累積 payout (TSTS P5: per-block cap 用)。
    ///
    /// `react` 内で payout を加算し、`PerBlockPayoutCapPpm × pool` を超える分は無視する。
    /// `on_finalize` で reset。
    #[pallet::storage]
    #[pallet::getter(fn per_block_payout_accumulator)]
    pub type PerBlockPayoutAccumulator<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// Reactor lock 構造体。
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
    pub struct ReactorLock<BlockNumber, Balance> {
        pub amount: Balance,
        pub unlock_at: BlockNumber,
    }

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
        /// Reactor lock を bond した (TSTS P5)
        ReactorLockBonded {
            who: T::AccountId,
            amount: BalanceOf<T>,
            unlock_at: BlockNumberFor<T>,
        },
        /// Reactor lock を release した (TSTS P5)
        ReactorLockReleased {
            who: T::AccountId,
            amount: BalanceOf<T>,
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
        /// Reactor lock 不足 (TSTS P5: 報酬を得るには ReactorLockMin 以上ロック必要)
        InsufficientReactorLock,
        /// すでに reactor lock を保有しており unlock 待ち中
        ReactorLockAlreadyHeld,
        /// unlock_at に達していないため unlock できない
        ReactorLockStillActive,
        /// reactor lock を持っていない
        ReactorLockNotFound,
        /// 残高不足で lock できない
        InsufficientBalance,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Submit a reaction to a post with PoW proof.
        ///
        /// # Arguments
        /// * `post_id` - The post to react to
        /// * `reaction_type` - Type of reaction (Like/Bad)
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
                    ReactionType::Bad => stats.bads = stats.bads.saturating_add(1),
                }
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

            // TSTS P5: 動的 γ × decay × per-block cap で payout 計算。
            // Bad reactions は報酬 0 (旧仕様維持)。Like のみが対象。
            // Reactor lock の有無もここでチェックし、lock 未保有なら signal-only (reward 0)。
            let mut reward_paid: BalanceOf<T> = 0u32.into();

            if reaction_type != ReactionType::Bad {
                if let Some(author) = T::PostAuthorProvider::get_post_author(post_id) {
                    let reactor_count = ReactorCumulativeCount::<T>::get(&reactor);
                    let pool_balance = ReactionRewardPool::<T>::get();
                    let total_issuance = T::TotalIssuanceProvider::total_issuance();

                    let computed_reward_u128 = Self::compute_reward(
                        pool_balance,
                        total_issuance,
                        reactor_count,
                    );

                    // Reactor lock チェック: lock 未保有 or 期限切れ → reward 0
                    let lock_required = T::ReactorLockMin::get();
                    let lock_ok = if lock_required == BalanceOf::<T>::default() {
                        // ReactorLockMin = 0 なら lock 不要 (旧挙動互換)
                        true
                    } else {
                        match ReactorLocks::<T>::get(&reactor) {
                            Some(lock) => lock.amount >= lock_required,
                            None => false,
                        }
                    };

                    if computed_reward_u128 > 0 && lock_ok {
                        // Per-block cap で truncate
                        let cap_ppm = T::PerBlockPayoutCapPpm::get() as u128;
                        let already_paid = PerBlockPayoutAccumulator::<T>::get();
                        let max_per_block = if cap_ppm == 0 {
                            u128::MAX
                        } else {
                            pool_balance.saturating_mul(cap_ppm) / 1_000_000
                        };
                        let remaining_budget = max_per_block.saturating_sub(already_paid);
                        let granted_u128 = computed_reward_u128.min(remaining_budget).min(pool_balance);

                        if granted_u128 > 0 {
                            let granted: BalanceOf<T> = granted_u128.try_into().unwrap_or_default();
                            if T::NativeToken::mint_into(&author, granted).is_ok() {
                                reward_paid = granted;
                                ReactionRewardPool::<T>::put(pool_balance.saturating_sub(granted_u128));
                                PerBlockPayoutAccumulator::<T>::mutate(|a| *a = a.saturating_add(granted_u128));
                            }
                        }
                    }

                    // 累積反応カウントは reward の有無に関わらず increment (signal-only でも進む)
                    ReactorCumulativeCount::<T>::mutate(&reactor, |c| *c = c.saturating_add(1));
                }
            }

            // Push the reaction into popularity (Like/Bad → Like/Dislike).
            let pop_kind = match reaction_type {
                ReactionType::Like => PopularityReactionType::Like,
                ReactionType::Bad => PopularityReactionType::Dislike,
            };
            T::Popularity::on_reaction(post_id, pop_kind);

            Self::deposit_event(Event::ReactionCreated {
                post_id,
                reactor,
                reaction_type,
                reward_paid: reward_paid.try_into().unwrap_or(0u128),
            });

            Ok(())
        }

        /// TSTS P5: 反応報酬を受け取るための bond をロックする。
        ///
        /// `ReservableCurrency::reserve` で実 balance を予約。`unlock_reactor` で
        /// `ReactorLockDuration` ブロック経過後に解除可能。`unlock_at` 未到達で再 lock しようとすると
        /// `ReactorLockAlreadyHeld` で失敗 (重ねがけ防止: シンプルさ優先)。
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2, 2))]
        pub fn lock_for_rewards(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 既存 lock があれば拒否 (TSTS v1: 単純化のため重ねがけ不可)
            ensure!(
                !ReactorLocks::<T>::contains_key(&who),
                Error::<T>::ReactorLockAlreadyHeld
            );

            ensure!(amount >= T::ReactorLockMin::get(), Error::<T>::InsufficientReactorLock);

            T::ReservableCurrency::reserve(&who, amount)
                .map_err(|_| Error::<T>::InsufficientBalance)?;

            let unlock_at = frame_system::Pallet::<T>::block_number()
                .saturating_add(T::ReactorLockDuration::get());

            ReactorLocks::<T>::insert(&who, ReactorLock { amount, unlock_at });
            Self::deposit_event(Event::ReactorLockBonded { who, amount, unlock_at });
            Ok(())
        }

        /// TSTS P5: lock 解除 (unlock_at 以後にのみ成功).
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2, 2))]
        pub fn unlock_reactor(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let lock = ReactorLocks::<T>::get(&who).ok_or(Error::<T>::ReactorLockNotFound)?;
            let now = frame_system::Pallet::<T>::block_number();
            ensure!(now >= lock.unlock_at, Error::<T>::ReactorLockStillActive);

            // unreserve は失敗しても 0 残差で返るのでチェック不要 (saturating)
            let _ = T::ReservableCurrency::unreserve(&who, lock.amount);
            ReactorLocks::<T>::remove(&who);
            Self::deposit_event(Event::ReactorLockReleased { who, amount: lock.amount });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// TSTS P5: γ × decay 報酬式の本体 (純粋関数なので unit test 容易).
        ///
        /// `reward = γ × ReactionReward × decay(reactor_count)` を u128 で計算。
        /// γ_max が 0 のときは旧 fixed reward に縮退する。
        ///
        /// - γ_ppm = min(GammaMaxPpm, pool × 1e6 / total_issuance)
        /// - decay_ppm = sqrt_ppm(K × 1e6 / (K + reactor_count))  (sqrt(K/(K+n)) を ppm で近似)
        /// - reward_u128 = ReactionReward × γ_ppm × decay_ppm / 1e12
        pub fn compute_reward(
            pool_balance: u128,
            total_issuance: u128,
            reactor_count: u64,
        ) -> u128 {
            let gamma_max_ppm = T::GammaMaxPpm::get() as u128;
            let base_reward_u128: u128 = T::ReactionReward::get().try_into().unwrap_or(0);

            // γ_max = 0 → fixed reward (旧挙動互換)。pool が枯渇していたら 0。
            if gamma_max_ppm == 0 {
                return base_reward_u128.min(pool_balance);
            }

            // total_issuance == 0 ならフォールバック: γ = γ_max とみなす (genesis 直後の安全装置)
            let gamma_ppm = if total_issuance == 0 {
                gamma_max_ppm
            } else {
                let raw = pool_balance.saturating_mul(1_000_000) / total_issuance.max(1);
                raw.min(gamma_max_ppm)
            };

            // decay_ppm = sqrt(K / (K + n)) × 1e6
            let k = T::ReactorDecayK::get() as u128;
            let decay_ppm = if k == 0 {
                1_000_000
            } else {
                let n = reactor_count as u128;
                // sqrt_ppm(x_ppm) で x_ppm/1e6 の sqrt を ppm 単位で返す
                let inner_ppm = k.saturating_mul(1_000_000) / (k + n).max(1);
                Self::sqrt_ppm(inner_ppm)
            };

            // base × γ × decay / 1e6 / 1e6
            let r1 = base_reward_u128.saturating_mul(gamma_ppm) / 1_000_000;
            r1.saturating_mul(decay_ppm) / 1_000_000
        }

        /// Newton iteration による整数 sqrt (TSTS P5 sqrt_ppm 用).
        ///
        /// 入力: x_ppm = x × 1e6 として、戻り値は sqrt(x) × 1e6 = sqrt(x_ppm × 1e6)。
        /// 例: x_ppm=1_000_000 (= x=1.0) → 戻り値 1_000_000。
        ///     x_ppm=  250_000 (= x=0.25) → 戻り値 500_000 (= sqrt(0.25)).
        pub fn sqrt_ppm(x_ppm: u128) -> u128 {
            // 1e12 倍した値の整数 sqrt が sqrt(x) × 1e6 になる
            let scaled = x_ppm.saturating_mul(1_000_000);
            if scaled == 0 {
                return 0;
            }
            let mut z = (scaled + 1) / 2;
            let mut y = scaled;
            // Newton method: y_new = (y + scaled / y) / 2、収束まで反復 (高々 ~64 回)
            while z < y {
                y = z;
                z = (scaled / z + z) / 2;
            }
            y
        }

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
        fn get_reaction_counts(post_id: u64) -> Option<(u32, u32)>;

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

        fn get_reaction_counts(post_id: u64) -> Option<(u32, u32)> {
            let stats = ReactionStatsStorage::<T>::get(post_id);
            Some((stats.likes, stats.bads))
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

            // TSTS P5: per-block payout accumulator をリセット (次ブロックの cap を再評価)
            PerBlockPayoutAccumulator::<T>::kill();

            // Prune old reaction history entries to prevent unbounded state growth
            // Keep only the last AdjustmentWindow * 2 blocks of history
            let prune_window = window.saturating_mul(2u32.into());
            if _n > prune_window {
                let prune_block = _n.saturating_sub(prune_window);
                ReactionHistory::<T>::remove(prune_block);
            }
        }
    }
}

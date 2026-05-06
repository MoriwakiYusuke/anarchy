//! # Block Reward Pallet
//!
//! PoW miner にブロック報酬を mint する。Bitcoin 風の halving (4 年毎)。
//! Author 取得は `T::AuthorOrigin` (FindAuthor) 経由。
//! Reaction-mining (foreground PoW) とは別ドメインであることに注意。

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::traits::{Currency, FindAuthor};
    use frame_system::pallet_prelude::*;
    use parity_scale_codec::DecodeWithMemTracking;

    pub type BalanceOf<T> = <<T as Config>::Currency as Currency<
        <T as frame_system::Config>::AccountId,
    >>::Balance;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 残高インスペクト・mint に使う Currency 実装。
        type Currency: Currency<Self::AccountId>;
        /// 初期報酬 (era 0)。spec §1 で 5 MORAL = 5 * 10^12 推奨。
        #[pallet::constant]
        type InitialReward: Get<BalanceOf<Self>>;
        /// 何ブロック毎に halving するか。spec §6.1 で 4_204_800 推奨。
        #[pallet::constant]
        type HalvingPeriod: Get<BlockNumberFor<Self>>;
        /// 何回 halving したら mint を停止するか。spec §6.1 で 64 推奨。
        #[pallet::constant]
        type MaxHalvings: Get<u32>;
        /// PoW author 抽出。Phase A では mock。Phase B で `node/src/pow/author.rs` 由来の
        /// `PowAuthor` を runtime 側で `FindAuthor` impl して渡す。
        type AuthorOrigin: FindAuthor<Self::AccountId>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BlockRewardMinted { author: T::AccountId, amount: BalanceOf<T> },
        BlockRewardSkipped { reason: SkipReason },
    }

    #[derive(Clone, Encode, Decode, DecodeWithMemTracking, TypeInfo, RuntimeDebug, PartialEq, Eq, MaxEncodedLen)]
    pub enum SkipReason {
        NoAuthor,
        ZeroReward,
    }

    impl<T: Config> Pallet<T> {
        /// 指定ブロック番号における halving 適用後の報酬。
        pub fn current_reward(n: BlockNumberFor<T>) -> BalanceOf<T> {
            use sp_runtime::traits::SaturatedConversion;

            let halving_period: u128 = T::HalvingPeriod::get().saturated_into();
            if halving_period == 0 {
                return T::InitialReward::get();
            }
            let block_n: u128 = n.saturated_into();
            let halvings = (block_n / halving_period) as u32;
            if halvings >= T::MaxHalvings::get() {
                return BalanceOf::<T>::default();
            }

            let initial: u128 = T::InitialReward::get().saturated_into();
            let reduced: u128 = initial >> halvings;
            reduced.saturated_into()
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(n: BlockNumberFor<T>) {
            let digest = <frame_system::Pallet<T>>::digest();
            let pre_runtime_iter = digest
                .logs
                .iter()
                .filter_map(|log| log.as_pre_runtime());
            let author = T::AuthorOrigin::find_author(pre_runtime_iter);

            let Some(author) = author else {
                log::warn!(target: "block_reward", "no author in pre-runtime digest at block {:?}, skipping reward", n);
                Self::deposit_event(Event::BlockRewardSkipped { reason: SkipReason::NoAuthor });
                return;
            };

            let reward = Self::current_reward(n);
            if reward == BalanceOf::<T>::default() {
                log::debug!(target: "block_reward", "reward is zero at block {:?} (post-halving), skipping", n);
                Self::deposit_event(Event::BlockRewardSkipped { reason: SkipReason::ZeroReward });
                return;
            }

            let _ = T::Currency::deposit_creating(&author, reward);
            Self::deposit_event(Event::BlockRewardMinted { author, amount: reward });
        }
    }
}

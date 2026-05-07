//! # Block Reward Pallet
//!
//! PoW miner にブロック報酬を mint する。Bitcoin 風 halving + Monero 風 tail emission。
//! 3-way fan-out で miner / Storage プール / Reaction プールに分配する (TSTS 経済モデル v1)。
//!
//! ## 報酬計算
//!
//! ```text
//! reward(h) = max(InitialReward >> halvings(h), TailEmission)
//! miner_share    = reward × MinerSharePermill
//! storage_share  = reward × StorageSharePermill
//! reaction_share = reward × ReactionSharePermill
//! ```
//!
//! 三者の Permill 合計が 1_000_000 を超えると runtime config の整合性違反として
//! `BlockRewardSplit` イベントの emit を抑止する。`MaxHalvings` 到達後は halved 部分が 0 だが
//! `TailEmission > 0` なら永続的に miner と pool への流入が続く (永続セキュリティ予算)。

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
    use sp_runtime::Permill;

    pub type BalanceOf<T> = <<T as Config>::Currency as Currency<
        <T as frame_system::Config>::AccountId,
    >>::Balance;

    /// 別 pallet (storage / reaction) のリワードプールに流入させるための trait。
    ///
    /// runtime 層で `pallet_storage` / `pallet_reaction` の `*Interface` を adapt する想定。
    /// テストでは unit `()` 実装が no-op で動くようにしてある。
    pub trait PoolDeposit {
        fn do_deposit(amount: u128);
    }

    impl PoolDeposit for () {
        fn do_deposit(_amount: u128) {}
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 残高インスペクト・mint に使う Currency 実装。
        type Currency: Currency<Self::AccountId>;
        /// 初期報酬 (era 0)。spec §1 で 5 MORAL = 5 * 10^12 推奨。
        #[pallet::constant]
        type InitialReward: Get<BalanceOf<Self>>;
        /// 永続的な報酬下限 (Monero 風 tail emission)。
        /// halving が進んでもこの値より下にならないため、
        /// 51% 攻撃コスト > 0 を永続保証する。spec §3.2.1 で 0.5 MORAL = 5 * 10^11 推奨。
        #[pallet::constant]
        type TailEmission: Get<BalanceOf<Self>>;
        /// 何ブロック毎に halving するか。spec §6.1 で 4_204_800 推奨。
        #[pallet::constant]
        type HalvingPeriod: Get<BlockNumberFor<Self>>;
        /// 何回 halving したら halved 部分が 0 になるか (TailEmission がある場合は実質無効)。
        #[pallet::constant]
        type MaxHalvings: Get<u32>;

        /// PoW author 抽出。runtime 側で `PowAuthorAdapter` を `FindAuthor` impl して渡す。
        type AuthorOrigin: FindAuthor<Self::AccountId>;

        /// Miner に直接 mint する割合 (Permill)。
        #[pallet::constant]
        type MinerSharePermill: Get<Permill>;

        /// Storage 報酬プールに流入させる割合 (Permill)。
        #[pallet::constant]
        type StorageSharePermill: Get<Permill>;

        /// Reaction 報酬プールに流入させる割合 (Permill)。
        #[pallet::constant]
        type ReactionSharePermill: Get<Permill>;

        /// Storage プール sink。`pallet_storage::Pallet` の `do_deposit_to_reward_pool` を adapt する。
        type StoragePoolSink: PoolDeposit;

        /// Reaction プール sink。`pallet_reaction::Pallet` の `do_deposit_to_reaction_pool` を adapt する。
        type ReactionPoolSink: PoolDeposit;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// レガシー互換: 旧テストや UI が author/amount だけ拾うパスも残す。
        BlockRewardMinted { author: T::AccountId, amount: BalanceOf<T> },
        /// 3-way fan-out の各取り分を完全に開示する (Grafana / シミュレータ検証用)。
        BlockRewardSplit {
            author: T::AccountId,
            miner: BalanceOf<T>,
            storage: BalanceOf<T>,
            reaction: BalanceOf<T>,
        },
        /// 報酬 0 もしくは author 不明でスキップした。
        BlockRewardSkipped { reason: SkipReason },
    }

    #[derive(Clone, Encode, Decode, DecodeWithMemTracking, TypeInfo, RuntimeDebug, PartialEq, Eq, MaxEncodedLen)]
    pub enum SkipReason {
        NoAuthor,
        ZeroReward,
        InvalidSharesConfig,
    }

    impl<T: Config> Pallet<T> {
        /// 指定ブロック番号における halving + tail emission 適用後の総報酬。
        ///
        /// `MaxHalvings` 到達後は halved 部分が 0 だが、`TailEmission` が大きければそちらに飽和する。
        pub fn current_reward(n: BlockNumberFor<T>) -> BalanceOf<T> {
            use sp_runtime::traits::SaturatedConversion;

            let halving_period: u128 = T::HalvingPeriod::get().saturated_into();
            let tail: u128 = T::TailEmission::get().saturated_into();

            if halving_period == 0 {
                let initial: u128 = T::InitialReward::get().saturated_into();
                return initial.max(tail).saturated_into();
            }

            let block_n: u128 = n.saturated_into();
            let halvings = (block_n / halving_period) as u32;
            let initial: u128 = T::InitialReward::get().saturated_into();

            let halved: u128 = if halvings >= T::MaxHalvings::get() {
                0
            } else {
                initial >> halvings
            };

            halved.max(tail).saturated_into()
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(n: BlockNumberFor<T>) {
            use sp_runtime::traits::SaturatedConversion;

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

            let total = Self::current_reward(n);
            if total == BalanceOf::<T>::default() {
                log::debug!(target: "block_reward", "reward is zero at block {:?}, skipping", n);
                Self::deposit_event(Event::BlockRewardSkipped { reason: SkipReason::ZeroReward });
                return;
            }

            // Permill 合計 sanity check。`MinerShare + StorageShare + ReactionShare > 100%`
            // が来たら chain spec 設定ミス → 全部 miner に渡すフォールバック
            // (security 予算は保ち、誤って多重 mint しないことを優先)。
            let miner_pp = T::MinerSharePermill::get().deconstruct() as u64;
            let storage_pp = T::StorageSharePermill::get().deconstruct() as u64;
            let reaction_pp = T::ReactionSharePermill::get().deconstruct() as u64;
            if miner_pp.saturating_add(storage_pp).saturating_add(reaction_pp) > 1_000_000 {
                log::error!(target: "block_reward", "share permill sum > 1_000_000, fallback to miner-only");
                Self::deposit_event(Event::BlockRewardSkipped { reason: SkipReason::InvalidSharesConfig });
                let _ = T::Currency::deposit_creating(&author, total);
                Self::deposit_event(Event::BlockRewardMinted { author, amount: total });
                return;
            }

            let miner_share = T::MinerSharePermill::get().mul_floor(total);
            let storage_share = T::StorageSharePermill::get().mul_floor(total);
            // reaction_share は割り算丸めの差分も拾う形で `total - miner - storage` ではなく
            // 明示的な mul_floor を使う。3 取り分の合計が `total` を上回らないことを保証するため、
            // 余り (=burn) は呼び出し元側 = ここで暗黙に Currency に戻ってきていない。
            // つまり余りは新規 mint されないので max supply の整合は崩れない。
            let reaction_share = T::ReactionSharePermill::get().mul_floor(total);

            // miner: Currency::deposit_creating で実 mint
            if miner_share > BalanceOf::<T>::default() {
                let _ = T::Currency::deposit_creating(&author, miner_share);
            }

            // storage / reaction pool: u128 で sink へ deposit (deferred mint authority)
            let storage_u128: u128 = storage_share.saturated_into();
            let reaction_u128: u128 = reaction_share.saturated_into();
            if storage_u128 > 0 {
                T::StoragePoolSink::do_deposit(storage_u128);
            }
            if reaction_u128 > 0 {
                T::ReactionPoolSink::do_deposit(reaction_u128);
            }

            // レガシーイベントは miner 取り分で埋める (互換性維持)
            Self::deposit_event(Event::BlockRewardMinted { author: author.clone(), amount: miner_share });
            // 詳細イベントを emit (Grafana / 分析用)
            Self::deposit_event(Event::BlockRewardSplit {
                author,
                miner: miner_share,
                storage: storage_share,
                reaction: reaction_share,
            });
        }
    }
}

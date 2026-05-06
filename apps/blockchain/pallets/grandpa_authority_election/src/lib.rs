//! # GRANDPA Authority Election Pallet
//!
//! 直近 N=100 ブロックを採掘した miner の上位 K=10 を集計し、
//! `pallet_grandpa::schedule_change` で GRANDPA authority set をローテーションする。
//! sudo 介在なしの permissionless finality を実現。
//!
//! Reaction-mining (foreground PoW) とは別ドメインであることに注意。

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::traits::FindAuthor;
    use frame_system::pallet_prelude::*;
    use sp_consensus_grandpa::AuthorityId as GrandpaId;
    use sp_std::collections::btree_map::BTreeMap;
    use sp_std::prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_grandpa::Config {
        /// 集計 window (直近何ブロック)。spec §1 で 100 推奨。
        #[pallet::constant]
        type WindowSize: Get<u32>;
        /// authority set サイズ (top-K)。spec §1 で 10 推奨。
        #[pallet::constant]
        type AuthorityCount: Get<u32>;
        /// 何ブロック毎に rotation を実行するか。spec §4.4 で 600 推奨。
        #[pallet::constant]
        type RotationPeriod: Get<BlockNumberFor<Self>>;
        /// rotation を pallet_grandpa::schedule_change に渡す delay (blocks)。
        #[pallet::constant]
        type RotationDelay: Get<BlockNumberFor<Self>>;
        /// PoW author 抽出 (pallet_block_reward と同じ FindAuthor を runtime で wire-up する)。
        type AuthorOrigin: FindAuthor<Self::AccountId>;
    }

    /// 直近 N 件の author を保持する ring buffer。容量は `T::WindowSize` と一致する。
    #[pallet::storage]
    pub type RecentAuthors<T: Config> = StorageValue<
        _,
        BoundedVec<T::AccountId, T::WindowSize>,
        ValueQuery,
    >;

    /// マイナーが事前登録した GRANDPA key。top-K に入った時点で active 化。
    #[pallet::storage]
    pub type AuthorityKeys<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, GrandpaId>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AuthorityKeyRegistered { who: T::AccountId },
        AuthorityKeyUnregistered { who: T::AccountId },
        AuthoritySetRotated { count: u32 },
        AuthoritySetRotationSkipped { reason: SkipReason },
    }

    #[derive(
        Clone,
        Encode,
        Decode,
        parity_scale_codec::DecodeWithMemTracking,
        TypeInfo,
        RuntimeDebug,
        PartialEq,
        Eq,
        MaxEncodedLen,
    )]
    pub enum SkipReason {
        NoCandidates,
        ScheduleChangeFailed,
    }

    #[pallet::error]
    pub enum Error<T> {
        AlreadyRegistered,
        NotRegistered,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// マイナーが GRANDPA key を登録。top-K に入れば次 rotation で active 化。
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn register_grandpa_key(origin: OriginFor<T>, key: GrandpaId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!AuthorityKeys::<T>::contains_key(&who), Error::<T>::AlreadyRegistered);
            AuthorityKeys::<T>::insert(&who, key);
            Self::deposit_event(Event::AuthorityKeyRegistered { who });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn unregister_grandpa_key(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(AuthorityKeys::<T>::contains_key(&who), Error::<T>::NotRegistered);
            AuthorityKeys::<T>::remove(&who);
            Self::deposit_event(Event::AuthorityKeyUnregistered { who });
            Ok(())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(n: BlockNumberFor<T>) {
            let cap = T::WindowSize::get() as usize;
            // window=0 設定では rotation も成立しないので早期 return (defensive guard)。
            if cap == 0 {
                return;
            }

            // 1. 現ブロックの author を ring buffer に push
            let digest = <frame_system::Pallet<T>>::digest();
            let pre_runtime_iter = digest.logs.iter().filter_map(|l| l.as_pre_runtime());
            if let Some(author) = T::AuthorOrigin::find_author(pre_runtime_iter) {
                RecentAuthors::<T>::mutate(|w| {
                    if w.len() >= cap && !w.is_empty() {
                        let _ = w.remove(0);
                    }
                    let _ = w.try_push(author);
                });
            }

            // 2. rotation period 境界で authority set を再計算
            use sp_runtime::traits::SaturatedConversion;
            let n_u128: u128 = n.saturated_into();
            let period: u128 = T::RotationPeriod::get().saturated_into();
            if period == 0 || !n_u128.is_multiple_of(period) {
                return;
            }

            Self::rotate();
        }
    }

    impl<T: Config> Pallet<T> {
        /// top-K authority を集計し、`pallet_grandpa::schedule_change` を呼ぶ。
        pub fn rotate() {
            let recent = RecentAuthors::<T>::get();
            if recent.is_empty() {
                log::warn!(
                    target: "grandpa_election",
                    "no recent authors, skipping rotation"
                );
                Self::deposit_event(Event::AuthoritySetRotationSkipped {
                    reason: SkipReason::NoCandidates,
                });
                return;
            }

            // 出現回数集計
            let mut counts: BTreeMap<T::AccountId, u32> = BTreeMap::new();
            for a in recent.iter() {
                *counts.entry(a.clone()).or_insert(0) += 1;
            }

            // (count desc, account_id asc) で並び替え
            let mut ranked: Vec<(T::AccountId, u32)> = counts.into_iter().collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

            // top-K のうち AuthorityKeys 登録済みのものだけ採用
            let k = T::AuthorityCount::get() as usize;
            let mut new_set: Vec<(GrandpaId, u64)> = Vec::with_capacity(k);
            for (acc, _) in ranked.into_iter() {
                if new_set.len() >= k {
                    break;
                }
                if let Some(key) = AuthorityKeys::<T>::get(&acc) {
                    new_set.push((key, 1)); // weight = 1 で平等
                }
            }

            if new_set.is_empty() {
                log::warn!(
                    target: "grandpa_election",
                    "no candidates have registered GRANDPA keys, skipping rotation"
                );
                Self::deposit_event(Event::AuthoritySetRotationSkipped {
                    reason: SkipReason::NoCandidates,
                });
                return;
            }

            let count = new_set.len() as u32;
            match pallet_grandpa::Pallet::<T>::schedule_change(
                new_set,
                T::RotationDelay::get(),
                None,
            ) {
                Ok(()) => {
                    Self::deposit_event(Event::AuthoritySetRotated { count });
                }
                Err(_) => {
                    log::warn!(
                        target: "grandpa_election",
                        "pallet_grandpa::schedule_change failed"
                    );
                    Self::deposit_event(Event::AuthoritySetRotationSkipped {
                        reason: SkipReason::ScheduleChangeFailed,
                    });
                }
            }
        }
    }
}

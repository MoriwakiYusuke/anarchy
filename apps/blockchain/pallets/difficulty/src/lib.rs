//! # Difficulty Pallet
//!
//! Consensus PoW の難易度を LWMA-3 で動的調整する。
//! Reaction-mining (foreground PoW) とは別ドメインであることに注意。

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

mod lwma;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_core::U256;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_timestamp::Config {
        /// 目標ブロック時間 (ms)。spec §1 で 30_000 推奨。
        #[pallet::constant]
        type TargetBlockTime: Get<Self::Moment>;
        /// LWMA window 長。spec §1 で 60 推奨。
        #[pallet::constant]
        type DifficultyAdjustWindow: Get<u32>;
        /// 下限 difficulty (0 を防ぐ)。spec §1 で 10_000 推奨。
        #[pallet::constant]
        type MinDifficulty: Get<U256>;
    }

    #[pallet::storage]
    pub type CurrentDifficulty<T> = StorageValue<_, U256, ValueQuery>;

    /// 直近 window 件の (difficulty, timestamp_ms) を保持する ring buffer。
    /// 容量は `T::DifficultyAdjustWindow` と一致するので、Config 値を変えれば
    /// 自動的にストレージサイズも追従する。
    #[pallet::storage]
    pub type PastDifficultiesAndTimestamps<T: Config> = StorageValue<
        _,
        BoundedVec<(U256, T::Moment), T::DifficultyAdjustWindow>,
        ValueQuery,
    >;

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub initial_difficulty: U256,
        #[serde(skip)]
        pub _phantom: core::marker::PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            CurrentDifficulty::<T>::put(self.initial_difficulty.max(T::MinDifficulty::get()));
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_n: BlockNumberFor<T>) {
            Self::adjust();
        }
    }

    impl<T: Config> Pallet<T> {
        /// 現在の difficulty を返す (Runtime API 用ヘルパ)。
        pub fn current_difficulty() -> U256 {
            CurrentDifficulty::<T>::get()
        }

        /// on_finalize から呼ばれる。現ブロックの (diff, ts) を window に push し、
        /// 充填されていれば LWMA-3 で次 difficulty を計算する。
        fn adjust() {
            let cap = T::DifficultyAdjustWindow::get() as usize;
            // window=0 設定では LWMA を回せない (ZeroDivision / 即時 mutate panic)。
            // 早期 return することで現 difficulty を据え置く。Config 設定ミスへの defensive guard。
            if cap == 0 {
                return;
            }

            let now = <pallet_timestamp::Pallet<T>>::get();
            let cur_diff = CurrentDifficulty::<T>::get();

            PastDifficultiesAndTimestamps::<T>::mutate(|window| {
                if window.len() >= cap && !window.is_empty() {
                    // 先頭要素を除去して ring buffer をスライド
                    window.remove(0);
                }
                // try_push は cap に空きが無いと Err を返すが、上で必ず空けているので
                // 失敗時は capacity と len の不整合を意味する。debug_assert で気付けるように。
                let push_result = window.try_push((cur_diff, now));
                debug_assert!(push_result.is_ok(), "ring buffer push failed despite eviction");
                if push_result.is_err() {
                    return;
                }

                if window.len() < cap {
                    return; // window 未充填 → 据え置き
                }

                let target_ms: u64 = T::TargetBlockTime::get().try_into().ok().unwrap_or(30_000);
                let next = super::lwma::lwma3_next_difficulty(window.as_slice(), target_ms);
                let floor = T::MinDifficulty::get();
                CurrentDifficulty::<T>::put(next.max(floor));
            });
        }
    }
}

// ─── Runtime API ────────────────────────────────────────────────────────────
sp_api::decl_runtime_apis! {
    pub trait DifficultyApi {
        fn difficulty() -> sp_core::U256;
    }
}

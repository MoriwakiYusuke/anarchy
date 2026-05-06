//! # Base Fee Pallet (TSTS P2 — EIP-1559 風動的手数料)
//!
//! Anarchy のスパム制御は、ユーザーの post / DM extrinsic に **base_fee × bytes** を上乗せ burn する形で行う。
//! base_fee は毎ブロック EIP-1559 と同じ式で調整する:
//!
//! ```text
//! utilization = block_bytes_used / GasTargetBytesPerBlock
//! adj         = clamp(1 + (utilization − 1) / 8, 0.875 .. 1.125)
//! base_fee'   = clamp(base_fee × adj, BaseFeeMin .. BaseFeeMax)
//! ```
//!
//! - 平常時 (target 50%) は `base_fee` が `BaseFeeMin` に張り付き、ユーザコストはほぼゼロ
//! - 攻撃時 (block を埋め尽くされる) は `base_fee` が指数的に上がり、攻撃者の MORAL を有限時間で枯渇させる
//!   (TSTS 不変条件 I-5)
//!
//! ## API
//!
//! - `current_base_fee()`: post / DM の支払い計算で使う最新値
//! - `record_gas(bytes)`: extrinsic ごとに使用 bytes を記録 (on_finalize で集計し base_fee 調整)
//! - `BaseFeeProvider` trait: pallet 外部 (post / messaging) からの依存断ち

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

/// 他 pallet (post / messaging) が現在の base_fee を取得する trait。
pub trait BaseFeeProvider {
    /// 現在の base_fee (MORAL の最小単位 / byte). 0 を返すと「base fee 無効」と解釈される。
    fn current_base_fee() -> u128;
    /// この block の使用 bytes を記録する (extrinsic 内で呼ぶ)。
    fn record_gas(bytes: u32);
}

/// 何もしない default 実装 (base fee 機能を無効化したい runtime 用)。
impl BaseFeeProvider for () {
    fn current_base_fee() -> u128 {
        0
    }
    fn record_gas(_bytes: u32) {}
}

#[frame_support::pallet]
pub mod pallet {
    use super::BaseFeeProvider;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// 1 ブロックあたりのターゲット byte 使用量 (= 50% utilization の中心点)。
        /// spec で 50_000 (= 50 KB ≒ 12 posts × 4KB) 推奨。
        #[pallet::constant]
        type GasTargetBytesPerBlock: Get<u32>;

        /// base_fee の下限 (MORAL の最小単位 / byte). 平常時はここに張り付く。
        /// spec で 100 (= 1e-10 MORAL/byte) 推奨。
        #[pallet::constant]
        type BaseFeeMin: Get<u128>;

        /// base_fee の上限 (cap). spam 攻撃時の最大徴収レート。
        /// spec で 100_000_000_000 (= 0.1 MORAL/byte) 推奨。
        #[pallet::constant]
        type BaseFeeMax: Get<u128>;

        /// base_fee 初期値 (genesis). 通常 BaseFeeMin と同じで良い。
        #[pallet::constant]
        type BaseFeeInit: Get<u128>;
    }

    /// 現在の base_fee. genesis で `BaseFeeInit` から初期化、毎ブロック on_finalize で更新。
    #[pallet::storage]
    #[pallet::getter(fn base_fee)]
    pub type BaseFee<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// この block で消費された bytes の合計。on_finalize で base_fee 計算に使い、リセットする。
    #[pallet::storage]
    #[pallet::getter(fn gas_used_this_block)]
    pub type GasUsedThisBlock<T: Config> = StorageValue<_, u32, ValueQuery>;

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        #[serde(skip)]
        pub _phantom: core::marker::PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            let init = T::BaseFeeInit::get().max(T::BaseFeeMin::get());
            BaseFee::<T>::put(init);
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// base_fee がブロック境界で更新された
        BaseFeeUpdated { old_fee: u128, new_fee: u128, gas_used: u32 },
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_n: BlockNumberFor<T>) {
            let used = GasUsedThisBlock::<T>::take();
            let target = T::GasTargetBytesPerBlock::get().max(1);
            let cur = BaseFee::<T>::get().max(T::BaseFeeMin::get());

            // EIP-1559: adj = 1 + (used/target − 1) / 8, clamped to ±12.5%
            let new_fee = if used >= target {
                let over = used - target;
                // bump = cur × over / (target × 8) を加算
                let bump = cur.saturating_mul(over as u128) / (target as u128 * 8);
                cur.saturating_add(bump)
            } else {
                let under = target - used;
                let cut = cur.saturating_mul(under as u128) / (target as u128 * 8);
                cur.saturating_sub(cut)
            };

            // ±12.5% に clamp (大幅な振動を防止)
            let max_bump = cur.saturating_add(cur / 8);
            let min_cut = cur.saturating_sub(cur / 8);
            let bumped = new_fee.min(max_bump).max(min_cut);

            // BaseFeeMin..BaseFeeMax に clamp
            let final_fee = bumped
                .max(T::BaseFeeMin::get())
                .min(T::BaseFeeMax::get());

            if final_fee != cur {
                BaseFee::<T>::put(final_fee);
                Self::deposit_event(Event::BaseFeeUpdated {
                    old_fee: cur,
                    new_fee: final_fee,
                    gas_used: used,
                });
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// 現在の base_fee (MORAL/byte) を返す。`max(BaseFee, BaseFeeMin)` を保証。
        pub fn current_base_fee_value() -> u128 {
            BaseFee::<T>::get().max(T::BaseFeeMin::get())
        }

        /// この block の使用 bytes を加算する。post / DM extrinsic から呼ぶ。
        pub fn record_gas_internal(bytes: u32) {
            GasUsedThisBlock::<T>::mutate(|g| *g = g.saturating_add(bytes));
        }
    }

    impl<T: Config> BaseFeeProvider for Pallet<T> {
        fn current_base_fee() -> u128 {
            Self::current_base_fee_value()
        }
        fn record_gas(bytes: u32) {
            Self::record_gas_internal(bytes);
        }
    }
}

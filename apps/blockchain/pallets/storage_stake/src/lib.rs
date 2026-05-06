//! # Storage Stake Pallet (TSTS P4)
//!
//! Storage node に skin-in-the-game を導入する bond 管理 pallet。
//!
//! ## 設計
//!
//! - ノード参加には `bond(declared_capacity_bytes)` で `BondPerGB × ⌈capacity_GB⌉` をロックする
//! - slashing は `do_slash_bond(node, amount)` を pallet_storage 側から呼ぶ
//!   (proof 失敗の重み付けに応じた段階 slash)
//! - bond 解放は `request_release` → `BondReleaseDelay` 経過後に `finalize_release`
//! - slash 配分: `SlashBurnSharePermill` を burn (Currency::slash で burn 経路へ)、残りは
//!   `RepairFundsRecipient` に送金 (現実装では burn のみ。後続 PR で repair 経路と接続)
//!
//! ## Sybil 抑制 (TSTS 不変条件 I-4)
//!
//! Sybil 大量参加 (1000 Sybil 各 1 GB) には `1000 × BondPerGB × 1` MORAL の流動性破壊が必要。
//! `BondPerGB = 10 MORAL` で 10,000 MORAL → bootstrap 困難な閾値を設ける。
//!
//! ## pallet_storage との結合
//!
//! pallet_storage は本 pallet の `BondInfo` trait を `Config::StakeProvider` で受け取り、
//! 報酬計算と slashing で参照する。runtime 側で `pallet_storage_stake::Pallet<Runtime>` を adapter する。

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

/// pallet_storage や runtime 層が bond 情報を読むための薄い trait。
///
/// `pallet_storage_stake` への依存を pallet_storage から切り離すために用意する。
/// runtime crate でこの trait を `pallet_storage_stake::Pallet<Runtime>` で impl する。
pub trait BondInfo<AccountId> {
    /// 指定アカウントが bond 済みかどうか。
    fn has_bond(who: &AccountId) -> bool;
    /// bond 残高 (u128 単位)。bond 無しなら 0。
    fn bond_amount(who: &AccountId) -> u128;
    /// 全体の active bond 合計 (u128). 0 のときは「未確立」と解釈し、
    /// 報酬式は `pool_ratio` のみで動く。
    fn total_active_bond() -> u128;
    /// `do_slash_bond` の薄い wrapper. amount は u128 単位。
    /// 戻り値: 実際に slash された額 (bond 不足で saturate されることがある)。
    fn slash_bond(who: &AccountId, amount: u128) -> u128;
}

#[frame_support::pallet]
pub mod pallet {
    use super::BondInfo;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::{Currency, ReservableCurrency};
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{SaturatedConversion, Saturating};
    use sp_runtime::Permill;

    pub type BalanceOf<T> = <<T as Config>::Currency as Currency<
        <T as frame_system::Config>::AccountId,
    >>::Balance;

    /// 1 GB = 1_073_741_824 bytes (= 2^30). 切り上げ計算で使う。
    pub const BYTES_PER_GB: u64 = 1_073_741_824;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Reservable currency (storage stake bond の reserve / unreserve / slash 用)。
        type Currency: ReservableCurrency<Self::AccountId>;

        /// 1 GB あたりの bond 額 (TSTS spec §3.2.5 で 10 MORAL 推奨)。
        #[pallet::constant]
        type BondPerGB: Get<BalanceOf<Self>>;

        /// 最小宣言容量 (バイト)。デフォルト 1 GB。
        #[pallet::constant]
        type MinDeclaredCapacity: Get<u64>;

        /// `request_release` から `finalize_release` 可能になるまでの待機ブロック数。
        /// spec で 7 日 = 100,800 blocks 推奨。
        #[pallet::constant]
        type BondReleaseDelay: Get<BlockNumberFor<Self>>;

        /// slash 額のうち burn する割合 (Permill)。残りは保持 (将来 repair pool 経路接続)。
        /// spec で 30% = Permill::from_percent(30) 推奨。
        #[pallet::constant]
        type SlashBurnSharePermill: Get<Permill>;
    }

    /// 各ノードの bond 情報。
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
    #[scale_info(skip_type_params(T))]
    pub struct Bond<T: Config> {
        /// 現在 reserve されている bond 額。
        pub amount: BalanceOf<T>,
        /// 宣言容量 (バイト)。
        pub declared_capacity_bytes: u64,
        /// bond した block。
        pub bonded_at: BlockNumberFor<T>,
        /// `request_release` した block。`Some` なら release 待ち、`None` ならアクティブ。
        pub release_requested_at: Option<BlockNumberFor<T>>,
    }

    /// アカウント → Bond.
    #[pallet::storage]
    #[pallet::getter(fn bonds)]
    pub type Bonds<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Bond<T>, OptionQuery>;

    /// active な bond の合計 (release 中も含む。slash されると即時減算)。
    #[pallet::storage]
    #[pallet::getter(fn total_active_bond)]
    pub type TotalActiveBond<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Bonded {
            who: T::AccountId,
            amount: BalanceOf<T>,
            declared_capacity_bytes: u64,
        },
        ReleaseRequested {
            who: T::AccountId,
            release_at: BlockNumberFor<T>,
        },
        Released {
            who: T::AccountId,
            amount: BalanceOf<T>,
        },
        Slashed {
            who: T::AccountId,
            amount: BalanceOf<T>,
            burned: BalanceOf<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// 既に bond 済み (上書き禁止)
        AlreadyBonded,
        /// 宣言容量が `MinDeclaredCapacity` 未満
        CapacityTooLow,
        /// 残高不足で reserve できない
        InsufficientBalance,
        /// bond していない
        NotBonded,
        /// `request_release` 未呼び出し
        ReleaseNotRequested,
        /// `BondReleaseDelay` 未経過
        ReleaseStillPending,
        /// release 中なので操作不可
        BondInRelease,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Storage node が容量を宣言して bond する。
        ///
        /// `BondPerGB × ⌈declared_capacity_bytes / 1GB⌉` を `Currency::reserve` する。
        /// 重複 bond は `AlreadyBonded` で失敗 (TSTS v1: 単純化のため重ねがけ不可)。
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2, 2))]
        pub fn bond(
            origin: OriginFor<T>,
            declared_capacity_bytes: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!Bonds::<T>::contains_key(&who), Error::<T>::AlreadyBonded);
            ensure!(
                declared_capacity_bytes >= T::MinDeclaredCapacity::get(),
                Error::<T>::CapacityTooLow
            );

            // ⌈capacity / 1GB⌉
            let gb = declared_capacity_bytes
                .saturating_add(BYTES_PER_GB - 1)
                / BYTES_PER_GB;
            let gb_balance: BalanceOf<T> = (gb as u128).saturated_into();
            let bond_amount = T::BondPerGB::get().saturating_mul(gb_balance);

            T::Currency::reserve(&who, bond_amount)
                .map_err(|_| Error::<T>::InsufficientBalance)?;

            Bonds::<T>::insert(
                &who,
                Bond {
                    amount: bond_amount,
                    declared_capacity_bytes,
                    bonded_at: frame_system::Pallet::<T>::block_number(),
                    release_requested_at: None,
                },
            );
            TotalActiveBond::<T>::mutate(|t| *t = t.saturating_add(bond_amount));

            Self::deposit_event(Event::Bonded {
                who,
                amount: bond_amount,
                declared_capacity_bytes,
            });
            Ok(())
        }

        /// release 申請。`BondReleaseDelay` 後に `finalize_release` 呼出可能。
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn request_release(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Bonds::<T>::try_mutate(&who, |maybe| -> DispatchResult {
                let bond = maybe.as_mut().ok_or(Error::<T>::NotBonded)?;
                ensure!(bond.release_requested_at.is_none(), Error::<T>::BondInRelease);
                let now = frame_system::Pallet::<T>::block_number();
                bond.release_requested_at = Some(now);
                let release_at = now.saturating_add(T::BondReleaseDelay::get());
                Self::deposit_event(Event::ReleaseRequested {
                    who: who.clone(),
                    release_at,
                });
                Ok(())
            })?;
            Ok(())
        }

        /// release 確定。`BondReleaseDelay` 経過後にのみ成功。reserve を解除し bond エントリを削除。
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 2))]
        pub fn finalize_release(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let bond = Bonds::<T>::get(&who).ok_or(Error::<T>::NotBonded)?;
            let requested = bond.release_requested_at.ok_or(Error::<T>::ReleaseNotRequested)?;
            let now = frame_system::Pallet::<T>::block_number();
            ensure!(
                now >= requested.saturating_add(T::BondReleaseDelay::get()),
                Error::<T>::ReleaseStillPending
            );

            let amount = bond.amount;
            let _ = T::Currency::unreserve(&who, amount);
            Bonds::<T>::remove(&who);
            TotalActiveBond::<T>::mutate(|t| *t = t.saturating_sub(amount));
            Self::deposit_event(Event::Released { who, amount });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// 内部 slash. pallet_storage の `do_slash_node` から runtime adapter 経由で呼ばれる。
        ///
        /// `amount` 分を bond から減算し、`SlashBurnSharePermill` 分を burn (slash) する。
        /// 残りは将来的に repair pool に流す予定だが、現実装では reserve 解除のみ
        /// (実 balance には戻さない = 中間状態として burn と等価)。
        ///
        /// 戻り値: 実際に slash された額 (bond 不足で saturate されたら少なくなる)。
        pub fn do_slash_bond(who: &T::AccountId, amount: u128) -> u128 {
            let mut actual: BalanceOf<T> = 0u32.into();

            let _ = Bonds::<T>::try_mutate(who, |maybe| -> Result<(), DispatchError> {
                let Some(bond) = maybe.as_mut() else {
                    return Ok(()); // bond 無しなら no-op
                };
                let amount_balance: BalanceOf<T> = amount.saturated_into();
                let slash = bond.amount.min(amount_balance);
                if slash == 0u32.into() {
                    return Ok(());
                }
                bond.amount = bond.amount.saturating_sub(slash);
                actual = slash;

                // Currency::slash_reserved で reserve から物理 burn
                let (_imbalance, _missing) = T::Currency::slash_reserved(who, slash);
                // _imbalance は drop で自動的に negative imbalance として burn される

                // total active bond を更新
                TotalActiveBond::<T>::mutate(|t| *t = t.saturating_sub(slash));

                // bond が 0 になったらエントリ削除
                if bond.amount == 0u32.into() {
                    *maybe = None;
                }
                Ok(())
            });

            // SlashBurnSharePermill 比率での個別処理は本 pallet では行わず、
            // slash_reserved による burn のみ行う (= 100% burn 相当)。
            // 30% burn / 70% repair の細かい配分は P4 の後続 PR で
            // RepairRewardPools 経路と接続する。

            let burned = actual; // 100% 相当 burn
            Self::deposit_event(Event::Slashed {
                who: who.clone(),
                amount: actual,
                burned,
            });

            actual.saturated_into()
        }
    }

    impl<T: Config> BondInfo<T::AccountId> for Pallet<T> {
        fn has_bond(who: &T::AccountId) -> bool {
            Bonds::<T>::contains_key(who)
        }

        fn bond_amount(who: &T::AccountId) -> u128 {
            Bonds::<T>::get(who)
                .map(|b| b.amount.saturated_into::<u128>())
                .unwrap_or(0)
        }

        fn total_active_bond() -> u128 {
            TotalActiveBond::<T>::get().saturated_into::<u128>()
        }

        fn slash_bond(who: &T::AccountId, amount: u128) -> u128 {
            Self::do_slash_bond(who, amount)
        }
    }
}

/// Stub `BondInfo` for tests/runtime contexts that don't enable storage_stake.
///
/// `has_bond` returns `true` (skin-in-the-game オフ)、`bond_amount` は 0、
/// `total_active_bond` も 0 → pool_ratio decay のみが効く形に縮退する。
pub struct NoBond;
impl<AccountId> BondInfo<AccountId> for NoBond {
    fn has_bond(_who: &AccountId) -> bool {
        true
    }
    fn bond_amount(_who: &AccountId) -> u128 {
        0
    }
    fn total_active_bond() -> u128 {
        0
    }
    fn slash_bond(_who: &AccountId, _amount: u128) -> u128 {
        0
    }
}

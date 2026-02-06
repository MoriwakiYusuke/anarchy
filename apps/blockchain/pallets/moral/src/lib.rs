//! # Moral Token Pallet
//!
//! $moral トークンの管理を行うパレット。
//! - 発行（mint）: 報酬として新規発行
//! - 焼却（burn）: 投稿コストとして消費
//! - 転送（transfer）: ユーザー間送金

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{CheckedAdd, CheckedSub, Zero};
    use sp_std::vec::Vec;

    pub type BalanceOf<T> = <T as Config>::Balance;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// イベント型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 残高の型
        type Balance: Parameter
            + Member
            + sp_runtime::traits::AtLeast32BitUnsigned
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + MaxEncodedLen
            + TypeInfo
            + CheckedAdd
            + CheckedSub
            + Zero;

        /// 新規アカウントへの初期配布量
        #[pallet::constant]
        type InitialBalance: Get<BalanceOf<Self>>;
    }

    /// Genesis設定
    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// 初期残高: (アカウント, 残高) のリスト
        pub balances: Vec<(T::AccountId, BalanceOf<T>)>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            let mut total: BalanceOf<T> = Zero::zero();
            for (account, balance) in &self.balances {
                Balances::<T>::insert(account, balance);
                total = total.checked_add(balance).expect("Total supply overflow at genesis");
            }
            TotalSupply::<T>::put(total);
        }
    }

    /// 残高ストレージ
    #[pallet::storage]
    #[pallet::getter(fn balance_of)]
    pub type Balances<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    /// 総供給量
    #[pallet::storage]
    #[pallet::getter(fn total_supply)]
    pub type TotalSupply<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// トークンが発行された
        Minted {
            who: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// トークンが焼却された
        Burned {
            who: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// トークンが転送された
        Transferred {
            from: T::AccountId,
            to: T::AccountId,
            amount: BalanceOf<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// 残高不足
        InsufficientBalance,
        /// オーバーフロー
        Overflow,
        /// 自分自身への転送
        SelfTransfer,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// トークンを転送する
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn transfer(
            origin: OriginFor<T>,
            to: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let from = ensure_signed(origin)?;
            ensure!(from != to, Error::<T>::SelfTransfer);

            Self::do_transfer(&from, &to, amount)?;

            Self::deposit_event(Event::Transferred { from, to, amount });
            Ok(())
        }

        /// トークンを焼却する（投稿コストなど）
        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn burn(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_burn(&who, amount)?;
            Ok(())
        }

        /// トークンを発行する（Sudo権限が必要）
        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn mint(origin: OriginFor<T>, to: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            ensure_root(origin)?;
            Self::do_mint(&to, amount)?;
            Ok(())
        }

        /// 新規アカウントに初期トークンを配布（faucet）
        #[pallet::call_index(3)]
        #[pallet::weight(10_000)]
        pub fn claim_initial(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 既に残高がある場合は拒否
            ensure!(
                Balances::<T>::get(&who).is_zero(),
                Error::<T>::InsufficientBalance
            );

            Self::do_mint(&who, T::InitialBalance::get())?;
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// 内部転送処理
        pub fn do_transfer(
            from: &T::AccountId,
            to: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let from_balance = Balances::<T>::get(from);
            let to_balance = Balances::<T>::get(to);

            let new_from = from_balance
                .checked_sub(&amount)
                .ok_or(Error::<T>::InsufficientBalance)?;
            let new_to = to_balance
                .checked_add(&amount)
                .ok_or(Error::<T>::Overflow)?;

            Balances::<T>::insert(from, new_from);
            Balances::<T>::insert(to, new_to);

            Ok(())
        }

        /// 内部発行処理
        pub fn do_mint(to: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            let balance = Balances::<T>::get(to);
            let new_balance = balance.checked_add(&amount).ok_or(Error::<T>::Overflow)?;

            let total = TotalSupply::<T>::get();
            let new_total = total.checked_add(&amount).ok_or(Error::<T>::Overflow)?;

            Balances::<T>::insert(to, new_balance);
            TotalSupply::<T>::put(new_total);

            Self::deposit_event(Event::Minted {
                who: to.clone(),
                amount,
            });
            Ok(())
        }

        /// 内部焼却処理
        pub fn do_burn(from: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            let balance = Balances::<T>::get(from);
            let new_balance = balance
                .checked_sub(&amount)
                .ok_or(Error::<T>::InsufficientBalance)?;

            let total = TotalSupply::<T>::get();
            let new_total = total.checked_sub(&amount).unwrap_or_default();

            Balances::<T>::insert(from, new_balance);
            TotalSupply::<T>::put(new_total);

            Self::deposit_event(Event::Burned {
                who: from.clone(),
                amount,
            });
            Ok(())
        }
    }
}

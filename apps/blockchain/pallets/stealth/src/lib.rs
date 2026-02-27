//! # Stealth Pallet
//!
//! ステルスアドレス送金とエフェメラル公開鍵の記録を担当するパレット。
//! EIP-5564互換プロトコルを採用。

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

mod types;
pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub use types::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement},
    };
    use frame_system::pallet_prelude::*;

    /// Currency type alias
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Configuration trait
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Runtime event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Currency type for transfers
        type Currency: Currency<Self::AccountId>;

        /// Maximum ephemeral key entries per block
        #[pallet::constant]
        type MaxEntriesPerBlock: Get<u32>;

        /// Weight information
        type WeightInfo: WeightInfo;
    }

    /// ブロック番号ごとのエフェメラル公開鍵リスト
    #[pallet::storage]
    #[pallet::getter(fn ephemeral_keys)]
    pub type EphemeralKeys<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        BoundedVec<EphemeralKeyEntry<T::AccountId>, T::MaxEntriesPerBlock>,
        ValueQuery,
    >;

    /// Events
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// ステルス送金が実行された
        StealthTransfer {
            sender: T::AccountId,
            stealth_address: T::AccountId,
            amount: BalanceOf<T>,
        },
    }

    /// Errors
    #[pallet::error]
    pub enum Error<T> {
        /// 送金額がゼロ
        ZeroAmount,
        /// 当ブロックのエントリ上限超過
        TooManyEntriesInBlock,
        /// 送信者の残高不足
        InsufficientBalance,
    }

    /// Extrinsics
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// ステルスアドレスへの送金を実行し、エフェメラル公開鍵をオンチェーンに記録する。
        ///
        /// # Arguments
        /// * `stealth_address` - ワンタイムステルスアドレス
        /// * `ephemeral_pubkey` - 送信者が生成したエフェメラル公開鍵
        /// * `amount` - 送金額 (MORAL最小単位)
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::send_to_stealth())]
        pub fn send_to_stealth(
            origin: OriginFor<T>,
            stealth_address: T::AccountId,
            ephemeral_pubkey: [u8; 32],
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            // 送金額がゼロでないことを確認
            ensure!(amount > BalanceOf::<T>::from(0u32), Error::<T>::ZeroAmount);

            // 送金を実行
            T::Currency::transfer(
                &sender,
                &stealth_address,
                amount,
                ExistenceRequirement::KeepAlive,
            )?;

            // エフェメラル公開鍵を記録
            let current_block = <frame_system::Pallet<T>>::block_number();
            let entry = EphemeralKeyEntry {
                ephemeral_pubkey,
                stealth_address: stealth_address.clone(),
            };

            EphemeralKeys::<T>::try_mutate(current_block, |entries| {
                entries.try_push(entry).map_err(|_| Error::<T>::TooManyEntriesInBlock)
            })?;

            // イベントを発行
            Self::deposit_event(Event::StealthTransfer {
                sender,
                stealth_address,
                amount,
            });

            Ok(())
        }
    }
}

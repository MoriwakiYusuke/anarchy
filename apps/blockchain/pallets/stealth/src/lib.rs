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
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
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

    /// Stealth reward pool (TSTS P6).
    ///
    /// `pallet_messaging` の DM コスト 20% 還流 + 将来の追加流入経路を保持する。
    /// `claim_stealth_reward` extrinsic で受信実績に応じて分配する (現実装は単純按分予定)。
    /// 単位は MORAL の最小単位 (= 1e-12 MORAL)。
    #[pallet::storage]
    #[pallet::getter(fn stealth_reward_pool)]
    pub type StealthRewardPool<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// 受信エフェメラル公開鍵ごとの累積受信回数 (TSTS P6).
    ///
    /// `claim_stealth_reward` の対象選別に使う。匿名性を保つため stealth_address (= AccountId) ではなく
    /// `[u8; 32]` の ephemeral_pubkey をキーにする (sender 視点の単位)。
    #[pallet::storage]
    #[pallet::getter(fn recipient_receive_count)]
    pub type RecipientReceiveCount<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        [u8; 32],
        u32,
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
        /// Stealth reward pool に金額が deposit された (TSTS P6)
        StealthRewardDeposit { amount: u128 },
        /// 受信エフェメラル公開鍵の受信回数が increment された (TSTS P6)
        RecipientReceiveCounted { ephemeral_pubkey: [u8; 32], new_count: u32 },
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

    impl<T: Config> Pallet<T> {
        /// Stealth reward pool に額を deposit (TSTS P6).
        ///
        /// runtime 側 adapter から呼び出す想定。`pallet_messaging::StealthRewardInterface` を
        /// 直接 impl すると messaging↔stealth が循環するため、ここでは pallet 内部 helper として
        /// 公開し、`runtime/src/lib.rs` で trait 適合させる。
        pub fn deposit_to_reward_pool(amount: u128) {
            if amount == 0 {
                return;
            }
            StealthRewardPool::<T>::mutate(|p| *p = p.saturating_add(amount));
            Self::deposit_event(Event::StealthRewardDeposit { amount });
        }

        /// 受信エフェメラル公開鍵の受信回数を 1 increment する (TSTS P6).
        ///
        /// 同じ extrinsic で `deposit_to_reward_pool` と並行に呼ぶ。idempotent ではなく、
        /// 呼び出し回数 = 受信回数。匿名性のため AccountId ではなく ephemeral_pubkey をキーにする。
        pub fn record_recipient_receive(ephemeral_pubkey: [u8; 32]) {
            let new_count = RecipientReceiveCount::<T>::mutate(ephemeral_pubkey, |c| {
                *c = c.saturating_add(1);
                *c
            });
            Self::deposit_event(Event::RecipientReceiveCounted { ephemeral_pubkey, new_count });
        }
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
            )
            .map_err(|e| match e {
                sp_runtime::DispatchError::Token(sp_runtime::TokenError::FundsUnavailable) => {
                    Error::<T>::InsufficientBalance.into()
                }
                _ => e,
            })?;

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

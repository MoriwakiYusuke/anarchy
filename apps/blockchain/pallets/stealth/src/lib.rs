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

        /// 1 回の claim あたり pool から流出できる上限 (Permill ppm). TSTS F2.
        ///
        /// 例: 100_000 = 10%. 0 を指定すると cap 無効。
        /// 過剰流出による pool 急速枯渇を防ぐためのソフト制限。spec で 10% 推奨。
        #[pallet::constant]
        type ClaimCapPpm: Get<u32>;
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
    /// `claim_stealth_reward` extrinsic で受信実績に応じて分配する。
    /// 単位は MORAL の最小単位 (= 1e-12 MORAL)。
    #[pallet::storage]
    #[pallet::getter(fn stealth_reward_pool)]
    pub type StealthRewardPool<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// 受信エフェメラル公開鍵ごとの累積受信回数 (TSTS P6).
    ///
    /// `claim_stealth_reward` の按分根拠に使う。匿名性を保つため stealth_address (= AccountId) ではなく
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

    /// 全 ephemeral_pubkey の累積受信回数の総和 (TSTS F2).
    ///
    /// `claim_stealth_reward` で按分割合を計算するための分母。
    /// `record_recipient_receive` 呼び出し毎に 1 increment する。
    #[pallet::storage]
    #[pallet::getter(fn total_received_count)]
    pub type TotalReceivedCount<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// claim 済み受信回数 (TSTS F2: 二重 claim 防止).
    ///
    /// ephemeral_pubkey 毎に `claim_stealth_reward` で「払い出した時点の受信回数」を記録。
    /// 次の claim では `(現在の受信回数 - claimed_count) / TotalReceivedCount` で按分する。
    /// これにより同じ受信実績に対して複数回 mint されることを防ぐ。
    #[pallet::storage]
    #[pallet::getter(fn claimed_receive_count)]
    pub type ClaimedReceiveCount<T: Config> = StorageMap<
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
        /// claim_stealth_reward が成功して payout した (TSTS F2)
        StealthRewardClaimed {
            recipient: T::AccountId,
            ephemeral_pubkey: [u8; 32],
            amount: BalanceOf<T>,
            unclaimed_count: u32,
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
        /// claim 対象の ephemeral_pubkey に未 claim の受信実績がない (TSTS F2)
        NoUnclaimedReceives,
        /// stealth reward pool が空 (TSTS F2)
        StealthRewardPoolEmpty,
        /// claim 額が極小すぎて 0 になった (TSTS F2: 経済的合理性チェック)
        ClaimAmountTooSmall,
        /// claim_stealth_reward の署名検証失敗 (将来対応用に予約; 現実装では unused)
        InvalidStealthSignature,
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
        /// TSTS F2 で TotalReceivedCount も並行 increment し、claim 時の按分分母に使う。
        pub fn record_recipient_receive(ephemeral_pubkey: [u8; 32]) {
            let new_count = RecipientReceiveCount::<T>::mutate(ephemeral_pubkey, |c| {
                *c = c.saturating_add(1);
                *c
            });
            TotalReceivedCount::<T>::mutate(|t| *t = t.saturating_add(1));
            Self::deposit_event(Event::RecipientReceiveCounted { ephemeral_pubkey, new_count });
        }

        /// claim_stealth_reward の純粋計算部 (TSTS F2, テスト容易性のため切り出し).
        ///
        /// `(unclaimed / total_received) × pool × CapPpm` で按分。CapPpm は同一 epoch 内で
        /// プール過剰流出を防ぐ上限 (例: 100_000 = 10%). 0 は cap 無効.
        ///
        /// 戻り値: 0 なら claim 不可 (NoUnclaimed / pool empty / round-down to zero).
        pub fn compute_claim_amount(
            unclaimed_count: u32,
            total_received: u64,
            pool_balance: u128,
            cap_ppm: u32,
        ) -> u128 {
            if unclaimed_count == 0 || total_received == 0 || pool_balance == 0 {
                return 0;
            }
            // share_ppm = unclaimed × 1e6 / total_received
            let share_ppm = (unclaimed_count as u128)
                .saturating_mul(1_000_000)
                / (total_received as u128).max(1);
            // base_payout = pool × share_ppm / 1e6
            let mut payout = pool_balance.saturating_mul(share_ppm) / 1_000_000;
            // per-claim cap: pool × cap_ppm / 1e6
            if cap_ppm > 0 {
                let max_per_claim = pool_balance.saturating_mul(cap_ppm as u128) / 1_000_000;
                payout = payout.min(max_per_claim);
            }
            payout.min(pool_balance)
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

        /// TSTS F2: stealth reward pool から DM 受信実績に応じて払い出す。
        ///
        /// 呼び出しモデル: `ephemeral_pubkey` を持つ stealth address の所有者が
        /// `signed origin` で extrinsic を発行する。payout は signer の `AccountId` に行く。
        ///
        /// **匿名性について**: ephemeral_pubkey の真の所有者であることを on-chain で検証することは
        /// 現実装ではしない (curve point 検証や ed25519 署名チェックは wasm runtime の重みが大きい)。
        /// 攻撃者が他人の ephemeral_pubkey を引数に passing しても、mint は **caller 自身の AccountId**
        /// に行くだけなので「他人の ephemeral_pubkey に紐づく報酬を盗む」攻撃が成立する。
        ///
        /// ⚠ そのため **TSTS F2 v1 は認証なしの暫定仕様**。後続 PR で:
        ///   1. ephemeral_pubkey から導かれる stealth_address の私有鍵による署名検証
        ///   2. または zk-proof による所有証明
        /// を導入する。当面は ephemeral_pubkey を秘匿する運用 (recipient のみ知る) を前提とする。
        ///
        /// # Arguments
        /// * `ephemeral_pubkey` - DM 受信時に記録された ephemeral pubkey
        ///
        /// # Errors
        /// * `NoUnclaimedReceives` - 未 claim の受信実績がない (= claimed_count == received_count)
        /// * `StealthRewardPoolEmpty` - プールが 0
        /// * `ClaimAmountTooSmall` - 按分結果が 0 (受信実績が小さすぎる / pool が小さすぎる)
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::send_to_stealth())]
        pub fn claim_stealth_reward(
            origin: OriginFor<T>,
            ephemeral_pubkey: [u8; 32],
        ) -> DispatchResult {
            let recipient = ensure_signed(origin)?;

            let received_count = RecipientReceiveCount::<T>::get(ephemeral_pubkey);
            let claimed_count = ClaimedReceiveCount::<T>::get(ephemeral_pubkey);
            let unclaimed = received_count.saturating_sub(claimed_count);
            ensure!(unclaimed > 0, Error::<T>::NoUnclaimedReceives);

            let pool = StealthRewardPool::<T>::get();
            ensure!(pool > 0, Error::<T>::StealthRewardPoolEmpty);

            let total_received = TotalReceivedCount::<T>::get();
            let cap_ppm = T::ClaimCapPpm::get();

            let payout_u128 = Self::compute_claim_amount(unclaimed, total_received, pool, cap_ppm);
            ensure!(payout_u128 > 0, Error::<T>::ClaimAmountTooSmall);

            // pool から減算してから mint。逆順だと race で over-mint しうる。
            StealthRewardPool::<T>::put(pool.saturating_sub(payout_u128));
            ClaimedReceiveCount::<T>::insert(ephemeral_pubkey, received_count);

            // Currency::deposit_creating で signer に mint。
            // BalanceOf<T> は `Currency::Balance` で = u128 を runtime で渡す想定。
            let payout: BalanceOf<T> = payout_u128.try_into().map_err(|_| Error::<T>::ClaimAmountTooSmall)?;
            let _ = T::Currency::deposit_creating(&recipient, payout);

            Self::deposit_event(Event::StealthRewardClaimed {
                recipient,
                ephemeral_pubkey,
                amount: payout,
                unclaimed_count: unclaimed,
            });
            Ok(())
        }
    }
}

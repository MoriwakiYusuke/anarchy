//! # Messaging Pallet
//!
//! DM (Direct Message) のオンチェーン受付を担うパレット。送信者は ciphertext
//! の所在と整合性を表すメタデータ (MerkleRoot / ephemeral pubkey) のみをチェーン
//! に記録し、本文は `pallet-post` / `pallet-storage` の既存オフチェーンパイプ
//! ラインで配送される。
//!
//! 本 crate はフェーズ 2 時点では types・Config・Storage・Event・Error・
//! WeightInfo・Runtime API 宣言のみを提供する。各 extrinsic の実装はフェーズ 3
//! 以降 (T034–T036) で追加する。

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

mod types;
pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// ステルス用リワードプール (TSTS DM 還流先) への接続トレイト。
///
/// `pallet-reaction::ReactionInterface` と同じ形だが、pallet-messaging 独自の
/// インターフェースとして分離することで依存関係を最小に保ち、実装側 (runtime)
/// はこれを `pallet-stealth` のリワードプール、または任意の pool 実装に接続する。
///
/// TSTS P6 で `record_recipient_receive` を追加し、受信エフェメラル公開鍵ごとの
/// カウントを更新できるようにした (claim_stealth_reward の按分根拠)。
pub trait StealthRewardInterface {
    /// 指定量をステルスリワードプールに加算する。
    fn do_deposit_to_stealth_reward_pool(amount: u128);
    /// 受信エフェメラル公開鍵の受信回数を 1 増やす (TSTS P6)。
    fn record_recipient_receive(ephemeral_pubkey: [u8; 32]);
}

/// No-op 実装 (主にテスト/プレースホルダ用)。
impl StealthRewardInterface for () {
    fn do_deposit_to_stealth_reward_pool(_amount: u128) {}
    fn record_recipient_receive(_ephemeral_pubkey: [u8; 32]) {}
}

// Runtime API: フロントエンド scanner が効率的に DmDispatchesByBlock を取得
// するためのインターフェース。contracts/pallet-messaging-extrinsics.md §RA 参照。
//
// 注: `decl_runtime_apis!` マクロが `where` 節を内部で再付与するため、
// bound はトレイトヘッダ側ではなく `where` 節に書く (clippy
// `multiple_bound_locations` 警告回避)。
sp_api::decl_runtime_apis! {
    pub trait DmScanApi<AccountId>
    where
        AccountId: parity_scale_codec::Codec,
    {
        /// 指定ブロックの DM 発行エントリを取得。
        fn dispatches_at(block_number: u32) -> sp_std::vec::Vec<DmDispatch<AccountId>>;

        /// 指定アカウントの DM メタアドレスを取得。
        fn reception_key(account: AccountId) -> Option<DmMetaAddress>;

        /// `from_block..=to_block` の dispatches を一括取得。
        /// `to_block - from_block > 1024` の場合は空配列を返す (過剰スキャン防止)。
        fn dispatches_range(
            from_block: u32,
            to_block: u32,
        ) -> sp_std::vec::Vec<(u32, sp_std::vec::Vec<DmDispatch<AccountId>>)>;
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::fungible::{Inspect, Mutate},
    };
    use frame_system::pallet_prelude::*;
    use pallet_storage::StorageInterface;

    /// Currency 型エイリアス。`pallet-post` と同様に fungible API を採用。
    pub type BalanceOf<T> = <<T as Config>::NativeToken as Inspect<
        <T as frame_system::Config>::AccountId,
    >>::Balance;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Configuration trait.
    ///
    /// 契約: contracts/pallet-messaging-extrinsics.md §Dependencies。
    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// ネイティブトークン ($moral)。
        type NativeToken: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

        /// 80% 流入先の Storage Pallet インターフェース (既存 `pallet-post` と同じ)。
        type Storage: pallet_storage::StorageInterface<Self::AccountId, BlockNumberFor<Self>>;

        /// 10% 還流先のステルスリワードプールインターフェース。
        type StealthReward: StealthRewardInterface;

        /// 1 ブロックあたりの最大 dispatch 件数 (DoS 防止)。
        #[pallet::constant]
        type MaxDispatchesPerBlock: Get<u32>;

        /// DM 発行の固定コスト。
        #[pallet::constant]
        type DmBaseCost: Get<BalanceOf<Self>>;

        /// 1 byte あたりの追加コスト (ciphertext_len に対して)。
        #[pallet::constant]
        type DmByteCost: Get<BalanceOf<Self>>;

        /// 暗号文長の上限 (最大バケット値 = 262_144)。
        #[pallet::constant]
        type MaxDmCiphertextLen: Get<u64>;

        /// 重み情報 (benchmarking で生成 / stub)。
        type WeightInfo: WeightInfo;
    }

    // ------- Storage -------

    /// 受信者アカウント → 公開 DM メタアドレス。
    #[pallet::storage]
    pub type DmReceptionKeys<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, DmMetaAddress>;

    /// ブロック番号 → 当該ブロックで発行された DM の一覧。
    #[pallet::storage]
    pub type DmDispatchesByBlock<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        BoundedVec<DmDispatch<T::AccountId>, T::MaxDispatchesPerBlock>,
        ValueQuery,
    >;

    /// メッセージ ID カウンタ (単調増加のみ、participant と紐付かない)。
    #[pallet::storage]
    pub type NextMessageId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// MerkleRoot → message_id。storage-layer replay 防止用のユニークインデックス。
    /// data-model.md §1.4 に記した通り、永続肥大化は Phase 3.4 の GC で抑える方針。
    #[pallet::storage]
    pub type DmMessagesByRoot<T: Config> = StorageMap<_, Blake2_128Concat, [u8; 32], u64>;

    // ------- Events -------

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// DM メタアドレスが公開 (または更新) された。
        DmKeyPublished { account: T::AccountId },
        /// DM メタアドレスが取り消された。
        DmKeyRevoked { account: T::AccountId },
        /// DM が発行された。
        DmDispatched {
            message_id: u64,
            block_number: BlockNumberFor<T>,
            recipient_stealth: T::AccountId,
            ephemeral_pubkey: [u8; 32],
            /// MerkleRoot と同値 (frontend 側のクロスチェック用)。
            content_hash: [u8; 32],
        },
    }

    // ------- Errors -------

    #[pallet::error]
    pub enum Error<T> {
        /// 受信者が DM 受信鍵を公開していない、または既に revoke 済み。
        ReceptionKeyNotPublished,
        /// ciphertext_len がパディングバケットに一致しない。
        InvalidPaddingBucket,
        /// 同一 MerkleRoot が既に存在 (重複送信)。
        DuplicateContent,
        /// 当ブロックの dispatch 件数上限超過。
        TooManyDispatchesInBlock,
        /// k/n パラメータが不正 (`k == 0` / `k > n` / `n > 255`)。
        InvalidKNParameters,
        /// 送信者 (stealth account) の残高不足。
        InsufficientStealthBalance,
        /// 無効なメタアドレス (all-zero pubkey 等)。
        InvalidMetaAddress,
        /// コスト計算オーバーフロー。
        CostCalculationOverflow,
    }

    // ------- Calls -------

    /// `ciphertext_len` として受理する固定バケット (FR-026, R4)。
    const DM_PADDING_BUCKETS: [u64; 5] = [1_024, 4_096, 16_384, 65_536, 262_144];

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// E1: DM 受信メタアドレスを公開する (上書き可)。
        ///
        /// **検証方針 (2026-05-03)**:
        /// - all-zero / all-FF などの "明らかな garbage" は弾く
        /// - Ed25519 / X25519 の curve point としての decompressibility 検証は
        ///   ここでは行わない。理由:
        ///     1. Substrate runtime に curve25519/ed25519-dalek を入れると wasm
        ///        ランタイム肥大化 + ホスト関数互換性リスクが大きい
        ///     2. 不正な点を publish しても被害は publisher 自身のみ (誰も
        ///        その address に向けて DM を暗号化できない)
        ///     3. 真に検証すべきは送信側 — wasm-engine の `dm_derive_recipient_stealth`
        ///        が parse_meta_address 経由で curve decompression を行い、
        ///        失敗時に extrinsic を発行しないので、不正な on-chain meta は
        ///        利用者間で害を持たない
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::publish_dm_key())]
        pub fn publish_dm_key(
            origin: OriginFor<T>,
            meta_address: DmMetaAddress,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                meta_address.scan_pub != [0u8; 32]
                    && meta_address.spend_pub != [0u8; 32]
                    && meta_address.scan_pub != [0xFFu8; 32]
                    && meta_address.spend_pub != [0xFFu8; 32]
                    && meta_address.scan_pub != meta_address.spend_pub,
                Error::<T>::InvalidMetaAddress
            );

            DmReceptionKeys::<T>::insert(&who, meta_address);
            Self::deposit_event(Event::DmKeyPublished { account: who });
            Ok(())
        }

        /// E2: DM 受信メタアドレスを取り消す。
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::revoke_dm_key())]
        pub fn revoke_dm_key(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                DmReceptionKeys::<T>::contains_key(&who),
                Error::<T>::ReceptionKeyNotPublished
            );

            DmReceptionKeys::<T>::remove(&who);
            Self::deposit_event(Event::DmKeyRevoked { account: who });
            Ok(())
        }

        /// E3: DM のコンテンツ参照をチェーンに書き込む。
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::send_dm(*ciphertext_len as u32))]
        pub fn send_dm(
            origin: OriginFor<T>,
            recipient_stealth: T::AccountId,
            ephemeral_pubkey: [u8; 32],
            merkle_root: [u8; 32],
            k: u32,
            n: u32,
            ciphertext_len: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                k > 0 && k <= n && n <= 255,
                Error::<T>::InvalidKNParameters
            );
            ensure!(
                DM_PADDING_BUCKETS.contains(&ciphertext_len)
                    && ciphertext_len <= T::MaxDmCiphertextLen::get(),
                Error::<T>::InvalidPaddingBucket
            );
            ensure!(ephemeral_pubkey != [0u8; 32], Error::<T>::InvalidMetaAddress);
            ensure!(
                !DmMessagesByRoot::<T>::contains_key(merkle_root),
                Error::<T>::DuplicateContent
            );

            let current_block = <frame_system::Pallet<T>>::block_number();
            let current_len = DmDispatchesByBlock::<T>::decode_len(current_block).unwrap_or(0);
            ensure!(
                (current_len as u32) < T::MaxDispatchesPerBlock::get(),
                Error::<T>::TooManyDispatchesInBlock
            );

            // Cost = DmBaseCost + DmByteCost * ciphertext_len。BalanceOf<T> ↔ u128 往復
            // パターンは pallet-post と同じ (既存 codebase 踏襲)。
            let base_cost: u128 = T::DmBaseCost::get()
                .try_into()
                .map_err(|_| Error::<T>::CostCalculationOverflow)?;
            let byte_cost: u128 = T::DmByteCost::get()
                .try_into()
                .map_err(|_| Error::<T>::CostCalculationOverflow)?;
            let byte_total = (ciphertext_len as u128)
                .checked_mul(byte_cost)
                .ok_or(Error::<T>::CostCalculationOverflow)?;
            let total_cost_u128 = base_cost
                .checked_add(byte_total)
                .ok_or(Error::<T>::CostCalculationOverflow)?;
            let total_cost: BalanceOf<T> = total_cost_u128
                .try_into()
                .map_err(|_| Error::<T>::CostCalculationOverflow)?;

            // 全額を送信者 (sender_stealth) から焼却。Precision::Exact により残高不足時は
            // atomically 失敗 (storage/pool/event は未変更)。pallet-post と同じ扱い。
            T::NativeToken::burn_from(
                &who,
                total_cost,
                frame_support::traits::tokens::Preservation::Expendable,
                frame_support::traits::tokens::Precision::Exact,
                frame_support::traits::tokens::Fortitude::Polite,
            )
            .map_err(|_| Error::<T>::InsufficientStealthBalance)?;

            // TSTS v1: 50% storage / 20% stealth reward / 30% 永久 burn.
            // 旧モデル (80/10/10) からの主要な変更:
            //  - storage 80→50: post と整合 (DM も storage 報酬流入にする)
            //  - stealth 10→20: 還流配線完了で受信者にマイクロ報酬
            //  - burn 10→30: tail emission 0.5 MORAL/block を相殺するデフレ圧の強化
            // 詳細: docs/economic_model_proposal.md §3.2.4
            let storage_share = total_cost_u128.saturating_mul(50) / 100;
            let stealth_share = total_cost_u128.saturating_mul(20) / 100;
            T::Storage::do_deposit_to_reward_pool(storage_share);
            T::StealthReward::do_deposit_to_stealth_reward_pool(stealth_share);
            // TSTS P6: 受信ステルスのカウントを記録し claim_stealth_reward の按分根拠にする
            T::StealthReward::record_recipient_receive(ephemeral_pubkey);

            let message_id = NextMessageId::<T>::get();
            let next_id = message_id
                .checked_add(1)
                .ok_or(Error::<T>::CostCalculationOverflow)?;
            NextMessageId::<T>::put(next_id);

            let content = DmContentRef {
                root: merkle_root,
                k,
                n,
                ciphertext_len,
            };
            let dispatch = DmDispatch {
                recipient_stealth: recipient_stealth.clone(),
                ephemeral_pubkey,
                content,
            };
            DmDispatchesByBlock::<T>::try_append(current_block, dispatch)
                .map_err(|_| Error::<T>::TooManyDispatchesInBlock)?;
            DmMessagesByRoot::<T>::insert(merkle_root, message_id);

            Self::deposit_event(Event::DmDispatched {
                message_id,
                block_number: current_block,
                recipient_stealth,
                ephemeral_pubkey,
                content_hash: merkle_root,
            });
            Ok(())
        }
    }
}

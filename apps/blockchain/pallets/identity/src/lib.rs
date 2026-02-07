//! # Identity Pallet
//!
//! WebAuthn公開鍵をオンチェーンで管理し、「秘密鍵をユーザーに扱わせない」を実現するパレット。
//! Constitution原則 II. Keyless UX の中核実装。
//!
//! ## 機能
//! - Identity作成: WebAuthn公開鍵でIdentityを登録
//! - Passkey追加: 既存Identityに新しいデバイスを追加
//! - Passkey削除: 不要なデバイスを削除（最後の1つは削除不可）
//! - WebAuthn署名検証: WYSIWYS (What You See Is What You Sign) の実現

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_core::blake2_256;
    use sp_runtime::SaturatedConversion;
    use sp_std::vec::Vec;

    /// PasskeyId: 公開鍵のBlake2b-256ハッシュ
    pub type PasskeyId = [u8; 32];

    /// Passkey: WebAuthn公開鍵情報
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(MaxPublicKeyLength, MaxDeviceNameLength))]
    pub struct Passkey<MaxPublicKeyLength: Get<u32>, MaxDeviceNameLength: Get<u32>> {
        pub id: PasskeyId,
        pub public_key: BoundedVec<u8, MaxPublicKeyLength>,
        pub registered_at: u64,
        pub last_used_at: u64,
        pub device_name: Option<BoundedVec<u8, MaxDeviceNameLength>>,
    }

    /// Identity: ユーザーを一意に識別するエンティティ
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(MaxPasskeys, MaxPublicKeyLength, MaxDeviceNameLength))]
    pub struct Identity<MaxPasskeys: Get<u32>, MaxPublicKeyLength: Get<u32>, MaxDeviceNameLength: Get<u32>> {
        pub created_at: u64,
        pub passkeys: BoundedVec<Passkey<MaxPublicKeyLength, MaxDeviceNameLength>, MaxPasskeys>,
    }

    /// Type alias for easier usage
    pub type PasskeyOf<T> = Passkey<<T as Config>::MaxPublicKeyLength, <T as Config>::MaxDeviceNameLength>;
    pub type IdentityOf<T> = Identity<<T as Config>::MaxPasskeys, <T as Config>::MaxPublicKeyLength, <T as Config>::MaxDeviceNameLength>;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// イベント型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 1つのIdentityに紐付けられる最大Passkey数
        #[pallet::constant]
        type MaxPasskeys: Get<u32>;

        /// 公開鍵の最大バイト長
        #[pallet::constant]
        type MaxPublicKeyLength: Get<u32>;

        /// デバイス名の最大バイト長
        #[pallet::constant]
        type MaxDeviceNameLength: Get<u32>;
    }

    /// Identity ID → Identity データ
    #[pallet::storage]
    #[pallet::getter(fn identities)]
    pub type Identities<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, IdentityOf<T>, OptionQuery>;

    /// 次に発行する Identity ID
    #[pallet::storage]
    #[pallet::getter(fn next_identity_id)]
    pub type NextIdentityId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// PasskeyId → Identity ID（逆引き、重複チェック用）
    #[pallet::storage]
    #[pallet::getter(fn passkey_owner)]
    pub type PasskeyOwner<T: Config> =
        StorageMap<_, Blake2_128Concat, PasskeyId, u64, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Identity が作成された
        IdentityCreated { identity_id: u64, passkey_id: PasskeyId },
        /// Passkey が追加された
        PasskeyAdded { identity_id: u64, passkey_id: PasskeyId },
        /// Passkey が削除された
        PasskeyRemoved { identity_id: u64, passkey_id: PasskeyId },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Identity が存在しない
        IdentityNotFound,
        /// Passkey が既に登録されている（別のIdentityで使用中）
        PasskeyAlreadyRegistered,
        /// Passkey が見つからない
        PasskeyNotFound,
        /// Passkey の最大数に達した
        TooManyPasskeys,
        /// 最後の Passkey は削除できない
        CannotRemoveLastPasskey,
        /// 公開鍵が空
        EmptyPublicKey,
        /// 公開鍵が長すぎる
        PublicKeyTooLong,
        /// 認証されていない（将来のWebAuthn検証用）
        Unauthorized,
    }

    /// Helper: 公開鍵からPasskeyIdを導出
    pub fn derive_passkey_id(public_key: &[u8]) -> PasskeyId {
        blake2_256(public_key)
    }

    impl<T: Config> Pallet<T> {
        /// 公開鍵のバリデーション
        pub fn validate_public_key(public_key: &[u8]) -> DispatchResult {
            ensure!(!public_key.is_empty(), Error::<T>::EmptyPublicKey);
            ensure!(
                public_key.len() <= T::MaxPublicKeyLength::get() as usize,
                Error::<T>::PublicKeyTooLong
            );
            Ok(())
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 新規Identityを作成し、最初のPasskeyを登録する
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn register_identity(
            origin: OriginFor<T>,
            public_key: Vec<u8>,
            device_name: Option<Vec<u8>>,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // 公開鍵バリデーション
            Self::validate_public_key(&public_key)?;

            // PasskeyIdを導出
            let passkey_id = derive_passkey_id(&public_key);

            // 重複チェック
            ensure!(
                !PasskeyOwner::<T>::contains_key(passkey_id),
                Error::<T>::PasskeyAlreadyRegistered
            );

            // Identity ID発行
            let identity_id = NextIdentityId::<T>::get();
            NextIdentityId::<T>::put(identity_id.saturating_add(1));

            // 現在のブロック番号
            let now: u64 = frame_system::Pallet::<T>::block_number().saturated_into();

            // デバイス名をBoundedVecに変換
            let bounded_device_name = device_name
                .map(|name| {
                    BoundedVec::<u8, T::MaxDeviceNameLength>::try_from(name)
                        .map_err(|_| Error::<T>::PublicKeyTooLong)
                })
                .transpose()?;

            // Passkey構造体作成
            let passkey = PasskeyOf::<T> {
                id: passkey_id,
                public_key: BoundedVec::try_from(public_key)
                    .map_err(|_| Error::<T>::PublicKeyTooLong)?,
                registered_at: now,
                last_used_at: now,
                device_name: bounded_device_name,
            };

            // Identity構造体作成
            let mut passkeys = BoundedVec::<PasskeyOf<T>, T::MaxPasskeys>::default();
            passkeys
                .try_push(passkey)
                .map_err(|_| Error::<T>::TooManyPasskeys)?;

            let identity = IdentityOf::<T> {
                created_at: now,
                passkeys,
            };

            // Storage更新
            Identities::<T>::insert(identity_id, identity);
            PasskeyOwner::<T>::insert(passkey_id, identity_id);

            // イベント発行
            Self::deposit_event(Event::IdentityCreated {
                identity_id,
                passkey_id,
            });

            Ok(())
        }

        /// 既存のIdentityに新しいPasskeyを追加する
        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn add_passkey(
            origin: OriginFor<T>,
            identity_id: u64,
            public_key: Vec<u8>,
            device_name: Option<Vec<u8>>,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // 公開鍵バリデーション
            Self::validate_public_key(&public_key)?;

            // Identityが存在するか確認
            let mut identity =
                Identities::<T>::get(identity_id).ok_or(Error::<T>::IdentityNotFound)?;

            // PasskeyIdを導出
            let passkey_id = derive_passkey_id(&public_key);

            // 重複チェック
            ensure!(
                !PasskeyOwner::<T>::contains_key(passkey_id),
                Error::<T>::PasskeyAlreadyRegistered
            );

            // 現在のブロック番号
            let now: u64 = frame_system::Pallet::<T>::block_number().saturated_into();

            // デバイス名をBoundedVecに変換
            let bounded_device_name = device_name
                .map(|name| {
                    BoundedVec::<u8, T::MaxDeviceNameLength>::try_from(name)
                        .map_err(|_| Error::<T>::PublicKeyTooLong)
                })
                .transpose()?;

            // Passkey構造体作成
            let passkey = PasskeyOf::<T> {
                id: passkey_id,
                public_key: BoundedVec::try_from(public_key)
                    .map_err(|_| Error::<T>::PublicKeyTooLong)?,
                registered_at: now,
                last_used_at: now,
                device_name: bounded_device_name,
            };

            // Passkey追加（上限チェック）
            identity
                .passkeys
                .try_push(passkey)
                .map_err(|_| Error::<T>::TooManyPasskeys)?;

            // Storage更新
            Identities::<T>::insert(identity_id, identity);
            PasskeyOwner::<T>::insert(passkey_id, identity_id);

            // イベント発行
            Self::deposit_event(Event::PasskeyAdded {
                identity_id,
                passkey_id,
            });

            Ok(())
        }

        /// IdentityからPasskeyを削除する
        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn remove_passkey(
            origin: OriginFor<T>,
            identity_id: u64,
            passkey_id: PasskeyId,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // Identityが存在するか確認
            let mut identity =
                Identities::<T>::get(identity_id).ok_or(Error::<T>::IdentityNotFound)?;

            // 最後のPasskeyは削除不可
            ensure!(
                identity.passkeys.len() > 1,
                Error::<T>::CannotRemoveLastPasskey
            );

            // Passkeyを検索
            let position = identity
                .passkeys
                .iter()
                .position(|p| p.id == passkey_id)
                .ok_or(Error::<T>::PasskeyNotFound)?;

            // Passkey削除
            identity.passkeys.remove(position);

            // Storage更新
            Identities::<T>::insert(identity_id, identity);
            PasskeyOwner::<T>::remove(passkey_id);

            // イベント発行
            Self::deposit_event(Event::PasskeyRemoved {
                identity_id,
                passkey_id,
            });

            Ok(())
        }
    }
}

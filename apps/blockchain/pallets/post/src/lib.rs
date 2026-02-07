//! # Post Pallet
//!
//! 投稿機能を提供するパレット。
//! ユーザーは投稿を作成し、オンチェーンに永続化できる。
//! 投稿時には $moral トークンをコストとして消費する。
//!
//! ## WebAuthn署名検証
//! - create_post_with_webauthn: WebAuthn署名付きで投稿を作成（WYSIWYS）

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use pallet_identity::webauthn::{
        parse_authenticator_data, parse_client_data_json, verify_signature,
        verify_user_present, verify_wysiwys_challenge, ClientDataType,
    };
    use sha2::{Digest, Sha256};
    use sp_std::vec::Vec;

    /// 投稿データ構造
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
    pub struct Post<T: Config> {
        /// 投稿者のアカウントID
        pub author: T::AccountId,
        /// コンテンツハッシュ
        pub content_hash: [u8; 32],
        /// 投稿時刻（ブロック番号）
        pub created_at: BlockNumberFor<T>,
        /// 親投稿ID（リプライの場合）
        pub parent_id: Option<u64>,
    }

    /// WebAuthn署名データ (for off-chain use and events)
    #[derive(Clone, Encode, Decode, TypeInfo, RuntimeDebug, PartialEq, Eq)]
    pub struct WebAuthnSignatureData {
        /// authenticatorData（生バイト列）
        pub authenticator_data: Vec<u8>,
        /// clientDataJSON（UTF-8文字列）
        pub client_data_json: Vec<u8>,
        /// ECDSA署名（DER形式またはraw形式）
        pub signature: Vec<u8>,
    }

    impl WebAuthnSignatureData {
        pub fn new(authenticator_data: Vec<u8>, client_data_json: Vec<u8>, signature: Vec<u8>) -> Self {
            Self {
                authenticator_data,
                client_data_json,
                signature,
            }
        }
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_moral::Config + pallet_identity::Config {
        /// イベント型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        
        /// 投稿の最大長（バイト）
        #[pallet::constant]
        type MaxContentLength: Get<u32>;

        /// 投稿の基本コスト（バイト数に関係なく必ずかかる）
        #[pallet::constant]
        type PostBaseCost: Get<pallet_moral::BalanceOf<Self>>;

        /// 1バイトあたりの追加コスト
        #[pallet::constant]
        type PostByteCost: Get<pallet_moral::BalanceOf<Self>>;
    }

    /// 次の投稿ID
    #[pallet::storage]
    #[pallet::getter(fn next_post_id)]
    pub type NextPostId<T> = StorageValue<_, u64, ValueQuery>;

    /// 投稿ストレージ
    #[pallet::storage]
    #[pallet::getter(fn posts)]
    pub type Posts<T: Config> = StorageMap<_, Blake2_128Concat, u64, Post<T>>;

    /// コンテンツ本文ストレージ（post_id → content bytes）
    #[pallet::storage]
    #[pallet::getter(fn contents)]
    pub type Contents<T: Config> = StorageMap<_, Blake2_128Concat, u64, BoundedVec<u8, T::MaxContentLength>>;

    /// ユーザーごとの投稿ID一覧
    #[pallet::storage]
    #[pallet::getter(fn user_posts)]
    pub type UserPosts<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, ConstU32<1000>>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 新しい投稿が作成された
        PostCreated {
            post_id: u64,
            author: T::AccountId,
            content_hash: [u8; 32],
        },
        /// WebAuthn署名付きで投稿が作成された
        PostCreatedWithWebAuthn {
            post_id: u64,
            identity_id: u64,
            content_hash: [u8; 32],
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// コンテンツが長すぎる
        ContentTooLong,
        /// ユーザーの投稿数が上限に達した
        TooManyPosts,
        /// 親投稿が存在しない
        ParentPostNotFound,
        /// $moral残高不足
        InsufficientMoralBalance,
        /// Identityが存在しない
        IdentityNotFound,
        /// Passkeyが見つからない
        PasskeyNotFound,
        /// 署名検証に失敗
        InvalidSignature,
        /// challengeが投稿ハッシュと一致しない
        ChallengeMismatch,
        /// userPresentフラグが立っていない
        UserNotPresent,
        /// clientDataのtypeが"webauthn.get"でない
        InvalidClientDataType,
        /// COSE公開鍵のパースに失敗
        InvalidCoseKey,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 新しい投稿を作成する
        ///
        /// # Arguments
        /// * `content` - 投稿内容（オンチェーンに保存）
        /// * `parent_id` - 親投稿ID（リプライの場合）
        ///
        /// # Cost
        /// * 基本コスト + (バイト数 × バイト単価) の $moral トークンを消費
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn create_post(
            origin: OriginFor<T>,
            content: Vec<u8>,
            parent_id: Option<u64>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // コンテンツ長チェック
            ensure!(
                content.len() <= T::MaxContentLength::get() as usize,
                Error::<T>::ContentTooLong
            );

            // 親投稿の存在チェック
            if let Some(pid) = parent_id {
                ensure!(Posts::<T>::contains_key(pid), Error::<T>::ParentPostNotFound);
            }

            // バイト数に基づくコスト計算: 基本コスト + (バイト数 × バイト単価)
            let content_len = content.len() as u128;
            let base_cost: u128 = T::PostBaseCost::get().try_into().unwrap_or(0);
            let byte_cost: u128 = T::PostByteCost::get().try_into().unwrap_or(0);
            let total_cost = base_cost.saturating_add(content_len.saturating_mul(byte_cost));
            let cost = total_cost.try_into().unwrap_or(T::PostBaseCost::get());

            // $moralトークンを消費（投稿コスト）
            pallet_moral::Pallet::<T>::do_burn(&who, cost)
                .map_err(|_| Error::<T>::InsufficientMoralBalance)?;

            // コンテンツハッシュを計算
            let content_hash = sp_io::hashing::blake2_256(&content);

            // 投稿IDを取得・インクリメント
            let post_id = NextPostId::<T>::get();
            NextPostId::<T>::put(post_id.saturating_add(1));

            // 投稿メタデータを保存
            let post = Post {
                author: who.clone(),
                content_hash,
                created_at: frame_system::Pallet::<T>::block_number(),
                parent_id,
            };
            Posts::<T>::insert(post_id, post);

            // コンテンツ本文を保存
            let bounded_content: BoundedVec<u8, T::MaxContentLength> = content
                .try_into()
                .map_err(|_| Error::<T>::ContentTooLong)?;
            Contents::<T>::insert(post_id, bounded_content);

            // ユーザーの投稿一覧に追加
            UserPosts::<T>::try_mutate(&who, |posts| {
                posts.try_push(post_id).map_err(|_| Error::<T>::TooManyPosts)
            })?;

            // イベント発行
            Self::deposit_event(Event::PostCreated {
                post_id,
                author: who,
                content_hash,
            });

            Ok(())
        }

        /// WebAuthn署名付きで新しい投稿を作成する（WYSIWYS）
        ///
        /// # Arguments
        /// * `identity_id` - 投稿者のIdentity ID
        /// * `passkey_id` - 使用するPasskeyのID
        /// * `content` - 投稿内容（オンチェーンに保存）
        /// * `authenticator_data` - WebAuthn authenticatorData（生バイト列）
        /// * `client_data_json` - WebAuthn clientDataJSON（UTF-8文字列）
        /// * `signature` - ECDSA署名（DER形式またはraw形式）
        /// * `parent_id` - 親投稿ID（リプライの場合）
        ///
        /// # WebAuthn Verification
        /// 1. Identity と Passkey の存在確認
        /// 2. authenticatorData の検証（userPresent フラグ）
        /// 3. clientDataJSON の検証（type が "webauthn.get"）
        /// 4. WYSIWYS チャレンジの検証（content_hash が challenge に含まれている）
        /// 5. ECDSA P-256 署名の検証
        ///
        /// # Cost
        /// * 基本コスト + (バイト数 × バイト単価) の $moral トークンを消費
        #[pallet::call_index(1)]
        #[pallet::weight(50_000)]
        pub fn create_post_with_webauthn(
            origin: OriginFor<T>,
            identity_id: u64,
            passkey_id: [u8; 32],
            content: Vec<u8>,
            authenticator_data: Vec<u8>,
            client_data_json: Vec<u8>,
            signature: Vec<u8>,
            parent_id: Option<u64>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // コンテンツ長チェック
            ensure!(
                content.len() <= T::MaxContentLength::get() as usize,
                Error::<T>::ContentTooLong
            );

            // 親投稿の存在チェック
            if let Some(pid) = parent_id {
                ensure!(Posts::<T>::contains_key(pid), Error::<T>::ParentPostNotFound);
            }

            // Identityの取得
            let identity = pallet_identity::Identities::<T>::get(identity_id)
                .ok_or(Error::<T>::IdentityNotFound)?;

            // Passkeyの検索
            let passkey = identity
                .passkeys
                .iter()
                .find(|p| p.id == passkey_id)
                .ok_or(Error::<T>::PasskeyNotFound)?;

            // COSE公開鍵をパース
            let public_key = pallet_identity::cose::parse_cose_key(&passkey.public_key)
                .map_err(|_| Error::<T>::InvalidCoseKey)?;

            // コンテンツハッシュを計算 (SHA-256)
            let content_hash_sha256: [u8; 32] = Sha256::digest(&content).into();

            // authenticatorDataをパース
            let auth_data = parse_authenticator_data(&authenticator_data)
                .map_err(|_| Error::<T>::InvalidSignature)?;

            // userPresentフラグの検証
            verify_user_present(&auth_data).map_err(|_| Error::<T>::UserNotPresent)?;

            // clientDataJSONをパース
            let client_data = parse_client_data_json(&client_data_json)
                .map_err(|_| Error::<T>::InvalidSignature)?;

            // clientData.typeが"webauthn.get"であることを確認
            ensure!(
                client_data.type_ == ClientDataType::Get,
                Error::<T>::InvalidClientDataType
            );

            // WYSIWYSチャレンジの検証（challengeがcontent_hashを含むことを確認）
            verify_wysiwys_challenge(&client_data.challenge, &content_hash_sha256)
                .map_err(|_| Error::<T>::ChallengeMismatch)?;

            // 署名を検証
            verify_signature(
                &public_key,
                &authenticator_data,
                &client_data_json,
                &signature,
            )
            .map_err(|_| Error::<T>::InvalidSignature)?;

            // バイト数に基づくコスト計算
            let content_len = content.len() as u128;
            let base_cost: u128 = T::PostBaseCost::get().try_into().unwrap_or(0);
            let byte_cost: u128 = T::PostByteCost::get().try_into().unwrap_or(0);
            let total_cost = base_cost.saturating_add(content_len.saturating_mul(byte_cost));
            let cost = total_cost.try_into().unwrap_or(T::PostBaseCost::get());

            // $moralトークンを消費（投稿コスト）
            pallet_moral::Pallet::<T>::do_burn(&who, cost)
                .map_err(|_| Error::<T>::InsufficientMoralBalance)?;

            // コンテンツハッシュを計算 (Blake2-256 for storage)
            let content_hash = sp_io::hashing::blake2_256(&content);

            // 投稿IDを取得・インクリメント
            let post_id = NextPostId::<T>::get();
            NextPostId::<T>::put(post_id.saturating_add(1));

            // 投稿メタデータを保存
            let post = Post {
                author: who.clone(),
                content_hash,
                created_at: frame_system::Pallet::<T>::block_number(),
                parent_id,
            };
            Posts::<T>::insert(post_id, post);

            // コンテンツ本文を保存
            let bounded_content: BoundedVec<u8, T::MaxContentLength> = content
                .try_into()
                .map_err(|_| Error::<T>::ContentTooLong)?;
            Contents::<T>::insert(post_id, bounded_content);

            // ユーザーの投稿一覧に追加
            UserPosts::<T>::try_mutate(&who, |posts| {
                posts.try_push(post_id).map_err(|_| Error::<T>::TooManyPosts)
            })?;

            // イベント発行
            Self::deposit_event(Event::PostCreatedWithWebAuthn {
                post_id,
                identity_id,
                content_hash,
            });

            Ok(())
        }
    }
}

//! # Post Pallet
//!
//! 投稿機能を提供するパレット。
//! ユーザーは投稿を作成し、オンチェーンに永続化できる。
//! 投稿時には $moral トークン（ネイティブ）をコストとして消費する。

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::traits::fungible::{Inspect, Mutate};
    use frame_system::pallet_prelude::*;
    use sp_std::vec::Vec;

    /// $moral残高型（ネイティブトークン）
    pub type BalanceOf<T> = <<T as Config>::NativeToken as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

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

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        // RuntimeEvent is now inferred from frame_system::Config bound
        
        /// ネイティブトークン（$moral）
        type NativeToken: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

        /// 投稿の最大長（バイト）
        #[pallet::constant]
        type MaxContentLength: Get<u32>;

        /// 投稿の基本コスト（バイト数に関係なく必ずかかる）
        #[pallet::constant]
        type PostBaseCost: Get<BalanceOf<Self>>;

        /// 1バイトあたりの追加コスト
        #[pallet::constant]
        type PostByteCost: Get<BalanceOf<Self>>;
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
        #[pallet::weight(T::DbWeight::get().reads_writes(3, 4))]
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
            let cost: BalanceOf<T> = total_cost.try_into().unwrap_or(T::PostBaseCost::get());

            // $moralトークンを焼却（投稿コスト）
            T::NativeToken::burn_from(
                &who,
                cost,
                frame_support::traits::tokens::Preservation::Expendable,
                frame_support::traits::tokens::Precision::Exact,
                frame_support::traits::tokens::Fortitude::Polite,
            ).map_err(|_| Error::<T>::InsufficientMoralBalance)?;

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
    }
}

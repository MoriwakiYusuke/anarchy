//! # Post Pallet
//!
//! 投稿機能を提供するパレット。
//! ユーザーは投稿を作成し、オンチェーンに永続化できる。
//! 投稿時には $moral トークン（ネイティブ）をコストとして消費する。

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// V2コンテンツ参照（オフチェーン分散ストレージへの参照）
/// Runtime API用に再エクスポート
#[derive(Clone, Debug, Encode, Decode, TypeInfo, PartialEq, Eq)]
pub struct PostContentInfo {
    /// MerkleRoot (32バイト)
    pub root: [u8; 32],
    /// 復元に必要な最小断片数
    pub k: u32,
    /// 総断片数
    pub n: u32,
    /// 元データサイズ（バイト）
    pub size: u64,
}

sp_api::decl_runtime_apis! {
    /// Post Pallet Runtime API
    ///
    /// RPCからチェーン上のコンテンツ参照を取得するためのAPI
    pub trait PostApi {
        /// MerkleRootからコンテンツ参照を取得
        fn get_content_by_merkle_root(merkle_root: [u8; 32]) -> Option<PostContentInfo>;

        /// PostIDからコンテンツ参照を取得
        fn get_content_by_post_id(post_id: u64) -> Option<PostContentInfo>;
    }
}

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::traits::fungible::{Inspect, Mutate};
    use frame_system::pallet_prelude::*;
    use pallet_storage::StorageInterface;
    use pallet_reaction::ReactionInterface;
    use pallet_popularity::PopularityInterface;

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

    /// 分散ストレージ参照情報（V2: SSS+Merkle形式）
    ///
    /// 投稿コンテンツはオフチェーンStorage Nodeに断片化して保存され、
    /// オンチェーンにはこの参照情報のみを記録する。
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq)]
    pub struct PostContent {
        /// MerkleRoot（断片ハッシュのマークルツリールート）
        pub root: [u8; 32],
        /// 復元に必要な最小断片数（しきい値）
        pub k: u32,
        /// 総断片数
        pub n: u32,
        /// 元データサイズ（バイト）
        pub size: u64,
        /// 暗号文の長さ（AES-GCM暗号化後のサイズ）
        pub ciphertext_len: u64,
        /// シャードサイズ（Reed-Solomonエンコード後の各シャードのサイズ）
        pub shard_size: u32,
        /// 圧縮フラグ（データが圧縮されているかどうか）
        pub compressed: bool,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        // RuntimeEvent is now inferred from frame_system::Config bound
        
        /// ネイティブトークン（$moral）
        type NativeToken: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

        /// Storage Pallet for atomic fragment registration (FR-401, FR-402).
        type Storage: pallet_storage::StorageInterface<Self::AccountId, BlockNumberFor<Self>>;

        /// Reaction Pallet for reward pool deposits.
        type Reaction: pallet_reaction::ReactionInterface;

        /// Popularity sink — receives push notifications on post creation.
        type Popularity: pallet_popularity::PopularityInterface;

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

    /// コンテンツ参照ストレージ（V2: オフチェーン分散ストレージへの参照）
    /// post_id → PostContent (MerkleRoot + k/n/size)
    #[pallet::storage]
    #[pallet::getter(fn content_refs)]
    pub type ContentRefs<T: Config> = StorageMap<_, Blake2_128Concat, u64, PostContent>;

    /// MerkleRoot → PostID 逆引きマップ（RPC用）
    #[pallet::storage]
    #[pallet::getter(fn merkle_root_to_post_id)]
    pub type MerkleRootToPostId<T: Config> = StorageMap<_, Blake2_128Concat, [u8; 32], u64>;

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
        /// k/nパラメータが無効（k > 0 && k <= n が必須）
        InvalidKNParameters,
        /// 同一MerkleRootの投稿が既に存在する
        DuplicateMerkleRoot,
        /// コスト計算でオーバーフローが発生
        CostCalculationOverflow,
        /// 投稿IDがオーバーフロー
        PostIdOverflow,
        /// Fragment registration in Storage Pallet failed (FR-402)
        FragmentRegistrationFailed,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 分散ストレージ形式で投稿を作成
        ///
        /// フロントエンドでSSS分割・MerkleTree構築後、このextrinsicを呼び出す。
        /// 断片データは別途Storage RPCでアップロード済みであること。
        ///
        /// # Arguments
        /// * `merkle_root` - 断片ハッシュのMerkleRoot（投稿の一意識別子）
        /// * `k` - 復元に必要な最小断片数（SSS threshold）
        /// * `n` - 総断片数
        /// * `total_size` - 元データサイズ（バイト）
        /// * `ciphertext_len` - 暗号文の長さ（AES-GCM暗号化後のサイズ）
        /// * `shard_size` - シャードサイズ（Reed-Solomonエンコード後の各シャードのサイズ）
        /// * `compressed` - 圧縮フラグ（データが圧縮されているかどうか）
        /// * `parent_id` - 親投稿ID（リプライの場合）
        ///
        /// # Cost
        /// * `total_cost = PostBaseCost + total_size * PostByteCost`
        /// * 全額焼却（partial deposit ではない）
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 5))]
        pub fn create_post(
            origin: OriginFor<T>,
            merkle_root: [u8; 32],
            k: u32,
            n: u32,
            total_size: u64,
            ciphertext_len: u64,
            shard_size: u32,
            compressed: bool,
            parent_id: Option<u64>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // k/n検証: k > 0 && k <= n && n <= 255
            ensure!(k > 0 && k <= n && n <= 255, Error::<T>::InvalidKNParameters);

            // total_size上限チェック
            ensure!(
                total_size <= T::MaxContentLength::get() as u64,
                Error::<T>::ContentTooLong
            );

            // MerkleRoot重複チェック
            ensure!(
                !MerkleRootToPostId::<T>::contains_key(merkle_root),
                Error::<T>::DuplicateMerkleRoot
            );

            // 親投稿の存在チェック
            if let Some(pid) = parent_id {
                ensure!(Posts::<T>::contains_key(pid), Error::<T>::ParentPostNotFound);
            }

            // ユーザーの投稿数上限を先にチェック（トークン焼却前）
            let current_posts = UserPosts::<T>::get(&who);
            ensure!(!current_posts.is_full(), Error::<T>::TooManyPosts);

            // コスト計算: base_cost + size_cost
            let base_cost: u128 = T::PostBaseCost::get()
                .try_into()
                .map_err(|_| Error::<T>::CostCalculationOverflow)?;
            let byte_cost: u128 = T::PostByteCost::get()
                .try_into()
                .map_err(|_| Error::<T>::CostCalculationOverflow)?;
            let size_cost = (total_size as u128).saturating_mul(byte_cost);
            let total_cost = base_cost.saturating_add(size_cost);
            let cost: BalanceOf<T> = total_cost
                .try_into()
                .map_err(|_| Error::<T>::CostCalculationOverflow)?;

            // $moralトークンを焼却 (全額をユーザーから取る)
            T::NativeToken::burn_from(
                &who,
                cost,
                frame_support::traits::tokens::Preservation::Expendable,
                frame_support::traits::tokens::Precision::Exact,
                frame_support::traits::tokens::Fortitude::Polite,
            ).map_err(|_| Error::<T>::InsufficientMoralBalance)?;

            // FR-113: 80%をStorage報酬プールに蓄積、10%をReaction報酬プールに蓄積、10%は永久にburn
            let storage_pool_amount = total_cost.saturating_mul(80) / 100;
            let reaction_pool_amount = total_cost.saturating_mul(10) / 100;
            T::Storage::do_deposit_to_reward_pool(storage_pool_amount);
            T::Reaction::do_deposit_to_reaction_pool(reaction_pool_amount);

            // 投稿IDを取得・インクリメント（オーバーフロー防止）
            let post_id = NextPostId::<T>::get();
            let next_id = post_id.checked_add(1).ok_or(Error::<T>::PostIdOverflow)?;
            NextPostId::<T>::put(next_id);

            // 投稿メタデータを保存（content_hash = merkle_root）
            let post = Post {
                author: who.clone(),
                content_hash: merkle_root,
                created_at: frame_system::Pallet::<T>::block_number(),
                parent_id,
            };
            Posts::<T>::insert(post_id, post);

            // コンテンツ参照を保存
            let content_ref = PostContent {
                root: merkle_root,
                k,
                n,
                size: total_size,
                ciphertext_len,
                shard_size,
                compressed,
            };
            ContentRefs::<T>::insert(post_id, content_ref);

            // Register fragment in Storage Pallet atomically (FR-401, FR-402)
            // Fragment size is truncated to u32 (already validated by MaxContentLength)
            let fragment_size = total_size.min(u32::MAX as u64) as u32;
            T::Storage::do_register_fragment(
                merkle_root,
                fragment_size,
                who.clone(),
                frame_system::Pallet::<T>::block_number(),
            ).map_err(|_| Error::<T>::FragmentRegistrationFailed)?;

            // MerkleRoot → PostID 逆引きマップを保存（RPC用）
            MerkleRootToPostId::<T>::insert(merkle_root, post_id);

            // ユーザーの投稿一覧に追加
            UserPosts::<T>::try_mutate(&who, |posts| {
                posts.try_push(post_id).map_err(|_| Error::<T>::TooManyPosts)
            })?;

            // Initialize popularity entry for the new post.
            T::Popularity::on_post_created(post_id);

            // イベント発行
            Self::deposit_event(Event::PostCreated {
                post_id,
                author: who,
                content_hash: merkle_root,
            });

            Ok(())
        }
    }
}

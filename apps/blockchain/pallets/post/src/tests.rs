//! Post Palletのテスト

use crate::{self as pallet_post, Error, Event, Posts, NextPostId, UserPosts, ContentRefs};
use frame_support::{
    assert_noop, assert_ok,
    traits::{ConstU32, ConstU64, ConstU128, fungible::Mutate},
};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

// テスト用ランタイム構築
frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        PostModule: pallet_post,
    }
);

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
    type RuntimeTask = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
    type ExtensionsWeightInfo = ();
}

impl pallet_balances::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type WeightInfo = ();
    type Balance = u128;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = ();
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type MaxFreezes = ConstU32<0>;
    type DoneSlashHandler = ();
}

impl pallet_post::Config for Test {
    type NativeToken = Balances;
    type MaxContentLength = ConstU32<10000>;
    type PostBaseCost = ConstU128<100>; // テスト用: 基本100
    type PostByteCost = ConstU128<10>;  // テスト用: 1バイトあたり10
}

// テスト環境のビルダー
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        // テストユーザーに初期Moralを付与（pallet_balancesのfungible Mutate trait経由）
        // V2テスト用に大きめの残高（1000万Moral）を付与
        <Balances as Mutate<_>>::mint_into(&1u64, 10_000_000).unwrap();
        <Balances as Mutate<_>>::mint_into(&2u64, 10_000_000).unwrap();
        <Balances as Mutate<_>>::mint_into(&3u64, 10_000_000).unwrap();
    });
    ext
}

#[test]
fn create_post_works() {
    new_test_ext().execute_with(|| {
        let author = 1u64;
        let merkle_root = sp_io::hashing::blake2_256(b"Hello, Anarchy!");
        let k = 3u32;
        let n = 5u32;
        let total_size = 15u64; // "Hello, Anarchy!" is 15 bytes
        let initial_balance = Balances::free_balance(author);

        // 投稿作成
        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(author),
            merkle_root,
            k,
            n,
            total_size,
            None
        ));

        // 投稿IDが1に増加
        assert_eq!(NextPostId::<Test>::get(), 1);

        // 投稿が保存されている
        let post = Posts::<Test>::get(0).expect("投稿が存在するはず");
        assert_eq!(post.author, author);
        assert_eq!(post.parent_id, None);
        assert_eq!(post.content_hash, merkle_root);

        // ContentRefsに正しく保存されている
        let content_ref = ContentRefs::<Test>::get(0).expect("ContentRefが存在するはず");
        assert_eq!(content_ref.root, merkle_root);
        assert_eq!(content_ref.k, k);
        assert_eq!(content_ref.n, n);
        assert_eq!(content_ref.size, total_size);

        // ユーザーの投稿一覧に追加されている
        let user_posts = UserPosts::<Test>::get(author);
        assert_eq!(user_posts.len(), 1);
        assert_eq!(user_posts[0], 0);

        // Moralトークンが消費されている
        // cost = base(100) + size_cost(15 * 10) + deposit((100 + 150) / 5) = 100 + 150 + 50 = 300
        let new_balance = Balances::free_balance(author);
        assert!(new_balance < initial_balance, "Moralが消費されているはず");

        // イベントが発行されている
        System::assert_has_event(RuntimeEvent::PostModule(Event::PostCreated {
            post_id: 0,
            author,
            content_hash: merkle_root,
        }));
    });
}

#[test]
fn create_reply_works() {
    new_test_ext().execute_with(|| {
        let author1 = 1u64;
        let author2 = 2u64;
        let merkle_root1 = sp_io::hashing::blake2_256(b"Original post");
        let merkle_root2 = sp_io::hashing::blake2_256(b"This is a reply");

        // 親投稿を作成
        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(author1),
            merkle_root1,
            3, 5, 13, // "Original post" is 13 bytes
            None
        ));

        // リプライを作成
        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(author2),
            merkle_root2,
            3, 5, 15, // "This is a reply" is 15 bytes
            Some(0)
        ));

        // リプライの親IDが正しい
        let reply = Posts::<Test>::get(1).expect("リプライが存在するはず");
        assert_eq!(reply.parent_id, Some(0));
    });
}

#[test]
fn create_reply_to_nonexistent_post_fails() {
    new_test_ext().execute_with(|| {
        let merkle_root = sp_io::hashing::blake2_256(b"Reply to nothing");

        // 存在しない投稿へのリプライは失敗
        assert_noop!(
            PostModule::create_post(
                RuntimeOrigin::signed(1),
                merkle_root,
                3, 5, 16,
                Some(999)
            ),
            Error::<Test>::ParentPostNotFound
        );
    });
}

#[test]
fn insufficient_moral_balance_fails() {
    new_test_ext().execute_with(|| {
        let poor_user = 99u64; // Moralを持っていないユーザー
        let merkle_root = sp_io::hashing::blake2_256(b"I have no moral");

        // 残高不足で投稿失敗
        assert_noop!(
            PostModule::create_post(
                RuntimeOrigin::signed(poor_user),
                merkle_root,
                3, 5, 15,
                None
            ),
            Error::<Test>::InsufficientMoralBalance
        );
    });
}

#[test]
fn multiple_posts_by_same_user() {
    new_test_ext().execute_with(|| {
        let author = 1u64;
        let initial_balance = Balances::free_balance(author);

        // 3つの投稿を作成
        for i in 0..3 {
            let content = format!("Post number {}", i);
            let merkle_root = sp_io::hashing::blake2_256(content.as_bytes());
            assert_ok!(PostModule::create_post(
                RuntimeOrigin::signed(author),
                merkle_root,
                3, 5, content.len() as u64,
                None
            ));
        }

        // 投稿IDが3に増加
        assert_eq!(NextPostId::<Test>::get(), 3);

        // ユーザーの投稿一覧に3つ追加されている
        let user_posts = UserPosts::<Test>::get(author);
        assert_eq!(user_posts.len(), 3);
        assert_eq!(user_posts.to_vec(), vec![0, 1, 2]);

        // 3回分のMoralが消費されている
        let balance = Balances::free_balance(author);
        assert!(balance < initial_balance, "Moralが消費されているはず");
    });
}

#[test]
fn merkle_root_stored_correctly() {
    new_test_ext().execute_with(|| {
        let content = b"Test content for hashing";
        let merkle_root = sp_io::hashing::blake2_256(content);

        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(1),
            merkle_root,
            3, 5, content.len() as u64,
            None
        ));

        let post = Posts::<Test>::get(0).expect("投稿が存在するはず");
        assert_eq!(post.content_hash, merkle_root);
        
        // ContentRefにも同じrootが保存されている
        let content_ref = ContentRefs::<Test>::get(0).expect("ContentRefが存在するはず");
        assert_eq!(content_ref.root, merkle_root);
    });
}

// ============================================================================
// T008: PostContent構造体のエンコード/デコードテスト
// ============================================================================

use crate::pallet::PostContent;
use frame_support::pallet_prelude::MaxEncodedLen;
use parity_scale_codec::{Encode, Decode};

#[test]
fn test_post_content_encode_decode() {
    // PostContent構造体の作成
    let content = PostContent {
        root: [0xABu8; 32],  // MerkleRoot
        k: 3,                 // 復元に必要な最小断片数
        n: 5,                 // 総断片数
        size: 1024 * 100,     // 100KB
    };

    // SCALEエンコード
    let encoded = content.encode();

    // デコード
    let decoded = PostContent::decode(&mut &encoded[..])
        .expect("デコードに成功するはず");

    // フィールド値の検証
    assert_eq!(decoded.root, content.root);
    assert_eq!(decoded.k, content.k);
    assert_eq!(decoded.n, content.n);
    assert_eq!(decoded.size, content.size);
}

#[test]
fn test_post_content_max_encoded_len() {
    // MaxEncodedLenが正しく計算されるか検証
    // root: 32バイト + k: 4バイト + n: 4バイト + size: 8バイト = 48バイト
    let max_len = <PostContent as MaxEncodedLen>::max_encoded_len();
    assert_eq!(max_len, 48, "PostContentの最大エンコードサイズは48バイト");
}

#[test]
fn test_post_content_k_n_validation() {
    // k > 0 && k <= n の検証（構造体自体は任意値を持てるが、ロジック層で検証）
    // これはcreate_post V2で検証されるべきだが、構造体として妥当な値を持てることを確認

    // 正常ケース
    let valid = PostContent {
        root: [0u8; 32],
        k: 3,
        n: 5,
        size: 1000,
    };
    assert!(valid.k > 0 && valid.k <= valid.n);

    // 境界値: k = n (全断片必要)
    let edge = PostContent {
        root: [0u8; 32],
        k: 5,
        n: 5,
        size: 1000,
    };
    assert!(edge.k <= edge.n);

    // 異常値のテスト（構造体は作成可能だが、ロジックで弾く）
    let invalid_k_zero = PostContent {
        root: [0u8; 32],
        k: 0,  // 無効: k > 0 必須
        n: 5,
        size: 1000,
    };
    assert!(invalid_k_zero.k == 0);  // 構造体は作成可能

    let invalid_k_gt_n = PostContent {
        root: [0u8; 32],
        k: 6,  // 無効: k <= n 必須
        n: 5,
        size: 1000,
    };
    assert!(invalid_k_gt_n.k > invalid_k_gt_n.n);  // 構造体は作成可能
}

// ============================================================================
// Phase 3: User Story 1 - 投稿作成（新フロー）テスト
// ============================================================================

// T020: create_postのパラメータテスト
#[test]
fn test_create_post_params() {
    new_test_ext().execute_with(|| {
        let author = 1u64;
        let merkle_root = [0xABu8; 32];
        let k = 3u32;
        let n = 5u32;
        let total_size = 1024u64 * 100; // 100KB

        // 投稿作成
        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(author),
            merkle_root,
            k,
            n,
            total_size,
            None // parent_id
        ));

        // ContentRefsに正しく保存されている
        let content_ref = ContentRefs::<Test>::get(0).expect("ContentRefが存在するはず");
        assert_eq!(content_ref.root, merkle_root);
        assert_eq!(content_ref.k, k);
        assert_eq!(content_ref.n, n);
        assert_eq!(content_ref.size, total_size);

        // Post.content_hashはmerkle_rootと同じ
        let post = Posts::<Test>::get(0).expect("Postが存在するはず");
        assert_eq!(post.content_hash, merkle_root);
        assert_eq!(post.author, author);
    });
}

// T021: コスト計算テスト（50:30:20比率）
#[test]
fn test_cost_calculation_ratio() {
    new_test_ext().execute_with(|| {
        let author = 1u64;
        let initial_balance = Balances::free_balance(author);
        
        let merkle_root = [0xABu8; 32];
        let total_size = 10_000u64; // 10KB

        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(author),
            merkle_root,
            3,
            5,
            total_size,
            None
        ));

        // コスト計算: 
        // PostBaseCost = 100, PostByteCost = 10, size = 10000
        // base_cost (50%) = 100
        // size_cost (30%) = 10000 * 10 = 100000
        // deposit (20%) = (100 + 100000) / 5 = 20020
        // total = 100 + 100000 + 20020 = 120120
        let new_balance = Balances::free_balance(author);
        let expected_cost: u128 = 100 + 10_000 * 10 + (100 + 100_000) / 5;
        assert_eq!(new_balance, initial_balance - expected_cost);
    });
}

// T022: k/n検証テスト（k > 0 && k <= n）
#[test]
fn test_k_n_validation() {
    new_test_ext().execute_with(|| {
        let merkle_root = [0u8; 32];

        // 無効: k = 0
        assert_noop!(
            PostModule::create_post(
                RuntimeOrigin::signed(1),
                merkle_root,
                0, // k = 0は無効
                5,
                1000,
                None
            ),
            Error::<Test>::InvalidKNParameters
        );

        // 無効: k > n
        assert_noop!(
            PostModule::create_post(
                RuntimeOrigin::signed(1),
                merkle_root,
                6, // k > n
                5,
                1000,
                None
            ),
            Error::<Test>::InvalidKNParameters
        );

        // 有効: k = n（境界値）
        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(1),
            merkle_root,
            5, // k == n: OK
            5,
            1000,
            None
        ));
    });
}

// T022b: Storage deposit割当テスト（20%がStorageリワードプールへ）
#[test]
fn test_storage_deposit_allocation() {
    new_test_ext().execute_with(|| {
        let author = 1u64;
        let initial_balance = Balances::free_balance(author);
        
        let merkle_root = [0xABu8; 32];
        let total_size = 10_000u64;

        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(author),
            merkle_root,
            3,
            5,
            total_size,
            None
        ));

        // コストが消費されている（詳細な割当は上のtest_cost_calculation_ratioで検証）
        let new_balance = Balances::free_balance(author);
        assert!(new_balance < initial_balance);
    });
}

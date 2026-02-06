//! Post Palletのテスト

use crate::{self as pallet_post, Error, Event, Posts, NextPostId, UserPosts};
use frame_support::{
    assert_noop, assert_ok,
    parameter_types,
    traits::{ConstU32, ConstU64},
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
        PostModule: pallet_post,
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

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
    type AccountData = ();
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

impl pallet_post::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxContentLength = ConstU32<10000>;
}

// テスト環境のビルダー
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

#[test]
fn create_post_works() {
    new_test_ext().execute_with(|| {
        let author = 1u64;
        let content = b"Hello, Anarchy!".to_vec();

        // 投稿作成
        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(author),
            content.clone(),
            None
        ));

        // 投稿IDが1に増加
        assert_eq!(NextPostId::<Test>::get(), 1);

        // 投稿が保存されている
        let post = Posts::<Test>::get(0).expect("投稿が存在するはず");
        assert_eq!(post.author, author);
        assert_eq!(post.parent_id, None);

        // ユーザーの投稿一覧に追加されている
        let user_posts = UserPosts::<Test>::get(author);
        assert_eq!(user_posts.len(), 1);
        assert_eq!(user_posts[0], 0);

        // イベントが発行されている
        System::assert_has_event(RuntimeEvent::PostModule(Event::PostCreated {
            post_id: 0,
            author,
            content_hash: sp_io::hashing::blake2_256(&content),
        }));
    });
}

#[test]
fn create_reply_works() {
    new_test_ext().execute_with(|| {
        let author1 = 1u64;
        let author2 = 2u64;

        // 親投稿を作成
        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(author1),
            b"Original post".to_vec(),
            None
        ));

        // リプライを作成
        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(author2),
            b"This is a reply".to_vec(),
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
        // 存在しない投稿へのリプライは失敗
        assert_noop!(
            PostModule::create_post(
                RuntimeOrigin::signed(1),
                b"Reply to nothing".to_vec(),
                Some(999)
            ),
            Error::<Test>::ParentPostNotFound
        );
    });
}

#[test]
fn content_too_long_fails() {
    new_test_ext().execute_with(|| {
        // 10001バイトのコンテンツ（上限超過）
        let content = vec![0u8; 10001];

        assert_noop!(
            PostModule::create_post(
                RuntimeOrigin::signed(1),
                content,
                None
            ),
            Error::<Test>::ContentTooLong
        );
    });
}

#[test]
fn multiple_posts_by_same_user() {
    new_test_ext().execute_with(|| {
        let author = 1u64;

        // 3つの投稿を作成
        for i in 0..3 {
            assert_ok!(PostModule::create_post(
                RuntimeOrigin::signed(author),
                format!("Post number {}", i).into_bytes(),
                None
            ));
        }

        // 投稿IDが3に増加
        assert_eq!(NextPostId::<Test>::get(), 3);

        // ユーザーの投稿一覧に3つ追加されている
        let user_posts = UserPosts::<Test>::get(author);
        assert_eq!(user_posts.len(), 3);
        assert_eq!(user_posts.to_vec(), vec![0, 1, 2]);
    });
}

#[test]
fn content_hash_is_correct() {
    new_test_ext().execute_with(|| {
        let content = b"Test content for hashing".to_vec();
        let expected_hash = sp_io::hashing::blake2_256(&content);

        assert_ok!(PostModule::create_post(
            RuntimeOrigin::signed(1),
            content,
            None
        ));

        let post = Posts::<Test>::get(0).expect("投稿が存在するはず");
        assert_eq!(post.content_hash, expected_hash);
    });
}

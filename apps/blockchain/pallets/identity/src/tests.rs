//! Identity Pallet のテスト

use crate::{self as pallet_identity, derive_passkey_id, Error, Event, Identities, NextIdentityId, PasskeyOwner};
use frame_support::{
    assert_noop, assert_ok,
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
        IdentityModule: pallet_identity,
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

impl pallet_identity::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxPasskeys = ConstU32<10>;
    type MaxPublicKeyLength = ConstU32<256>;
    type MaxDeviceNameLength = ConstU32<64>;
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

// ヘルパー: テスト用公開鍵を生成
fn test_public_key(seed: u8) -> Vec<u8> {
    vec![seed; 32]
}

// ヘルパー: デバイス名を生成
fn test_device_name(name: &str) -> Option<Vec<u8>> {
    Some(name.as_bytes().to_vec())
}

// ============================================================================
// User Story 1: 新規ユーザーがIdentityを作成する
// ============================================================================

#[test]
fn register_identity_works() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let public_key = test_public_key(1);
        let device_name = test_device_name("MacBook Pro");
        let expected_passkey_id = derive_passkey_id(&public_key);

        // Identity作成
        assert_ok!(IdentityModule::register_identity(
            RuntimeOrigin::signed(alice),
            public_key.clone(),
            device_name.clone(),
        ));

        // Identity ID = 0 が発行されていることを確認
        assert_eq!(NextIdentityId::<Test>::get(), 1);

        // Identityが正しく保存されていることを確認
        let identity = Identities::<Test>::get(0).expect("Identity should exist");
        assert_eq!(identity.passkeys.len(), 1);
        assert_eq!(identity.passkeys[0].public_key.as_slice(), public_key.as_slice());
        assert_eq!(identity.passkeys[0].id, expected_passkey_id);

        // PasskeyOwnerが設定されていることを確認
        assert_eq!(PasskeyOwner::<Test>::get(expected_passkey_id), Some(0));

        // イベント確認
        System::assert_has_event(RuntimeEvent::IdentityModule(Event::IdentityCreated {
            identity_id: 0,
            passkey_id: expected_passkey_id,
        }));
    });
}

#[test]
fn register_identity_empty_pubkey_fails() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let empty_key: Vec<u8> = vec![];

        // 空の公開鍵は失敗
        assert_noop!(
            IdentityModule::register_identity(RuntimeOrigin::signed(alice), empty_key, None),
            Error::<Test>::EmptyPublicKey
        );
    });
}

#[test]
fn register_identity_pubkey_too_long_fails() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let long_key: Vec<u8> = vec![0u8; 257]; // 256バイト超過

        // 長すぎる公開鍵は失敗
        assert_noop!(
            IdentityModule::register_identity(RuntimeOrigin::signed(alice), long_key, None),
            Error::<Test>::PublicKeyTooLong
        );
    });
}

#[test]
fn register_identity_duplicate_passkey_fails() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let bob = 2u64;
        let public_key = test_public_key(1);

        // Aliceが先に登録
        assert_ok!(IdentityModule::register_identity(
            RuntimeOrigin::signed(alice),
            public_key.clone(),
            None,
        ));

        // Bobが同じ公開鍵で登録しようとすると失敗
        assert_noop!(
            IdentityModule::register_identity(RuntimeOrigin::signed(bob), public_key, None),
            Error::<Test>::PasskeyAlreadyRegistered
        );
    });
}

// ============================================================================
// User Story 2: 既存ユーザーが新しいデバイスを追加する
// ============================================================================

#[test]
fn add_passkey_works() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let public_key1 = test_public_key(1);
        let public_key2 = test_public_key(2);
        let expected_passkey_id2 = derive_passkey_id(&public_key2);

        // 先にIdentity作成
        assert_ok!(IdentityModule::register_identity(
            RuntimeOrigin::signed(alice),
            public_key1,
            test_device_name("MacBook Pro"),
        ));

        // 2台目のデバイスを追加
        assert_ok!(IdentityModule::add_passkey(
            RuntimeOrigin::signed(alice),
            0, // identity_id
            public_key2.clone(),
            test_device_name("iPhone"),
        ));

        // 2つのPasskeyが登録されていることを確認
        let identity = Identities::<Test>::get(0).expect("Identity should exist");
        assert_eq!(identity.passkeys.len(), 2);

        // イベント確認
        System::assert_has_event(RuntimeEvent::IdentityModule(Event::PasskeyAdded {
            identity_id: 0,
            passkey_id: expected_passkey_id2,
        }));
    });
}

#[test]
fn add_passkey_identity_not_found() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let public_key = test_public_key(1);

        // 存在しないIdentityにPasskeyを追加しようとすると失敗
        assert_noop!(
            IdentityModule::add_passkey(
                RuntimeOrigin::signed(alice),
                999, // 存在しないidentity_id
                public_key,
                None,
            ),
            Error::<Test>::IdentityNotFound
        );
    });
}

#[test]
fn add_passkey_duplicate_fails() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let bob = 2u64;
        let public_key1 = test_public_key(1);
        let public_key2 = test_public_key(2);

        // AliceがIdentity作成
        assert_ok!(IdentityModule::register_identity(
            RuntimeOrigin::signed(alice),
            public_key1.clone(),
            None,
        ));

        // BobがIdentity作成（別の公開鍵）
        assert_ok!(IdentityModule::register_identity(
            RuntimeOrigin::signed(bob),
            public_key2,
            None,
        ));

        // AliceのIdentityにBobの公開鍵（既に登録済み）を追加しようとすると失敗
        // ここでは public_key1 が既に Alice の Identity 0 に登録されている
        // 別の identity_id=1 に public_key1 を追加しようとする
        assert_noop!(
            IdentityModule::add_passkey(
                RuntimeOrigin::signed(bob),
                1, // BobのIdentity
                public_key1, // Aliceが既に登録した公開鍵
                None,
            ),
            Error::<Test>::PasskeyAlreadyRegistered
        );
    });
}

#[test]
fn add_passkey_max_limit() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;

        // Identity作成（1つ目のPasskey）
        assert_ok!(IdentityModule::register_identity(
            RuntimeOrigin::signed(alice),
            test_public_key(0),
            None,
        ));

        // 9個追加（合計10個）
        for i in 1..10u8 {
            assert_ok!(IdentityModule::add_passkey(
                RuntimeOrigin::signed(alice),
                0,
                test_public_key(i),
                None,
            ));
        }

        // Identityに10個のPasskeyがあることを確認
        let identity = Identities::<Test>::get(0).expect("Identity should exist");
        assert_eq!(identity.passkeys.len(), 10);

        // 11個目を追加しようとすると失敗
        assert_noop!(
            IdentityModule::add_passkey(
                RuntimeOrigin::signed(alice),
                0,
                test_public_key(10),
                None,
            ),
            Error::<Test>::TooManyPasskeys
        );
    });
}

// ============================================================================
// User Story 3: ユーザーがデバイスを削除する
// ============================================================================

#[test]
fn remove_passkey_works() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let public_key1 = test_public_key(1);
        let public_key2 = test_public_key(2);
        let passkey_id1 = derive_passkey_id(&public_key1);

        // Identity作成
        assert_ok!(IdentityModule::register_identity(
            RuntimeOrigin::signed(alice),
            public_key1,
            None,
        ));

        // 2台目追加
        assert_ok!(IdentityModule::add_passkey(
            RuntimeOrigin::signed(alice),
            0,
            public_key2,
            None,
        ));

        // 1台目を削除
        assert_ok!(IdentityModule::remove_passkey(
            RuntimeOrigin::signed(alice),
            0,
            passkey_id1,
        ));

        // 1つのPasskeyが残っていることを確認
        let identity = Identities::<Test>::get(0).expect("Identity should exist");
        assert_eq!(identity.passkeys.len(), 1);

        // PasskeyOwnerから削除されていることを確認
        assert_eq!(PasskeyOwner::<Test>::get(passkey_id1), None);

        // イベント確認
        System::assert_has_event(RuntimeEvent::IdentityModule(Event::PasskeyRemoved {
            identity_id: 0,
            passkey_id: passkey_id1,
        }));
    });
}

#[test]
fn remove_passkey_not_found() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let public_key = test_public_key(1);
        let fake_passkey_id = [99u8; 32];

        // Identity作成
        assert_ok!(IdentityModule::register_identity(
            RuntimeOrigin::signed(alice),
            public_key,
            None,
        ));

        // 2台目追加して2台ある状態にする
        assert_ok!(IdentityModule::add_passkey(
            RuntimeOrigin::signed(alice),
            0,
            test_public_key(2),
            None,
        ));

        // 存在しないPasskeyを削除しようとすると失敗
        assert_noop!(
            IdentityModule::remove_passkey(RuntimeOrigin::signed(alice), 0, fake_passkey_id),
            Error::<Test>::PasskeyNotFound
        );
    });
}

#[test]
fn remove_last_passkey_fails() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let public_key = test_public_key(1);
        let passkey_id = derive_passkey_id(&public_key);

        // Identity作成（1つのPasskeyのみ）
        assert_ok!(IdentityModule::register_identity(
            RuntimeOrigin::signed(alice),
            public_key,
            None,
        ));

        // 最後のPasskeyを削除しようとすると失敗
        assert_noop!(
            IdentityModule::remove_passkey(RuntimeOrigin::signed(alice), 0, passkey_id),
            Error::<Test>::CannotRemoveLastPasskey
        );
    });
}

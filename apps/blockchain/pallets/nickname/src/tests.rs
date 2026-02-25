//! Nickname Pallet Tests
//!
//! TDD: Tests written first based on spec.md requirements
//! T-006: Nickname Pallet tests

use crate::{self as pallet_nickname, Error, Event, Nicknames};
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

// Test runtime construction
frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Nickname: pallet_nickname,
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

// MaxNicknameLength = 128 bytes
impl pallet_nickname::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxNicknameLength = ConstU32<128>;
}

/// Build test externalities
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}

// ============================================================================
// T-006a: set_nickname - Basic functionality
// ============================================================================

#[test]
fn set_nickname_works() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        let nickname = b"alice_anarchy".to_vec();
        
        // Should succeed
        assert_ok!(Nickname::set_nickname(
            RuntimeOrigin::signed(account),
            nickname.clone()
        ));
        
        // Verify storage
        let stored = Nicknames::<Test>::get(account);
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().to_vec(), nickname);
        
        // Verify event
        System::assert_last_event(
            Event::NicknameSet {
                who: account,
                nickname: nickname.clone(),
            }
            .into(),
        );
    });
}

#[test]
fn set_nickname_update_works() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        let nickname1 = b"alice".to_vec();
        let nickname2 = b"alice_updated".to_vec();
        
        // Set initial nickname
        assert_ok!(Nickname::set_nickname(
            RuntimeOrigin::signed(account),
            nickname1.clone()
        ));
        
        // Update nickname
        assert_ok!(Nickname::set_nickname(
            RuntimeOrigin::signed(account),
            nickname2.clone()
        ));
        
        // Verify updated value
        let stored = Nicknames::<Test>::get(account);
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().to_vec(), nickname2);
    });
}

#[test]
fn set_nickname_max_length_works() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        // Exactly 128 bytes
        let nickname = vec![b'a'; 128];
        
        assert_ok!(Nickname::set_nickname(
            RuntimeOrigin::signed(account),
            nickname.clone()
        ));
        
        let stored = Nicknames::<Test>::get(account);
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().len(), 128);
    });
}

// ============================================================================
// T-006b: set_nickname - Error cases
// ============================================================================

#[test]
fn set_nickname_too_long_fails() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        // 129 bytes - exceeds limit
        let nickname = vec![b'a'; 129];
        
        assert_noop!(
            Nickname::set_nickname(RuntimeOrigin::signed(account), nickname),
            Error::<Test>::NicknameTooLong
        );
    });
}

#[test]
fn set_nickname_empty_fails() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        let nickname: Vec<u8> = vec![];
        
        assert_noop!(
            Nickname::set_nickname(RuntimeOrigin::signed(account), nickname),
            Error::<Test>::NicknameEmpty
        );
    });
}

#[test]
fn set_nickname_invalid_utf8_fails() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        // Invalid UTF-8 sequence
        let nickname = vec![0xFF, 0xFE, 0x00, 0x01];
        
        assert_noop!(
            Nickname::set_nickname(RuntimeOrigin::signed(account), nickname),
            Error::<Test>::InvalidUtf8
        );
    });
}

#[test]
fn set_nickname_requires_signed_origin() {
    new_test_ext().execute_with(|| {
        let nickname = b"test".to_vec();
        
        // Root should fail
        assert_noop!(
            Nickname::set_nickname(RuntimeOrigin::root(), nickname.clone()),
            sp_runtime::DispatchError::BadOrigin
        );
        
        // None should fail
        assert_noop!(
            Nickname::set_nickname(RuntimeOrigin::none(), nickname),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ============================================================================
// T-006c: clear_nickname - Basic functionality
// ============================================================================

#[test]
fn clear_nickname_works() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        let nickname = b"alice".to_vec();
        
        // Set nickname first
        assert_ok!(Nickname::set_nickname(
            RuntimeOrigin::signed(account),
            nickname
        ));
        
        // Clear nickname
        assert_ok!(Nickname::clear_nickname(RuntimeOrigin::signed(account)));
        
        // Verify storage is empty
        let stored = Nicknames::<Test>::get(account);
        assert!(stored.is_none());
        
        // Verify event
        System::assert_last_event(
            Event::NicknameCleared { who: account }.into(),
        );
    });
}

#[test]
fn clear_nickname_no_existing_succeeds() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        
        // Clear without setting first - should still succeed (idempotent)
        assert_ok!(Nickname::clear_nickname(RuntimeOrigin::signed(account)));
        
        // Verify storage is empty
        let stored = Nicknames::<Test>::get(account);
        assert!(stored.is_none());
    });
}

#[test]
fn clear_nickname_requires_signed_origin() {
    new_test_ext().execute_with(|| {
        // Root should fail
        assert_noop!(
            Nickname::clear_nickname(RuntimeOrigin::root()),
            sp_runtime::DispatchError::BadOrigin
        );
        
        // None should fail
        assert_noop!(
            Nickname::clear_nickname(RuntimeOrigin::none()),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ============================================================================
// T-006d: UTF-8 validation
// ============================================================================

#[test]
fn set_nickname_utf8_japanese_works() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        // Japanese characters (valid UTF-8)
        let nickname = "アリス".as_bytes().to_vec();
        
        assert_ok!(Nickname::set_nickname(
            RuntimeOrigin::signed(account),
            nickname.clone()
        ));
        
        let stored = Nicknames::<Test>::get(account);
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().to_vec(), nickname);
    });
}

#[test]
fn set_nickname_utf8_emoji_works() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        // Emoji (valid UTF-8)
        let nickname = "🦀rust🔥".as_bytes().to_vec();
        
        assert_ok!(Nickname::set_nickname(
            RuntimeOrigin::signed(account),
            nickname.clone()
        ));
        
        let stored = Nicknames::<Test>::get(account);
        assert!(stored.is_some());
    });
}

#[test]
fn set_nickname_utf8_mixed_works() {
    new_test_ext().execute_with(|| {
        let account: u64 = 1;
        // Mixed ASCII, Japanese, and emoji
        let nickname = "alice_アリス_🎉".as_bytes().to_vec();
        
        assert_ok!(Nickname::set_nickname(
            RuntimeOrigin::signed(account),
            nickname.clone()
        ));
        
        let stored = Nicknames::<Test>::get(account);
        assert!(stored.is_some());
    });
}

// ============================================================================
// T-006e: Multiple accounts
// ============================================================================

#[test]
fn multiple_accounts_independent_nicknames() {
    new_test_ext().execute_with(|| {
        let account1: u64 = 1;
        let account2: u64 = 2;
        let nickname1 = b"alice".to_vec();
        let nickname2 = b"bob".to_vec();
        
        // Set nicknames for both accounts
        assert_ok!(Nickname::set_nickname(
            RuntimeOrigin::signed(account1),
            nickname1.clone()
        ));
        assert_ok!(Nickname::set_nickname(
            RuntimeOrigin::signed(account2),
            nickname2.clone()
        ));
        
        // Verify independent storage
        assert_eq!(Nicknames::<Test>::get(account1).unwrap().to_vec(), nickname1);
        assert_eq!(Nicknames::<Test>::get(account2).unwrap().to_vec(), nickname2);
        
        // Clear one doesn't affect the other
        assert_ok!(Nickname::clear_nickname(RuntimeOrigin::signed(account1)));
        assert!(Nicknames::<Test>::get(account1).is_none());
        assert!(Nicknames::<Test>::get(account2).is_some());
    });
}

//! Moral Token Palletのテスト

use crate::{self as pallet_moral, Balances, Error, Event, TotalSupply};
use frame_support::{
    assert_noop, assert_ok,
    parameter_types,
    traits::{ConstU128, ConstU32, ConstU64},
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
        MoralModule: pallet_moral,
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

parameter_types! {
    pub const InitialBalance: u128 = 100_000;
}

impl pallet_moral::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type InitialBalance = InitialBalance;
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

// ヘルパー: アカウントにトークンを付与
fn mint_to(account: u64, amount: u128) {
    Balances::<Test>::insert(account, amount);
    TotalSupply::<Test>::put(TotalSupply::<Test>::get() + amount);
}

#[test]
fn transfer_works() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let bob = 2u64;
        let amount = 500u128;

        // Aliceに1000トークン付与
        mint_to(alice, 1000);

        // AliceからBobに500トークン送金
        assert_ok!(MoralModule::transfer(
            RuntimeOrigin::signed(alice),
            bob,
            amount
        ));

        // 残高確認
        assert_eq!(Balances::<Test>::get(alice), 500);
        assert_eq!(Balances::<Test>::get(bob), 500);

        // イベント確認
        System::assert_has_event(RuntimeEvent::MoralModule(Event::Transferred {
            from: alice,
            to: bob,
            amount,
        }));
    });
}

#[test]
fn transfer_insufficient_balance_fails() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let bob = 2u64;

        // Aliceに100トークン付与
        mint_to(alice, 100);

        // 残高以上の送金は失敗
        assert_noop!(
            MoralModule::transfer(RuntimeOrigin::signed(alice), bob, 200),
            Error::<Test>::InsufficientBalance
        );
    });
}

#[test]
fn self_transfer_fails() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        mint_to(alice, 1000);

        // 自分自身への送金は失敗
        assert_noop!(
            MoralModule::transfer(RuntimeOrigin::signed(alice), alice, 100),
            Error::<Test>::SelfTransfer
        );
    });
}

#[test]
fn burn_works() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let initial = 1000u128;
        let burn_amount = 300u128;

        mint_to(alice, initial);
        let initial_supply = TotalSupply::<Test>::get();

        // 300トークン焼却
        assert_ok!(MoralModule::burn(RuntimeOrigin::signed(alice), burn_amount));

        // 残高確認
        assert_eq!(Balances::<Test>::get(alice), initial - burn_amount);

        // 総供給量が減少
        assert_eq!(TotalSupply::<Test>::get(), initial_supply - burn_amount);

        // イベント確認
        System::assert_has_event(RuntimeEvent::MoralModule(Event::Burned {
            who: alice,
            amount: burn_amount,
        }));
    });
}

#[test]
fn burn_insufficient_balance_fails() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        mint_to(alice, 100);

        // 残高以上の焼却は失敗
        assert_noop!(
            MoralModule::burn(RuntimeOrigin::signed(alice), 200),
            Error::<Test>::InsufficientBalance
        );
    });
}

#[test]
fn mint_works_for_root() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let amount = 5000u128;

        // Root権限でミント
        assert_ok!(MoralModule::mint(RuntimeOrigin::root(), alice, amount));

        // 残高確認
        assert_eq!(Balances::<Test>::get(alice), amount);

        // 総供給量が増加
        assert_eq!(TotalSupply::<Test>::get(), amount);

        // イベント確認
        System::assert_has_event(RuntimeEvent::MoralModule(Event::Minted {
            who: alice,
            amount,
        }));
    });
}

#[test]
fn mint_fails_for_non_root() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;

        // 一般ユーザーからのミントは失敗
        assert_noop!(
            MoralModule::mint(RuntimeOrigin::signed(alice), alice, 1000),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn claim_initial_works() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;

        // 初期トークンを請求
        assert_ok!(MoralModule::claim_initial(RuntimeOrigin::signed(alice)));

        // InitialBalance分が付与される
        assert_eq!(Balances::<Test>::get(alice), InitialBalance::get());
    });
}

#[test]
fn claim_initial_fails_if_already_has_balance() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;

        // 既に残高がある
        mint_to(alice, 100);

        // 再度の請求は失敗
        assert_noop!(
            MoralModule::claim_initial(RuntimeOrigin::signed(alice)),
            Error::<Test>::InsufficientBalance // エラー名は要検討
        );
    });
}

#[test]
fn total_supply_tracking() {
    new_test_ext().execute_with(|| {
        let alice = 1u64;
        let bob = 2u64;

        // 初期状態
        assert_eq!(TotalSupply::<Test>::get(), 0);

        // ミント: +1000
        assert_ok!(MoralModule::mint(RuntimeOrigin::root(), alice, 1000));
        assert_eq!(TotalSupply::<Test>::get(), 1000);

        // ミント: +500
        assert_ok!(MoralModule::mint(RuntimeOrigin::root(), bob, 500));
        assert_eq!(TotalSupply::<Test>::get(), 1500);

        // バーン: -200
        assert_ok!(MoralModule::burn(RuntimeOrigin::signed(alice), 200));
        assert_eq!(TotalSupply::<Test>::get(), 1300);

        // 転送は総供給量に影響しない
        assert_ok!(MoralModule::transfer(RuntimeOrigin::signed(alice), bob, 100));
        assert_eq!(TotalSupply::<Test>::get(), 1300);
    });
}

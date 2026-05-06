//! Tests for pallet-storage-stake (TSTS P4).

use crate::{self as pallet_storage_stake, BondInfo};
use frame_support::{
    assert_noop, assert_ok, construct_runtime, parameter_types,
    traits::{ConstU32, ConstU64, ConstU128},
};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, Permill,
};

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        StorageStake: pallet_storage_stake,
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
    type RuntimeTask = RuntimeTask;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
    type ExtensionsWeightInfo = ();
}

impl pallet_balances::Config for Test {
    type Balance = Balance;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<0>;
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}

parameter_types! {
    pub const BondPerGB: Balance = 10_000_000_000_000;     // 10 MORAL/GB
    pub const MinDeclaredCapacity: u64 = 1_073_741_824;     // 1 GB
    pub const BondReleaseDelay: u64 = 100_800;              // 7 days @ 30s
    pub SlashBurnShare: Permill = Permill::from_percent(30);
}

impl pallet_storage_stake::Config for Test {
    type Currency = Balances;
    type BondPerGB = BondPerGB;
    type MinDeclaredCapacity = MinDeclaredCapacity;
    type BondReleaseDelay = BondReleaseDelay;
    type SlashBurnSharePermill = SlashBurnShare;
}

fn new_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 1_000_000_000_000_000), (2, 1_000_000_000_000_000)],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();
    t.into()
}

#[test]
fn bond_locks_balance_proportional_to_capacity() {
    new_ext().execute_with(|| {
        // 2 GB → 20 MORAL bond
        assert_ok!(StorageStake::bond(RuntimeOrigin::signed(1), 2 * 1_073_741_824));
        let bond = StorageStake::bonds(1).expect("bonded");
        assert_eq!(bond.amount, 20_000_000_000_000);
        assert_eq!(bond.declared_capacity_bytes, 2 * 1_073_741_824);
        assert_eq!(StorageStake::total_active_bond(), 20_000_000_000_000);
        // free balance reduced by reserve
        assert_eq!(Balances::reserved_balance(1), 20_000_000_000_000);
    });
}

#[test]
fn bond_capacity_below_minimum_fails() {
    new_ext().execute_with(|| {
        assert_noop!(
            StorageStake::bond(RuntimeOrigin::signed(1), 1024),
            pallet_storage_stake::Error::<Test>::CapacityTooLow
        );
    });
}

#[test]
fn double_bond_fails() {
    new_ext().execute_with(|| {
        assert_ok!(StorageStake::bond(RuntimeOrigin::signed(1), 1_073_741_824));
        assert_noop!(
            StorageStake::bond(RuntimeOrigin::signed(1), 1_073_741_824),
            pallet_storage_stake::Error::<Test>::AlreadyBonded
        );
    });
}

#[test]
fn finalize_release_before_delay_fails() {
    new_ext().execute_with(|| {
        assert_ok!(StorageStake::bond(RuntimeOrigin::signed(1), 1_073_741_824));
        assert_ok!(StorageStake::request_release(RuntimeOrigin::signed(1)));
        // No block advance → still pending
        assert_noop!(
            StorageStake::finalize_release(RuntimeOrigin::signed(1)),
            pallet_storage_stake::Error::<Test>::ReleaseStillPending
        );
    });
}

#[test]
fn finalize_release_after_delay_returns_bond() {
    new_ext().execute_with(|| {
        assert_ok!(StorageStake::bond(RuntimeOrigin::signed(1), 1_073_741_824));
        assert_ok!(StorageStake::request_release(RuntimeOrigin::signed(1)));
        System::set_block_number(100_801);
        assert_ok!(StorageStake::finalize_release(RuntimeOrigin::signed(1)));
        assert_eq!(Balances::reserved_balance(1), 0);
        assert!(StorageStake::bonds(1).is_none());
        assert_eq!(StorageStake::total_active_bond(), 0);
    });
}

#[test]
fn slash_bond_reduces_amount_and_burns() {
    new_ext().execute_with(|| {
        assert_ok!(StorageStake::bond(RuntimeOrigin::signed(1), 1_073_741_824));
        let initial_total = pallet_balances::Pallet::<Test>::total_issuance();

        // 1 MORAL slash
        let actual = <StorageStake as BondInfo<u64>>::slash_bond(&1, 1_000_000_000_000);
        assert_eq!(actual, 1_000_000_000_000);

        let bond = StorageStake::bonds(1).expect("still bonded after partial slash");
        assert_eq!(bond.amount, 9_000_000_000_000); // 10 - 1 = 9 MORAL

        // total issuance dropped by 1 MORAL (burn via slash_reserved)
        let final_total = pallet_balances::Pallet::<Test>::total_issuance();
        assert_eq!(initial_total - final_total, 1_000_000_000_000);
    });
}

#[test]
fn slash_bond_saturates_at_bond_amount() {
    new_ext().execute_with(|| {
        assert_ok!(StorageStake::bond(RuntimeOrigin::signed(1), 1_073_741_824));
        // Try to slash 100 MORAL but bond is only 10 MORAL
        let actual = <StorageStake as BondInfo<u64>>::slash_bond(&1, 100_000_000_000_000);
        assert_eq!(actual, 10_000_000_000_000); // saturated

        // Bond removed entirely
        assert!(StorageStake::bonds(1).is_none());
        assert_eq!(StorageStake::total_active_bond(), 0);
    });
}

#[test]
fn bond_info_trait_returns_correct_values() {
    new_ext().execute_with(|| {
        assert!(!<StorageStake as BondInfo<u64>>::has_bond(&1));
        assert_eq!(<StorageStake as BondInfo<u64>>::bond_amount(&1), 0);

        assert_ok!(StorageStake::bond(RuntimeOrigin::signed(1), 1_073_741_824));

        assert!(<StorageStake as BondInfo<u64>>::has_bond(&1));
        assert_eq!(<StorageStake as BondInfo<u64>>::bond_amount(&1), 10_000_000_000_000);
        assert_eq!(<StorageStake as BondInfo<u64>>::total_active_bond(), 10_000_000_000_000);
    });
}

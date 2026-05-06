//! Tests for pallet-base-fee.

use crate::{self as pallet_base_fee, BaseFeeProvider};
use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstU32, ConstU64, ConstU128, Hooks},
};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        BaseFee: pallet_base_fee,
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
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<0>;
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}

parameter_types! {
    pub const GasTargetBytesPerBlock: u32 = 50_000;
    pub const BaseFeeMin: u128 = 100;                     // 1e-10 MORAL/byte
    pub const BaseFeeMax: u128 = 100_000_000_000;          // 0.1 MORAL/byte
    pub const BaseFeeInit: u128 = 10_000;                  // 1e-8 MORAL/byte
}

impl pallet_base_fee::Config for Test {
    type GasTargetBytesPerBlock = GasTargetBytesPerBlock;
    type BaseFeeMin = BaseFeeMin;
    type BaseFeeMax = BaseFeeMax;
    type BaseFeeInit = BaseFeeInit;
}

fn new_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    pallet_base_fee::GenesisConfig::<Test> {
        _phantom: Default::default(),
    }
    .assimilate_storage(&mut t)
    .unwrap();
    t.into()
}

#[test]
fn base_fee_at_target_utilization_stays_constant() {
    new_ext().execute_with(|| {
        // utilization = 1.0 (target に等しい) → bump = 0
        <BaseFee as BaseFeeProvider>::record_gas(50_000);
        BaseFee::on_finalize(1);
        // 完全に target ぴったりだと加算 0 → BaseFeeInit と同じ
        assert_eq!(<BaseFee as BaseFeeProvider>::current_base_fee(), 10_000);
    });
}

#[test]
fn base_fee_increases_above_target() {
    new_ext().execute_with(|| {
        // 100% over (= 2x target) → bump = cur × over / (target × 8) = cur × 50000 / 400000 = cur/8
        // つまり 12.5% 増加
        <BaseFee as BaseFeeProvider>::record_gas(100_000);
        BaseFee::on_finalize(1);
        assert_eq!(<BaseFee as BaseFeeProvider>::current_base_fee(), 11_250); // 10000 × 1.125
    });
}

#[test]
fn base_fee_decreases_below_target() {
    new_ext().execute_with(|| {
        // 0 used → cut = cur × target / (target × 8) = cur/8 = 12.5% 減
        <BaseFee as BaseFeeProvider>::record_gas(0);
        BaseFee::on_finalize(1);
        assert_eq!(<BaseFee as BaseFeeProvider>::current_base_fee(), 8_750); // 10000 × 0.875
    });
}

#[test]
fn base_fee_clamped_at_min() {
    new_ext().execute_with(|| {
        // 何度も 0 used で base_fee を下げ続けると BaseFeeMin (100) で止まる
        for _ in 0..200 {
            <BaseFee as BaseFeeProvider>::record_gas(0);
            BaseFee::on_finalize(1);
        }
        assert_eq!(<BaseFee as BaseFeeProvider>::current_base_fee(), 100);
    });
}

#[test]
fn base_fee_clamped_at_max() {
    new_ext().execute_with(|| {
        // 何度も 200% 利用 → 12.5% 増を繰り返して BaseFeeMax (1e11) で止まる
        for _ in 0..400 {
            <BaseFee as BaseFeeProvider>::record_gas(100_000);
            BaseFee::on_finalize(1);
        }
        assert_eq!(<BaseFee as BaseFeeProvider>::current_base_fee(), 100_000_000_000);
    });
}

#[test]
fn record_gas_accumulates_in_block() {
    new_ext().execute_with(|| {
        <BaseFee as BaseFeeProvider>::record_gas(10_000);
        <BaseFee as BaseFeeProvider>::record_gas(15_000);
        assert_eq!(BaseFee::gas_used_this_block(), 25_000);
        // on_finalize でリセット
        BaseFee::on_finalize(1);
        assert_eq!(BaseFee::gas_used_this_block(), 0);
    });
}

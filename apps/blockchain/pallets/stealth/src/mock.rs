//! Mock runtime for pallet-stealth tests

use crate as pallet_stealth;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::ConstU32,
};
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

// Configure mock runtime
frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        StealthPallet: pallet_stealth,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

parameter_types! {
    pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<0>;
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}

impl pallet_stealth::Config for Test {
    type Currency = Balances;
    type MaxEntriesPerBlock = ConstU32<1000>;
    type WeightInfo = ();
    /// TSTS F2: tests は cap = 10% (mainnet と同等) で claim ロジックを検証
    type ClaimCapPpm = ConstU32<100_000>;
}

/// Test accounts
pub const ALICE: u64 = 1;
pub const BOB: u64 = 2;
pub const STEALTH_ADDR: u64 = 100;

/// Build genesis storage for tests
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, 1_000_000_000_000_000), // 1000 MORAL
            (BOB, 500_000_000_000_000),     // 500 MORAL
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

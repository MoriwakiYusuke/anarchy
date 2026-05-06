//! Difficulty pallet tests.

use crate as pallet_difficulty;
use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstU32, ConstU64},
};
use sp_core::U256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Difficulty: pallet_difficulty,
    }
);

impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Hashing = BlakeTwo256;
    type BaseCallFilter = frame_support::traits::Everything;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeTask = RuntimeTask;
    type Nonce = u64;
    type Hash = sp_core::H256;
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
    type DbWeight = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
    type ExtensionsWeightInfo = ();
    type BlockWeights = ();
    type BlockLength = ();
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<1>;
    type WeightInfo = ();
}

parameter_types! {
    pub const TargetBlockTime: u64 = 30_000;
    pub const DifficultyAdjustWindow: u32 = 60;
    pub const MinDifficulty: U256 = U256([10_000, 0, 0, 0]);
}

impl pallet_difficulty::Config for Test {
    type TargetBlockTime = TargetBlockTime;
    type DifficultyAdjustWindow = DifficultyAdjustWindow;
    type MinDifficulty = MinDifficulty;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    pallet_difficulty::GenesisConfig::<Test> {
        initial_difficulty: U256::from(100_000u64),
        _phantom: Default::default(),
    }
    .assimilate_storage(&mut t)
    .unwrap();
    t.into()
}

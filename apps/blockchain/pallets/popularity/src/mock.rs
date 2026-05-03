//! Mock runtime for pallet-popularity unit tests.

use crate as pallet_popularity;
use frame_support::traits::{ConstU32, ConstU64};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, Permill,
};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Popularity: pallet_popularity,
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
    type AccountData = ();
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

frame_support::parameter_types! {
    // Decay 0.999 per block — fast for tests
    pub DecayRate: Permill = Permill::from_parts(999_000);
}

impl pallet_popularity::Config for Test {
    type InitialScore = ConstU64<10_000>;
    type LikeWeight = ConstU64<100>;
    type DislikeWeight = ConstU64<50>;
    type DecayRatePermill = DecayRate;
    type LowPopularityThreshold = ConstU64<1_000>;
    type HysteresisMargin = ConstU64<500>;
    type GracePeriod = ConstU64<10>;
    type MaxPostsScannedPerBlock = ConstU32<4>;
    type MaxDeletionsPerBlock = ConstU32<2>;
    type MaxDecaySteps = ConstU32<100_000>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::set_block_number(System::block_number() + 1);
    }
}

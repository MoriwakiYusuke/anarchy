//! Mock runtime for pallet-storage tests

use crate as pallet_storage;
use frame_support::traits::{ConstU128, ConstU32, ConstU64, ConstU8};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Storage: pallet_storage,
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

impl pallet_storage::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxFragmentSize = ConstU32<1_048_576>; // 1MB
    type MaxPeerIdLen = ConstU32<64>;
    type MaxHoldersPerFragment = ConstU32<100>;
    type MaxFragmentsPerNode = ConstU32<10_000>;
    // New security constants (relaxed for basic tests)
    type MinPeerIdLen = ConstU32<2>;                // Relaxed for basic tests
    type MaxRegistrationsPerBlock = ConstU32<5>;
    type MaxDeclarationsPerBlockPerNode = ConstU32<10>;
    type MinNodeCapacity = ConstU64<1>;              // Relaxed for basic tests
    type PowObservationPeriod = ConstU32<10>;
    type BasePowDifficulty = ConstU8<0>;             // No PoW for basic tests
    type MaxHttpUrlLen = ConstU32<256>;
    type BaseRewardPerByte = ConstU128<1>;           // 1 unit per byte for tests
    type ScoreThreshold = ConstU64<100>;             // Score threshold for tests
}

/// Build test externalities
#[allow(dead_code)]
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

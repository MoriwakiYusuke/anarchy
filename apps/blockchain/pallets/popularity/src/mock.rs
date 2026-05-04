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
    type MaxDeletionScanReads = ConstU32<8>;
    type MaxDecaySteps = ConstU32<100_000>;
    type PostCountProvider = MockPostCount;
    type PostMutator = MockPostMutator;
    type StorageReleaser = MockStorageReleaser;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::set_block_number(System::block_number() + 1);
    }
}

use std::cell::RefCell;
thread_local! {
    static MAX_POST_ID: RefCell<u64> = RefCell::new(0);
}

pub fn set_max_post_id(n: u64) {
    MAX_POST_ID.with(|c| *c.borrow_mut() = n);
}

#[allow(dead_code)]
pub fn reset_max_post_id() {
    MAX_POST_ID.with(|c| *c.borrow_mut() = 0);
}

pub struct MockPostCount;
impl crate::PostCountProvider for MockPostCount {
    fn next_post_id() -> u64 {
        MAX_POST_ID.with(|c| *c.borrow())
    }
}

thread_local! {
    static DELETED: RefCell<Vec<u64>> = RefCell::new(Vec::new());
    static RELEASED: RefCell<Vec<[u8; 32]>> = RefCell::new(Vec::new());
    /// post_ids for which the mock should return Err (simulating race "post already gone").
    static FAIL_DELETE: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
}

pub fn deleted_posts() -> Vec<u64> {
    DELETED.with(|c| c.borrow().clone())
}

pub fn released_hashes() -> Vec<[u8; 32]> {
    RELEASED.with(|c| c.borrow().clone())
}

pub fn reset_deletion_trackers() {
    DELETED.with(|c| c.borrow_mut().clear());
    RELEASED.with(|c| c.borrow_mut().clear());
    FAIL_DELETE.with(|c| c.borrow_mut().clear());
}

/// Mark a post id so the next `MockPostMutator::delete_post(post_id)` returns Err.
pub fn fail_delete_for(post_id: u64) {
    FAIL_DELETE.with(|c| {
        c.borrow_mut().insert(post_id);
    });
}

pub struct MockPostMutator;
impl pallet_popularity::PostMutator<u64> for MockPostMutator {
    fn delete_post(post_id: u64) -> Result<[u8; 32], frame_support::pallet_prelude::DispatchError> {
        let should_fail = FAIL_DELETE.with(|c| c.borrow().contains(&post_id));
        if should_fail {
            return Err(frame_support::pallet_prelude::DispatchError::Other("mock: post already gone"));
        }
        DELETED.with(|c| c.borrow_mut().push(post_id));
        // Synthesize a deterministic merkle_root.
        let mut root = [0u8; 32];
        root[0..8].copy_from_slice(&post_id.to_le_bytes());
        Ok(root)
    }
}

pub struct MockStorageReleaser;
impl pallet_popularity::StorageReleaser for MockStorageReleaser {
    fn release_fragment(h: [u8; 32]) -> frame_support::pallet_prelude::DispatchResult {
        RELEASED.with(|c| c.borrow_mut().push(h));
        Ok(())
    }
}

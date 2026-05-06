//! Block-reward pallet tests.

use crate as pallet_block_reward;
use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstU32, ConstU64, ConstU128, FindAuthor},
};
use sp_runtime::ConsensusEngineId;
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
        BlockReward: pallet_block_reward,
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
    type AccountData = pallet_balances::AccountData<Balance>;
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

impl pallet_balances::Config for Test {
    type Balance = Balance;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}

/// Mock FindAuthor: 常に AccountId 42 を返す。
pub struct MockAuthor;
impl FindAuthor<u64> for MockAuthor {
    fn find_author<'a, I>(_digests: I) -> Option<u64>
    where
        I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
    {
        Some(42u64)
    }
}

parameter_types! {
    pub const InitialReward: Balance = 5_000_000_000_000;
    pub const HalvingPeriod: u64 = 4_204_800;
    pub const MaxHalvings: u32 = 64;
}

impl pallet_block_reward::Config for Test {
    type Currency = Balances;
    type InitialReward = InitialReward;
    type HalvingPeriod = HalvingPeriod;
    type MaxHalvings = MaxHalvings;
    type AuthorOrigin = MockAuthor;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}

// ─── 純粋関数: current_reward ──────────────────────────────────────────────

#[test]
fn current_reward_era_0() {
    new_test_ext().execute_with(|| {
        assert_eq!(BlockReward::current_reward(0), 5_000_000_000_000);
        assert_eq!(BlockReward::current_reward(4_204_799), 5_000_000_000_000);
    });
}

#[test]
fn current_reward_era_1() {
    new_test_ext().execute_with(|| {
        assert_eq!(BlockReward::current_reward(4_204_800), 2_500_000_000_000);
        assert_eq!(BlockReward::current_reward(8_409_599), 2_500_000_000_000);
    });
}

#[test]
fn current_reward_era_2() {
    new_test_ext().execute_with(|| {
        assert_eq!(BlockReward::current_reward(8_409_600), 1_250_000_000_000);
    });
}

#[test]
fn current_reward_after_max_halvings_is_zero() {
    new_test_ext().execute_with(|| {
        let n = 4_204_800u64 * 64;
        assert_eq!(BlockReward::current_reward(n), 0);
    });
}

// ─── on_finalize: mint to author ──────────────────────────────────────────

use frame_support::traits::Hooks;

#[test]
fn on_finalize_mints_to_author() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        BlockReward::on_finalize(1);
        assert_eq!(Balances::free_balance(42u64), 5_000_000_000_000);
    });
}

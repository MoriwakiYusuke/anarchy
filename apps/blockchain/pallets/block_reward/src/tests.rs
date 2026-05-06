//! Block-reward pallet tests.

use crate as pallet_block_reward;
use crate::pallet::PoolDeposit;
use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstU32, ConstU64, ConstU128, FindAuthor},
};
use sp_runtime::{ConsensusEngineId, Permill};
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};
use std::sync::Mutex;

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

/// Mock pool sink: spy で deposit 量を観測できるようにする。
///
/// `STATE_LOCK` でテスト間の並列実行を直列化する (cargo test は default で並列)。
/// 各 test 冒頭で `STATE_LOCK.lock()` を取り、reset → execute → assert の順に流す。
static STATE_LOCK: Mutex<()> = Mutex::new(());
static STORAGE_POOL_SPY: Mutex<u128> = Mutex::new(0);
static REACTION_POOL_SPY: Mutex<u128> = Mutex::new(0);

pub struct MockStoragePool;
impl PoolDeposit for MockStoragePool {
    fn do_deposit(amount: u128) {
        *STORAGE_POOL_SPY.lock().unwrap() += amount;
    }
}

pub struct MockReactionPool;
impl PoolDeposit for MockReactionPool {
    fn do_deposit(amount: u128) {
        *REACTION_POOL_SPY.lock().unwrap() += amount;
    }
}

fn reset_pool_spies() {
    *STORAGE_POOL_SPY.lock().unwrap() = 0;
    *REACTION_POOL_SPY.lock().unwrap() = 0;
}

fn storage_spy() -> u128 {
    *STORAGE_POOL_SPY.lock().unwrap()
}

fn reaction_spy() -> u128 {
    *REACTION_POOL_SPY.lock().unwrap()
}

parameter_types! {
    pub const InitialReward: Balance = 5_000_000_000_000;        // 5 MORAL
    pub const TailEmission: Balance = 500_000_000_000;            // 0.5 MORAL
    pub const HalvingPeriod: u64 = 4_204_800;
    pub const MaxHalvings: u32 = 64;
    pub MinerShare: Permill = Permill::from_percent(50);
    pub StorageShare: Permill = Permill::from_percent(30);
    pub ReactionShare: Permill = Permill::from_percent(20);
}

impl pallet_block_reward::Config for Test {
    type Currency = Balances;
    type InitialReward = InitialReward;
    type TailEmission = TailEmission;
    type HalvingPeriod = HalvingPeriod;
    type MaxHalvings = MaxHalvings;
    type AuthorOrigin = MockAuthor;
    type MinerSharePermill = MinerShare;
    type StorageSharePermill = StorageShare;
    type ReactionSharePermill = ReactionShare;
    type StoragePoolSink = MockStoragePool;
    type ReactionPoolSink = MockReactionPool;
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
fn current_reward_after_max_halvings_falls_back_to_tail() {
    new_test_ext().execute_with(|| {
        let n = 4_204_800u64 * 64;
        // halved = 0 だが TailEmission = 0.5 MORAL に飽和する
        assert_eq!(BlockReward::current_reward(n), 500_000_000_000);
        // さらに先のブロックでも tail を維持
        assert_eq!(BlockReward::current_reward(n + 10_000_000), 500_000_000_000);
    });
}

#[test]
fn current_reward_falls_back_to_tail_when_halving_lower_than_tail() {
    new_test_ext().execute_with(|| {
        // 5 MORAL >> 4 = 0.3125 MORAL < 0.5 MORAL tail → 4 era 目以降は tail
        let block_at_era_4 = 4_204_800u64 * 4;
        assert_eq!(BlockReward::current_reward(block_at_era_4), 500_000_000_000);
    });
}

// ─── on_finalize: 3-way fan-out ──────────────────────────────────────────────

use frame_support::traits::Hooks;

#[test]
fn on_finalize_splits_three_ways_at_block_one() {
    let _guard = STATE_LOCK.lock().unwrap();
    new_test_ext().execute_with(|| {
        reset_pool_spies();
        System::set_block_number(1);
        BlockReward::on_finalize(1);
        // 5 MORAL × 50% = 2.5 MORAL miner
        assert_eq!(Balances::free_balance(42u64), 2_500_000_000_000);
        // 5 MORAL × 30% = 1.5 MORAL storage
        assert_eq!(storage_spy(), 1_500_000_000_000);
        // 5 MORAL × 20% = 1.0 MORAL reaction
        assert_eq!(reaction_spy(), 1_000_000_000_000);
    });
}

#[test]
fn on_finalize_uses_tail_after_max_halvings() {
    let _guard = STATE_LOCK.lock().unwrap();
    new_test_ext().execute_with(|| {
        reset_pool_spies();
        let n = 4_204_800u64 * 64;
        System::set_block_number(n);
        BlockReward::on_finalize(n);
        // 0.5 MORAL × 50% = 0.25 MORAL miner
        assert_eq!(Balances::free_balance(42u64), 250_000_000_000);
        // 0.5 MORAL × 30% = 0.15 MORAL
        assert_eq!(storage_spy(), 150_000_000_000);
        // 0.5 MORAL × 20% = 0.10 MORAL
        assert_eq!(reaction_spy(), 100_000_000_000);
    });
}

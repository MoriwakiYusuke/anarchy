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

use crate::{CurrentDifficulty, PastDifficultiesAndTimestamps};
use frame_support::traits::Hooks;

/// window 未充填 (5 ブロック) の場合は初期難易度を維持する。
#[test]
fn window_not_full_keeps_initial_difficulty() {
    new_test_ext().execute_with(|| {
        // 5 ブロック分 (window=60 未満) しか進めない
        for n in 1..=5u64 {
            System::set_block_number(n);
            Timestamp::set_timestamp(n * 30_000);
            Difficulty::on_finalize(n);
        }
        assert_eq!(CurrentDifficulty::<Test>::get(), U256::from(100_000u64));
        assert_eq!(PastDifficultiesAndTimestamps::<Test>::get().len(), 5);
    });
}

/// ちょうど目標間隔 (30s/block) で 60 ブロック進めると難易度はほぼ変わらない。
/// LWMA-3 の定常状態は ±20% 以内に収まるはず。
#[test]
fn window_full_at_target_keeps_difficulty_steady() {
    new_test_ext().execute_with(|| {
        for n in 1..=60u64 {
            System::set_block_number(n);
            Timestamp::set_timestamp(n * 30_000); // 各ブロック 30s 間隔
            Difficulty::on_finalize(n);
        }
        let d = CurrentDifficulty::<Test>::get();
        let initial = U256::from(100_000u64);
        eprintln!("steady-state difficulty: {}", d);
        assert!(d >= initial * U256::from(80u64) / U256::from(100u64),
            "difficulty {} too low (expected >= 80_000)", d);
        assert!(d <= initial * U256::from(120u64) / U256::from(100u64),
            "difficulty {} too high (expected <= 120_000)", d);
    });
}

/// ブロック生成が目標の 10 倍速い (3s/block) → 難易度は大幅に上昇する。
#[test]
fn faster_blocks_increase_difficulty() {
    new_test_ext().execute_with(|| {
        for n in 1..=60u64 {
            System::set_block_number(n);
            Timestamp::set_timestamp(n * 3_000); // 3s/block (target の 1/10)
            Difficulty::on_finalize(n);
        }
        let d = CurrentDifficulty::<Test>::get();
        eprintln!("10x-faster difficulty: {}", d);
        assert!(d > U256::from(500_000u64),
            "difficulty {} expected > 500_000 after 10x hashrate jump", d);
    });
}

/// ブロック生成が目標の 10 倍遅い (300s/block) → 難易度は下がるが MinDifficulty を下回らない。
#[test]
fn slower_blocks_decrease_difficulty_but_respect_floor() {
    new_test_ext().execute_with(|| {
        for n in 1..=60u64 {
            System::set_block_number(n);
            Timestamp::set_timestamp(n * 300_000); // 300s/block (target の 10x)
            Difficulty::on_finalize(n);
        }
        let d = CurrentDifficulty::<Test>::get();
        eprintln!("10x-slower difficulty: {}", d);
        assert!(d >= U256::from(10_000u64), "floor violated: {}", d);
        assert!(d < U256::from(50_000u64),
            "difficulty {} expected < 50_000 after 10x slowdown", d);
    });
}

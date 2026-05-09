//! Tests for pallet-economic-params (TSTS F5).

use crate as pallet_economic_params;
use frame_support::{
    assert_noop, assert_ok, construct_runtime, parameter_types,
    traits::{ConstU32, ConstU64, ConstU128, EnsureOrigin},
};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, Permill,
};

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        EconomicParams: pallet_economic_params,
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

parameter_types! {
    pub DefaultPostStorageShare: Permill = Permill::from_percent(50);
    pub DefaultPostReactionShare: Permill = Permill::from_percent(20);
    pub DefaultDmStorageShare: Permill = Permill::from_percent(50);
    pub DefaultDmStealthShare: Permill = Permill::from_percent(20);
    pub DefaultMinerShare: Permill = Permill::from_percent(50);
    pub DefaultStorageShare: Permill = Permill::from_percent(30);
    pub DefaultReactionShare: Permill = Permill::from_percent(20);
}

impl pallet_economic_params::Config for Test {
    type GovernanceOrigin = EnsureRoot<u64>;
    type DefaultPostStorageSharePermill = DefaultPostStorageShare;
    type DefaultPostReactionSharePermill = DefaultPostReactionShare;
    type DefaultDmStorageSharePermill = DefaultDmStorageShare;
    type DefaultDmStealthSharePermill = DefaultDmStealthShare;
    type DefaultMinerSharePermill = DefaultMinerShare;
    type DefaultStorageSharePermill = DefaultStorageShare;
    type DefaultReactionSharePermill = DefaultReactionShare;
    type DefaultReactorLockMin = ConstU128<100_000_000_000>; // 0.1 MORAL
    type DefaultBondPerGB = ConstU128<10_000_000_000_000>;    // 10 MORAL
    type DefaultSlashRatePerFailPpm = ConstU32<50_000>;       // 5%
    type DefaultBaseFeeMin = ConstU128<100>;
    type DefaultBaseFeeMax = ConstU128<100_000_000_000>;
}

fn new_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}

#[test]
fn defaults_returned_when_unset() {
    new_ext().execute_with(|| {
        // 何も set してない時点では Default* がそのまま返る
        assert_eq!(EconomicParams::effective_post_storage_share(), Permill::from_percent(50));
        assert_eq!(EconomicParams::effective_dm_stealth_share(), Permill::from_percent(20));
        assert_eq!(EconomicParams::effective_reactor_lock_min(), 100_000_000_000);
        assert_eq!(EconomicParams::effective_bond_per_gb(), 10_000_000_000_000);
        assert_eq!(EconomicParams::effective_slash_rate_per_fail_ppm(), 50_000);
        assert_eq!(EconomicParams::effective_base_fee_min(), 100);
        assert_eq!(EconomicParams::effective_base_fee_max(), 100_000_000_000);
    });
}

#[test]
fn set_post_storage_share_works_for_root() {
    new_ext().execute_with(|| {
        assert_ok!(EconomicParams::set_post_storage_share(
            RuntimeOrigin::root(),
            Permill::from_percent(40),
        ));
        assert_eq!(EconomicParams::effective_post_storage_share(), Permill::from_percent(40));
    });
}

#[test]
fn set_post_share_rejects_sum_above_hundred() {
    new_ext().execute_with(|| {
        // default reaction share = 20%. set storage to 90% → sum = 110% → reject
        assert_noop!(
            EconomicParams::set_post_storage_share(
                RuntimeOrigin::root(),
                Permill::from_percent(90),
            ),
            pallet_economic_params::Error::<Test>::SharesSumExceedsHundred
        );
    });
}

#[test]
fn set_dm_share_rejects_sum_above_hundred() {
    new_ext().execute_with(|| {
        // default storage = 50%. set stealth to 60% → sum = 110% → reject
        assert_noop!(
            EconomicParams::set_dm_stealth_share(
                RuntimeOrigin::root(),
                Permill::from_percent(60),
            ),
            pallet_economic_params::Error::<Test>::SharesSumExceedsHundred
        );
    });
}

#[test]
fn set_slash_rate_above_one_million_rejected() {
    new_ext().execute_with(|| {
        assert_noop!(
            EconomicParams::set_slash_rate_per_fail_ppm(RuntimeOrigin::root(), 1_000_001),
            pallet_economic_params::Error::<Test>::SlashRateAbove100Percent
        );
        // 100% (1_000_000) はギリギリ OK
        assert_ok!(EconomicParams::set_slash_rate_per_fail_ppm(RuntimeOrigin::root(), 1_000_000));
    });
}

#[test]
fn set_post_storage_share_fails_for_non_root() {
    new_ext().execute_with(|| {
        assert_noop!(
            EconomicParams::set_post_storage_share(
                RuntimeOrigin::signed(1),
                Permill::from_percent(40),
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn set_block_reward_shares_validates_sum() {
    new_ext().execute_with(|| {
        // 合計 110% → エラー
        assert_noop!(
            EconomicParams::set_block_reward_shares(
                RuntimeOrigin::root(),
                Permill::from_percent(60),
                Permill::from_percent(30),
                Permill::from_percent(20),
            ),
            pallet_economic_params::Error::<Test>::SharesSumExceedsHundred
        );
        // 合計 100% → OK
        assert_ok!(EconomicParams::set_block_reward_shares(
            RuntimeOrigin::root(),
            Permill::from_percent(60),
            Permill::from_percent(25),
            Permill::from_percent(15),
        ));
        assert_eq!(EconomicParams::effective_miner_share(), Permill::from_percent(60));
        assert_eq!(EconomicParams::effective_storage_share(), Permill::from_percent(25));
        assert_eq!(EconomicParams::effective_reaction_share(), Permill::from_percent(15));
    });
}

#[test]
fn set_base_fee_range_validates_inversion() {
    new_ext().execute_with(|| {
        assert_noop!(
            EconomicParams::set_base_fee_range(RuntimeOrigin::root(), 1_000_000_000, 100),
            pallet_economic_params::Error::<Test>::InvertedBaseFeeRange
        );
        assert_ok!(EconomicParams::set_base_fee_range(
            RuntimeOrigin::root(),
            500,
            5_000_000_000,
        ));
        assert_eq!(EconomicParams::effective_base_fee_min(), 500);
        assert_eq!(EconomicParams::effective_base_fee_max(), 5_000_000_000);
    });
}

#[test]
fn set_reactor_lock_min_works() {
    new_ext().execute_with(|| {
        assert_ok!(EconomicParams::set_reactor_lock_min(
            RuntimeOrigin::root(),
            500_000_000_000, // 0.5 MORAL
        ));
        assert_eq!(EconomicParams::effective_reactor_lock_min(), 500_000_000_000);
    });
}

#[test]
fn set_bond_per_gb_works() {
    new_ext().execute_with(|| {
        assert_ok!(EconomicParams::set_bond_per_gb(
            RuntimeOrigin::root(),
            50_000_000_000_000, // 50 MORAL/GB
        ));
        assert_eq!(EconomicParams::effective_bond_per_gb(), 50_000_000_000_000);
    });
}

//! Mock runtime for `pallet-messaging` unit tests.

use crate::{self as pallet_messaging, StealthRewardInterface};
use frame_support::{
    derive_impl,
    dispatch::DispatchResult,
    traits::{ConstU32, ConstU64, ConstU128},
};
use pallet_storage::{FragmentId, StorageInterface};
use sp_runtime::BuildStorage;
use std::cell::RefCell;

pub type Balance = u128;
pub type AccountId = u64;
pub type BlockNumber = u64;
type Block = frame_system::mocking::MockBlock<Test>;

thread_local! {
    /// `MockStorage::do_deposit_to_reward_pool` に渡された累計額。
    static STORAGE_POOL_DEPOSITS: RefCell<u128> = const { RefCell::new(0) };
    /// `MockStealthReward::do_deposit_to_stealth_reward_pool` に渡された累計額。
    static STEALTH_REWARD_DEPOSITS: RefCell<u128> = const { RefCell::new(0) };
}

/// Storage pool への流入累計を取得 (テスト用)。
pub fn storage_pool_deposits() -> u128 {
    STORAGE_POOL_DEPOSITS.with(|c| *c.borrow())
}

/// StealthReward pool への流入累計を取得 (テスト用)。
pub fn stealth_reward_deposits() -> u128 {
    STEALTH_REWARD_DEPOSITS.with(|c| *c.borrow())
}

/// プール流入カウンタをリセット (テスト分離用)。
pub fn reset_pool_deposits() {
    STORAGE_POOL_DEPOSITS.with(|c| *c.borrow_mut() = 0);
    STEALTH_REWARD_DEPOSITS.with(|c| *c.borrow_mut() = 0);
}

/// Mock pallet-storage interface。`do_deposit_to_reward_pool` のみカウント。
pub struct MockStorage;

impl StorageInterface<AccountId, BlockNumber> for MockStorage {
    fn do_register_fragment(
        _fragment_id: FragmentId,
        _size: u32,
        _creator: AccountId,
        _created_at: BlockNumber,
    ) -> DispatchResult {
        Ok(())
    }

    fn do_register_kzg_fragment(
        _owner: AccountId,
        _content_hash: [u8; 32],
        _commitment: sp_std::vec::Vec<u8>,
        _data_size: u32,
        _fragment_count: u8,
        _threshold: u8,
    ) -> DispatchResult {
        Ok(())
    }

    fn do_deposit_to_reward_pool(amount: u128) {
        STORAGE_POOL_DEPOSITS.with(|c| *c.borrow_mut() += amount);
    }
}

/// Mock stealth reward interface。加算をカウント。
pub struct MockStealthReward;

impl StealthRewardInterface for MockStealthReward {
    fn do_deposit_to_stealth_reward_pool(amount: u128) {
        STEALTH_REWARD_DEPOSITS.with(|c| *c.borrow_mut() += amount);
    }
}

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Messaging: pallet_messaging,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
    type AccountId = AccountId;
    type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
}

impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<0>;
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}

/// 1 MORAL = 10^12。contracts/pallet-messaging-extrinsics.md §Dependencies と同値。
const MORAL: Balance = 1_000_000_000_000;

impl pallet_messaging::Config for Test {
    type NativeToken = Balances;
    type Storage = MockStorage;
    type StealthReward = MockStealthReward;
    type MaxDispatchesPerBlock = ConstU32<256>;
    type DmBaseCost = ConstU128<MORAL>;           // 1 MORAL
    type DmByteCost = ConstU128<50_000_000_000>;  // 0.05 MORAL / byte
    type MaxDmCiphertextLen = ConstU64<262_144>;
    type WeightInfo = ();
}

/// Test accounts
pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;
/// 256 KB バケット送信時でも残高が足りるリッチ送信者 (≈ 13 108 MORAL 以上必要)。
pub const RICH_SENDER: AccountId = 100;
/// 最小バケット (1 KB, ≈ 52.2 MORAL) すら賄えない貧弱送信者。
pub const POOR_SENDER: AccountId = 101;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, 1_000 * MORAL),
            (BOB, 1_000 * MORAL),
            (CHARLIE, 1_000 * MORAL),
            (RICH_SENDER, 100_000 * MORAL),
            (POOR_SENDER, 10 * MORAL),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        System::set_block_number(1);
        reset_pool_deposits();
    });
    ext
}

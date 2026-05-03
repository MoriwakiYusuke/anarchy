//! T092: Smoke test for pallet-stealth interop (FR-024 pre-fund path).
//!
//! 目的:
//! 1. 同一 runtime に `pallet-messaging` と `pallet-stealth` を組み込んでコンパイル
//!    できることを型レベルで確認する。
//! 2. `pallet_stealth::Pallet::send_to_stealth(origin, stealth_address,
//!    ephemeral_pubkey, amount)` が `apps/frontend/src/lib/dm/sender.ts`
//!    (Phase 3 実装予定) の tx1 (pre-fund) で必要とする call signature と
//!    一致していることを確認する。
//!
//! 内容ロジックは Phase 3 (T034–T036) で書く `send_dm` extrinsic に依存しないため、
//! pre-fund tx1 → tx2 順序の整合は本テストの対象外 (T090 / T091 でカバー)。

#![cfg(test)]

use crate as pallet_messaging;
use crate::StealthRewardInterface;
use frame_support::{
    assert_ok,
    derive_impl,
    traits::{ConstU32, ConstU64, ConstU128},
};
use pallet_storage::{FragmentId, StorageInterface};
use sp_runtime::{traits::IdentityLookup, BuildStorage, DispatchResult};

pub type Balance = u128;
pub type AccountId = u64;
type Block = frame_system::mocking::MockBlock<Test>;

/// Mock pallet-storage interface (no-op)。
pub struct MockStorage;

impl StorageInterface<AccountId, u64> for MockStorage {
    fn do_register_fragment(
        _fragment_id: FragmentId,
        _size: u32,
        _creator: AccountId,
        _created_at: u64,
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

    fn do_deposit_to_reward_pool(_amount: u128) {}

    fn do_release_fragment(_content_hash: [u8; 32]) -> DispatchResult {
        Ok(())
    }
}

/// Mock stealth reward interface (no-op)。
pub struct MockStealthReward;

impl StealthRewardInterface for MockStealthReward {
    fn do_deposit_to_stealth_reward_pool(_amount: u128) {}
}

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Stealth: pallet_stealth,
        Messaging: pallet_messaging,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
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

impl pallet_stealth::Config for Test {
    type Currency = Balances;
    type MaxEntriesPerBlock = ConstU32<1000>;
    type WeightInfo = ();
}

const MORAL: Balance = 1_000_000_000_000;

impl pallet_messaging::Config for Test {
    type NativeToken = Balances;
    type Storage = MockStorage;
    type StealthReward = MockStealthReward;
    type MaxDispatchesPerBlock = ConstU32<256>;
    type DmBaseCost = ConstU128<MORAL>;
    type DmByteCost = ConstU128<50_000_000_000>;
    type MaxDmCiphertextLen = ConstU64<262_144>;
    type WeightInfo = ();
}

const ALICE: AccountId = 1;
const SENDER_STEALTH: AccountId = 1001;

fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(ALICE, 1_000 * MORAL)],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

/// FR-024 の pre-fund 経路: Alice (main account) が pallet_stealth::send_to_stealth
/// で sender_stealth に送金する型シグネチャを確認する。実値が通ることまで確認することで
/// 「単純な型エイリアスがある」だけでなく「両 pallet が同一 runtime で生きている」ことを
/// 保証する。
#[test]
fn stealth_pre_fund_call_resolves_at_type_level() {
    new_test_ext().execute_with(|| {
        let ephemeral_pubkey: [u8; 32] = [0x42; 32];
        let pre_fund_amount: Balance = 5 * MORAL;

        assert_ok!(pallet_stealth::Pallet::<Test>::send_to_stealth(
            RuntimeOrigin::signed(ALICE),
            SENDER_STEALTH,
            ephemeral_pubkey,
            pre_fund_amount,
        ));

        // sender_stealth が pre-fund 額をしっかり受け取ったことを確認 (tx2 の send_dm が
        // この残高から fee を引けることを担保する)。
        assert_eq!(
            pallet_balances::Pallet::<Test>::free_balance(SENDER_STEALTH),
            pre_fund_amount,
        );
    });
}

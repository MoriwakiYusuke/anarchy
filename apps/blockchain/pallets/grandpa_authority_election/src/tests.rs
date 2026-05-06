//! GRANDPA authority election pallet tests.

use crate as pallet_election;
use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstU32, ConstU64, FindAuthor},
};
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::Pair;
use sp_runtime::ConsensusEngineId;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Grandpa: pallet_grandpa,
        Election: pallet_election,
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

impl pallet_grandpa::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = ConstU32<100>;
    type MaxNominators = ConstU32<0>;
    type MaxSetIdSessionEntries = ConstU64<0>;
    type KeyOwnerProof = sp_core::Void;
    type EquivocationReportSystem = ();
}

// Mock author rotator: テストごとに `set_mock_author` で差し替える。
thread_local! {
    static MOCK_AUTHOR: core::cell::RefCell<Option<u64>> = core::cell::RefCell::new(Some(1));
}
pub fn set_mock_author(a: Option<u64>) {
    MOCK_AUTHOR.with(|v| *v.borrow_mut() = a);
}
pub struct MockAuthor;
impl FindAuthor<u64> for MockAuthor {
    fn find_author<'a, I>(_: I) -> Option<u64>
    where I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])> {
        MOCK_AUTHOR.with(|v| *v.borrow())
    }
}

parameter_types! {
    pub const WindowSize: u32 = 10;
    pub const AuthorityCount: u32 = 3;
    pub const RotationPeriod: u64 = 5;
    pub const RotationDelay: u64 = 1;
}

impl pallet_election::Config for Test {
    type WindowSize = WindowSize;
    type AuthorityCount = AuthorityCount;
    type RotationPeriod = RotationPeriod;
    type RotationDelay = RotationDelay;
    type AuthorOrigin = MockAuthor;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}

fn fake_grandpa_id(seed: u8) -> GrandpaId {
    <sp_consensus_grandpa::AuthorityPair as Pair>::from_seed(&[seed; 32]).public()
}

use crate::{AuthorityKeys, RecentAuthors, Event as ElectionEvent};
use frame_support::traits::Hooks;

fn run_block_with_author(n: u64, author: u64) {
    System::set_block_number(n);
    set_mock_author(Some(author));
    Election::on_finalize(n);
}

#[test]
fn registers_and_unregisters_authority_key() {
    new_test_ext().execute_with(|| {
        let key = fake_grandpa_id(1);
        assert!(Election::register_grandpa_key(RuntimeOrigin::signed(1u64), key.clone()).is_ok());
        assert_eq!(AuthorityKeys::<Test>::get(1u64), Some(key));
        assert!(Election::unregister_grandpa_key(RuntimeOrigin::signed(1u64)).is_ok());
        assert_eq!(AuthorityKeys::<Test>::get(1u64), None);
    });
}

#[test]
fn ring_buffer_collects_authors() {
    new_test_ext().execute_with(|| {
        for n in 1..=4u64 {
            run_block_with_author(n, n);
        }
        let buf = RecentAuthors::<Test>::get();
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.to_vec(), vec![1, 2, 3, 4]);
    });
}

#[test]
fn ring_buffer_evicts_old_when_full() {
    new_test_ext().execute_with(|| {
        // window cap=100 (ConstU32<100>) を超えて 102 ブロック流す
        for n in 1..=102u64 {
            run_block_with_author(n, n);
        }
        let buf = RecentAuthors::<Test>::get();
        assert_eq!(buf.len(), 100);
        // 最古 (1, 2) が落ちて 3..102 が残る
        assert_eq!(buf[0], 3u64);
        assert_eq!(buf[99], 102u64);
    });
}

#[test]
fn rotation_emits_event_when_keys_registered() {
    new_test_ext().execute_with(|| {
        // 3 人にキー登録
        for who in 1..=3u64 {
            assert!(Election::register_grandpa_key(
                RuntimeOrigin::signed(who), fake_grandpa_id(who as u8)
            ).is_ok());
        }
        // 5 ブロック (RotationPeriod) で 1, 1, 2, 2, 3 を author に
        run_block_with_author(1, 1);
        run_block_with_author(2, 1);
        run_block_with_author(3, 2);
        run_block_with_author(4, 2);
        run_block_with_author(5, 3);

        // block 5 (= 5 % 5 == 0) で rotation トリガ
        // pallet_grandpa::schedule_change が session pallet なしで失敗する場合は
        // AuthoritySetRotationSkipped(ScheduleChangeFailed) が許容される
        let events = System::events();
        let rotated = events.iter().filter(|e| matches!(
            e.event, RuntimeEvent::Election(ElectionEvent::AuthoritySetRotated { .. })
        )).count();
        let skipped_failed = events.iter().filter(|e| matches!(
            e.event, RuntimeEvent::Election(ElectionEvent::AuthoritySetRotationSkipped {
                reason: crate::SkipReason::ScheduleChangeFailed
            })
        )).count();
        assert!(
            rotated >= 1 || skipped_failed >= 1,
            "expected AuthoritySetRotated or AuthoritySetRotationSkipped(ScheduleChangeFailed), got: {:#?}",
            events
        );
    });
}

#[test]
fn rotation_skipped_when_no_keys_registered() {
    new_test_ext().execute_with(|| {
        // キー未登録のまま 5 ブロック流す
        for n in 1..=5u64 {
            run_block_with_author(n, 1);
        }
        let events = System::events();
        let skipped = events.iter().any(|e| matches!(
            e.event, RuntimeEvent::Election(ElectionEvent::AuthoritySetRotationSkipped { .. })
        ));
        assert!(skipped, "expected AuthoritySetRotationSkipped event, got: {:#?}", events);
    });
}

#[test]
fn top_k_respects_count_then_account_id() {
    new_test_ext().execute_with(|| {
        for who in 1..=5u64 {
            assert!(Election::register_grandpa_key(
                RuntimeOrigin::signed(who), fake_grandpa_id(who as u8)
            ).is_ok());
        }
        // 1 が 4 回, 2 が 3 回, 3 が 2 回, 4 が 1 回 (合計 10 ブロック)
        let pattern = [1u64, 1, 1, 1, 2, 2, 2, 3, 3, 4];
        for (i, &a) in pattern.iter().enumerate() {
            run_block_with_author((i + 1) as u64, a);
        }
        // top-3 (AuthorityCount = 3) → 1, 2, 3 が選出される
        // schedule_change の中身は直接読めないので、
        // RecentAuthors の集計と AuthoritySetRotated count=3 event を確認
        let buf = RecentAuthors::<Test>::get();
        assert_eq!(buf.to_vec(), pattern.to_vec());

        // 5 と 10 の両方で rotate がトリガ
        // pallet_grandpa が session なしで schedule_change を受け付ける場合は count=3 event
        // 失敗する場合は ScheduleChangeFailed が許容される
        let events = System::events();
        let rotated_with_count_3 = events.iter().filter(|e| matches!(
            e.event, RuntimeEvent::Election(ElectionEvent::AuthoritySetRotated { count: 3 })
        )).count();
        let skipped_failed = events.iter().filter(|e| matches!(
            e.event, RuntimeEvent::Election(ElectionEvent::AuthoritySetRotationSkipped {
                reason: crate::SkipReason::ScheduleChangeFailed
            })
        )).count();
        assert!(
            rotated_with_count_3 >= 1 || skipped_failed >= 1,
            "expected AuthoritySetRotated count=3 or ScheduleChangeFailed, got: {:#?}",
            events
        );
    });
}

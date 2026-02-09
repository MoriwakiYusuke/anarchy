//! Storage Pallet Tests
//!
//! TDD: Tests written first based on spec.md requirements
//! T-001 to T-009 cover all functional requirements

use crate::{self as pallet_storage, Error, Event, FragmentId};
use frame_support::{
    assert_noop, assert_ok,
    traits::{ConstU32, ConstU64},
    BoundedVec,
};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

// Test runtime construction
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

// Storage pallet constants for testing
// MaxFragmentSize = 1MB
// MaxPeerIdLen = 64 bytes
// MaxHoldersPerFragment = 100
// MaxFragmentsPerNode = 10,000
impl pallet_storage::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxFragmentSize = ConstU32<1_048_576>; // 1MB
    type MaxPeerIdLen = ConstU32<64>;
    type MaxHoldersPerFragment = ConstU32<100>;
    type MaxFragmentsPerNode = ConstU32<10_000>;
}

/// Build test externalities
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

/// Helper: Create a test fragment ID
fn test_fragment_id(n: u8) -> FragmentId {
    let mut id = [0u8; 32];
    id[0] = n;
    id
}

/// Helper: Create a test PeerID
fn test_peer_id(n: u8) -> BoundedVec<u8, ConstU32<64>> {
    let mut id = vec![0u8; 38]; // Minimum valid PeerID length
    id[0] = n;
    BoundedVec::try_from(id).unwrap()
}

// ============ User Story 1: 断片メタデータの登録 ============

/// T-001: Fragment registration succeeds
#[test]
fn t001_register_fragment_succeeds() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let fragment_id = test_fragment_id(1);
        let size = 1024u32;

        // Register fragment
        assert_ok!(Storage::register_fragment(
            RuntimeOrigin::signed(account),
            fragment_id,
            size
        ));

        // Verify storage
        let metadata = Storage::fragments(fragment_id).expect("Fragment should exist");
        assert_eq!(metadata.size, size);
        assert_eq!(metadata.creator, account);
        assert_eq!(metadata.created_at, 1);

        // Verify event
        System::assert_last_event(
            Event::FragmentRegistered {
                fragment_id,
                creator: account,
                size,
            }
            .into(),
        );
    });
}

/// T-002: Duplicate fragment ID returns error
#[test]
fn t002_duplicate_fragment_id_fails() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let fragment_id = test_fragment_id(1);
        let size = 1024u32;

        // First registration succeeds
        assert_ok!(Storage::register_fragment(
            RuntimeOrigin::signed(account),
            fragment_id,
            size
        ));

        // Second registration fails
        assert_noop!(
            Storage::register_fragment(RuntimeOrigin::signed(account), fragment_id, size),
            Error::<Test>::FragmentAlreadyExists
        );
    });
}

/// Additional test: Fragment size validation
#[test]
fn fragment_size_too_large_fails() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let fragment_id = test_fragment_id(1);
        let size = 2_000_000u32; // > 1MB

        assert_noop!(
            Storage::register_fragment(RuntimeOrigin::signed(account), fragment_id, size),
            Error::<Test>::FragmentTooLarge
        );
    });
}

/// Additional test: Fragment size zero fails
#[test]
fn fragment_size_zero_fails() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let fragment_id = test_fragment_id(1);
        let size = 0u32;

        assert_noop!(
            Storage::register_fragment(RuntimeOrigin::signed(account), fragment_id, size),
            Error::<Test>::FragmentTooSmall
        );
    });
}

// ============ User Story 2: ストレージノードの登録 ============

/// T-003: Node registration succeeds
#[test]
fn t003_register_node_succeeds() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let peer_id = test_peer_id(1);
        let capacity = 10_000_000_000u64; // 10GB

        // Register node
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator),
            peer_id.clone(),
            capacity
        ));

        // Verify storage
        let node_info = Storage::storage_nodes(&peer_id).expect("Node should exist");
        assert_eq!(node_info.operator, operator);
        assert_eq!(node_info.capacity, capacity);
        assert_eq!(node_info.registered_at, 1);

        // Verify reverse lookup
        let lookup = Storage::operator_nodes(operator).expect("Operator lookup should exist");
        assert_eq!(lookup, peer_id);

        // Verify event
        System::assert_last_event(
            Event::NodeRegistered {
                peer_id,
                operator,
                capacity,
            }
            .into(),
        );
    });
}

/// T-004: Duplicate PeerID returns error
#[test]
fn t004_duplicate_peer_id_fails() {
    new_test_ext().execute_with(|| {
        let operator1 = 1u64;
        let operator2 = 2u64;
        let peer_id = test_peer_id(1);
        let capacity = 10_000_000_000u64;

        // First registration succeeds
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator1),
            peer_id.clone(),
            capacity
        ));

        // Second registration with same PeerID fails
        assert_noop!(
            Storage::register_node(RuntimeOrigin::signed(operator2), peer_id, capacity),
            Error::<Test>::NodeAlreadyRegistered
        );
    });
}

/// Additional test: Operator already has node fails
#[test]
fn operator_already_has_node_fails() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let peer_id1 = test_peer_id(1);
        let peer_id2 = test_peer_id(2);
        let capacity = 10_000_000_000u64;

        // First registration succeeds
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator),
            peer_id1,
            capacity
        ));

        // Second registration with different PeerID fails
        assert_noop!(
            Storage::register_node(RuntimeOrigin::signed(operator), peer_id2, capacity),
            Error::<Test>::OperatorAlreadyHasNode
        );
    });
}

/// T-005: Node update succeeds
#[test]
fn t005_update_node_succeeds() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let peer_id = test_peer_id(1);
        let capacity = 10_000_000_000u64;
        let new_capacity = 20_000_000_000u64;

        // Register node first
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator),
            peer_id.clone(),
            capacity
        ));

        // Update capacity
        assert_ok!(Storage::update_node(
            RuntimeOrigin::signed(operator),
            new_capacity
        ));

        // Verify updated
        let node_info = Storage::storage_nodes(&peer_id).expect("Node should exist");
        assert_eq!(node_info.capacity, new_capacity);

        // Verify event
        System::assert_last_event(
            Event::NodeUpdated {
                peer_id,
                new_capacity,
            }
            .into(),
        );
    });
}

/// T-006: Node unregistration succeeds
#[test]
fn t006_unregister_node_succeeds() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let peer_id = test_peer_id(1);
        let capacity = 10_000_000_000u64;

        // Register node first
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator),
            peer_id.clone(),
            capacity
        ));

        // Unregister
        assert_ok!(Storage::unregister_node(RuntimeOrigin::signed(operator)));

        // Verify removed
        assert!(Storage::storage_nodes(&peer_id).is_none());
        assert!(Storage::operator_nodes(operator).is_none());

        // Verify event
        System::assert_last_event(
            Event::NodeUnregistered {
                peer_id,
                operator,
            }
            .into(),
        );
    });
}

/// Additional test: Unregister with holdings fails
#[test]
fn unregister_with_holdings_fails() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let peer_id = test_peer_id(1);
        let capacity = 10_000_000_000u64;
        let fragment_id = test_fragment_id(1);

        // Setup: Register node and fragment, then declare holding
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator),
            peer_id.clone(),
            capacity
        ));
        assert_ok!(Storage::register_fragment(
            RuntimeOrigin::signed(operator),
            fragment_id,
            1024
        ));
        assert_ok!(Storage::declare_holding(
            RuntimeOrigin::signed(operator),
            fragment_id
        ));

        // Unregister should fail
        assert_noop!(
            Storage::unregister_node(RuntimeOrigin::signed(operator)),
            Error::<Test>::NodeHasHoldings
        );
    });
}

// ============ User Story 3: 保持表明 ============

/// T-007: Declare holding succeeds
#[test]
fn t007_declare_holding_succeeds() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let peer_id = test_peer_id(1);
        let capacity = 10_000_000_000u64;
        let fragment_id = test_fragment_id(1);

        // Setup
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator),
            peer_id.clone(),
            capacity
        ));
        assert_ok!(Storage::register_fragment(
            RuntimeOrigin::signed(operator),
            fragment_id,
            1024
        ));

        // Declare holding
        assert_ok!(Storage::declare_holding(
            RuntimeOrigin::signed(operator),
            fragment_id
        ));

        // Verify FragmentHolders
        let holders = Storage::fragment_holders(fragment_id);
        assert_eq!(holders.len(), 1);
        assert_eq!(holders[0], peer_id);

        // Verify NodeHoldings
        let holdings = Storage::node_holdings(&peer_id);
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0], fragment_id);

        // Verify event
        System::assert_last_event(
            Event::HoldingDeclared {
                peer_id,
                fragment_id,
            }
            .into(),
        );
    });
}

/// T-008: Revoke holding succeeds
#[test]
fn t008_revoke_holding_succeeds() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let peer_id = test_peer_id(1);
        let capacity = 10_000_000_000u64;
        let fragment_id = test_fragment_id(1);

        // Setup
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator),
            peer_id.clone(),
            capacity
        ));
        assert_ok!(Storage::register_fragment(
            RuntimeOrigin::signed(operator),
            fragment_id,
            1024
        ));
        assert_ok!(Storage::declare_holding(
            RuntimeOrigin::signed(operator),
            fragment_id
        ));

        // Revoke holding
        assert_ok!(Storage::revoke_holding(
            RuntimeOrigin::signed(operator),
            fragment_id
        ));

        // Verify removed
        let holders = Storage::fragment_holders(fragment_id);
        assert!(holders.is_empty());

        let holdings = Storage::node_holdings(&peer_id);
        assert!(holdings.is_empty());

        // Verify event
        System::assert_last_event(
            Event::HoldingRevoked {
                peer_id,
                fragment_id,
            }
            .into(),
        );
    });
}

/// T-009: Get fragment holders returns correct list
#[test]
fn t009_get_fragment_holders() {
    new_test_ext().execute_with(|| {
        let operator1 = 1u64;
        let operator2 = 2u64;
        let peer_id1 = test_peer_id(1);
        let peer_id2 = test_peer_id(2);
        let capacity = 10_000_000_000u64;
        let fragment_id = test_fragment_id(1);

        // Setup: Register two nodes
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator1),
            peer_id1.clone(),
            capacity
        ));
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator2),
            peer_id2.clone(),
            capacity
        ));

        // Register fragment
        assert_ok!(Storage::register_fragment(
            RuntimeOrigin::signed(operator1),
            fragment_id,
            1024
        ));

        // Both nodes declare holding
        assert_ok!(Storage::declare_holding(
            RuntimeOrigin::signed(operator1),
            fragment_id
        ));
        assert_ok!(Storage::declare_holding(
            RuntimeOrigin::signed(operator2),
            fragment_id
        ));

        // Verify holder list
        let holders = Storage::fragment_holders(fragment_id);
        assert_eq!(holders.len(), 2);
        assert!(holders.contains(&peer_id1));
        assert!(holders.contains(&peer_id2));
    });
}

/// Additional test: Declare holding is idempotent
#[test]
fn declare_holding_idempotent() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let peer_id = test_peer_id(1);
        let capacity = 10_000_000_000u64;
        let fragment_id = test_fragment_id(1);

        // Setup
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator),
            peer_id.clone(),
            capacity
        ));
        assert_ok!(Storage::register_fragment(
            RuntimeOrigin::signed(operator),
            fragment_id,
            1024
        ));

        // First declaration
        assert_ok!(Storage::declare_holding(
            RuntimeOrigin::signed(operator),
            fragment_id
        ));

        // Second declaration should succeed (idempotent)
        assert_ok!(Storage::declare_holding(
            RuntimeOrigin::signed(operator),
            fragment_id
        ));

        // Should still only have one entry
        let holders = Storage::fragment_holders(fragment_id);
        assert_eq!(holders.len(), 1);
    });
}

/// Additional test: Declare holding without registered node fails
#[test]
fn declare_holding_without_node_fails() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let fragment_id = test_fragment_id(1);

        // Register fragment only
        assert_ok!(Storage::register_fragment(
            RuntimeOrigin::signed(operator),
            fragment_id,
            1024
        ));

        // Declare holding without node should fail
        assert_noop!(
            Storage::declare_holding(RuntimeOrigin::signed(operator), fragment_id),
            Error::<Test>::NodeNotRegistered
        );
    });
}

/// Additional test: Declare holding for non-existent fragment fails
#[test]
fn declare_holding_nonexistent_fragment_fails() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let peer_id = test_peer_id(1);
        let capacity = 10_000_000_000u64;
        let fragment_id = test_fragment_id(99); // Non-existent

        // Register node only
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator),
            peer_id,
            capacity
        ));

        // Declare holding for non-existent fragment should fail
        assert_noop!(
            Storage::declare_holding(RuntimeOrigin::signed(operator), fragment_id),
            Error::<Test>::FragmentNotFound
        );
    });
}

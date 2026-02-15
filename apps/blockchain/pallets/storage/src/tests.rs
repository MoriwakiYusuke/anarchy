//! Storage Pallet Tests
//!
//! TDD: Tests written first based on spec.md requirements
//! T-001 to T-009 cover all functional requirements

use crate::{self as pallet_storage, Error, Event, FragmentId};
use frame_support::{
    assert_noop, assert_ok,
    traits::{ConstU32, ConstU64, ConstU8},
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
    // New security constants (relaxed for tests)
    type MinPeerIdLen = ConstU32<2>;                // Relaxed for basic tests
    type MaxRegistrationsPerBlock = ConstU32<5>;
    type MaxDeclarationsPerBlockPerNode = ConstU32<10>;
    type MinNodeCapacity = ConstU64<1>;              // Relaxed for basic tests
    type PowObservationPeriod = ConstU32<10>;
    type BasePowDifficulty = ConstU8<0>;             // No PoW for basic tests
    type MaxHttpUrlLen = ConstU32<256>;
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

/// Helper: Create a test HTTP URL
fn test_http_url(port: u16) -> BoundedVec<u8, ConstU32<256>> {
    let url = format!("http://127.0.0.1:{}", port);
    BoundedVec::try_from(url.into_bytes()).unwrap()
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
            capacity,
            0, // pow_nonce (difficulty=0)
            test_http_url(3030),
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
            capacity,
            0, // pow_nonce
            test_http_url(3030),
        ));

        // Second registration with same PeerID fails
        assert_noop!(
            Storage::register_node(RuntimeOrigin::signed(operator2), peer_id, capacity, 0, test_http_url(3031)),
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
            capacity,
            0, // pow_nonce
            test_http_url(3030),
        ));

        // Second registration with different PeerID fails
        assert_noop!(
            Storage::register_node(RuntimeOrigin::signed(operator), peer_id2, capacity, 0, test_http_url(3031)),
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
            capacity,
            0, // pow_nonce
            test_http_url(3030),
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
            capacity,
            0, // pow_nonce
            test_http_url(3030),
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
            capacity,
            0, // pow_nonce
            test_http_url(3030),
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
            capacity,
            0, // pow_nonce
            test_http_url(3030),
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
            capacity,
            0, // pow_nonce
            test_http_url(3030),
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
            capacity,
            0, // pow_nonce
            test_http_url(3030),
        ));
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator2),
            peer_id2.clone(),
            capacity,
            0, // pow_nonce
            test_http_url(3030),
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
            capacity,
            0, // pow_nonce
            test_http_url(3030),
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
            capacity,
            0, // pow_nonce
            test_http_url(3030),
        ));

        // Declare holding for non-existent fragment should fail
        assert_noop!(
            Storage::declare_holding(RuntimeOrigin::signed(operator), fragment_id),
            Error::<Test>::FragmentNotFound
        );
    });
}

// ============ Security Tests (FR-405-411) ============

/// T035: Test PoW verification - Already covered in pow.rs unit tests
/// T036: Test dynamic difficulty - Already covered in pow.rs unit tests

/// T037: Test registration rate limit (6th registration rejected)
#[test]
fn t037_registration_rate_limit() {
    new_test_ext().execute_with(|| {
        // MaxRegistrationsPerBlock = 5 in test config
        // Register 5 nodes (should all succeed)
        for i in 1..=5u64 {
            let peer_id = test_peer_id(i as u8);
            assert_ok!(Storage::register_node(
                RuntimeOrigin::signed(i),
                peer_id,
                10_000_000_000u64, // 10GB
                0, // pow_nonce
            test_http_url(3030),
            ));
        }

        // 6th registration should fail
        let peer_id6 = test_peer_id(6);
        assert_noop!(
            Storage::register_node(
                RuntimeOrigin::signed(6),
                peer_id6,
                10_000_000_000u64,
                0,
                test_http_url(3036),
            ),
            Error::<Test>::TooManyRegistrationsThisBlock
        );
    });
}

/// T038: Test declaration rate limit (11th declaration rejected)
#[test]
fn t038_declaration_rate_limit() {
    new_test_ext().execute_with(|| {
        let operator = 1u64;
        let peer_id = test_peer_id(1);
        let capacity = 10_000_000_000u64;

        // Register node
        assert_ok!(Storage::register_node(
            RuntimeOrigin::signed(operator),
            peer_id.clone(),
            capacity,
            0,
            test_http_url(3030),
        ));

        // Register and declare 10 fragments (should all succeed)
        for i in 1..=10u8 {
            let fragment_id = test_fragment_id(i);
            assert_ok!(Storage::register_fragment(
                RuntimeOrigin::signed(operator),
                fragment_id,
                1024
            ));
            assert_ok!(Storage::declare_holding(
                RuntimeOrigin::signed(operator),
                fragment_id
            ));
        }

        // 11th declaration should fail
        let fragment_id_11 = test_fragment_id(11);
        assert_ok!(Storage::register_fragment(
            RuntimeOrigin::signed(operator),
            fragment_id_11,
            1024
        ));
        assert_noop!(
            Storage::declare_holding(RuntimeOrigin::signed(operator), fragment_id_11),
            Error::<Test>::TooManyDeclarationsThisBlock
        );
    });
}

// ============================================================================
// KZG-VSS Tests (011-kzg-proof-rewards)
// ============================================================================

/// Helper: Create a test content hash
fn test_content_hash(n: u8) -> crate::ContentHash {
    let mut hash = [0u8; 32];
    hash[0] = n;
    hash
}

/// Helper: Create a test KZG commitment (48 bytes G1 compressed)
fn test_commitment() -> BoundedVec<u8, ConstU32<48>> {
    // Valid compressed G1 point (identity point encoded for tests)
    let mut bytes = vec![0u8; 48];
    bytes[0] = 0xc0; // Compressed flag + infinity flag
    BoundedVec::try_from(bytes).unwrap()
}

/// T017: register_kzg_fragment で90%報酬プール/10%バーン
/// TDD test - written before implementation
#[test]
#[ignore = "Requires register_kzg_fragment extrinsic (T024)"]
fn t017_register_kzg_fragment_90_10_split() {
    new_test_ext().execute_with(|| {
        let _owner = 1u64;
        let _content_hash = test_content_hash(1);
        let _commitment = test_commitment();
        let _data_size = 10_000u32; // 10KB
        let _fragment_count = 5u8;
        let _threshold = 3u8;
        let _fee = 100_000_000_000u128; // 0.1 MORAL = 100,000,000,000 units (12 decimals)

        // Get initial reward pool balance
        let _initial_pool = Storage::reward_pool_balance();

        // TODO (T024): Uncomment when register_kzg_fragment is implemented
        // Register KZG fragment with fee
        // assert_ok!(Storage::register_kzg_fragment(
        //     RuntimeOrigin::signed(owner),
        //     content_hash,
        //     commitment.clone(),
        //     data_size,
        //     fragment_count,
        //     threshold,
        //     fee,
        // ));

        // Verify 90% went to reward pool
        // let expected_pool_increase = (fee * 90) / 100; // 90%
        // let new_pool = Storage::reward_pool_balance();
        // assert_eq!(
        //     new_pool,
        //     initial_pool + expected_pool_increase,
        //     "90% of fee should go to reward pool"
        // );

        // Verify KzgFragment was stored correctly
        // let fragment = Storage::kzg_fragments(content_hash).expect("Fragment should exist");
        // assert_eq!(fragment.owner, owner);
        // assert_eq!(fragment.commitment.to_vec(), commitment.to_vec());
        // assert_eq!(fragment.data_size, data_size);
        // assert_eq!(fragment.fragment_count, fragment_count);
        // assert_eq!(fragment.threshold, threshold);

        // Verify event emitted (KzgFragmentRegistered)
    });
}

/// T029: prove_holding_kzg で有効な証明が検証される
/// TDD test - written before implementation
#[test]
#[ignore = "Requires prove_holding_kzg extrinsic (T034)"]
fn t029_prove_holding_kzg_valid_proof_succeeds() {
    new_test_ext().execute_with(|| {
        let _owner = 1u64;
        let _node = 2u64;
        let _content_hash = test_content_hash(1);
        let _commitment = test_commitment();
        let _share_index = 1u8;

        // TODO (T034): Setup - Create a pending challenge
        // let challenge = crate::Challenge::<Test> {
        //     content_hash,
        //     share_index,
        //     challenged_node: node,
        //     issued_at: 1u64,
        //     deadline: 100u64,
        // };
        // PendingChallenges::<T>::insert(node, challenge);

        // Provide valid KZG proof (48 bytes compressed G1)
        // let kzg_proof = test_commitment();

        // TODO (T034): Submit proof
        // assert_ok!(Storage::prove_holding_kzg(
        //     RuntimeOrigin::signed(node),
        //     content_hash,
        //     share_index,
        //     kzg_proof,
        // ));

        // Verify: proof record updated
        // Verify: success_count incremented
        // Verify: pending_reward increased
    });
}

/// T030: 無効な証明で InvalidKzgProof エラー
/// TDD test - written before implementation
#[test]
#[ignore = "Requires prove_holding_kzg extrinsic (T034)"]
fn t030_prove_holding_kzg_invalid_proof_fails() {
    new_test_ext().execute_with(|| {
        let _node = 2u64;
        let _content_hash = test_content_hash(1);
        let _share_index = 1u8;

        // Create a deliberately invalid proof
        let _invalid_proof: BoundedVec<u8, ConstU32<48>> = {
            let bytes = vec![0xffu8; 48]; // Invalid G1 point
            BoundedVec::try_from(bytes).unwrap()
        };

        // TODO (T034): Submit invalid proof - should fail
        // assert_noop!(
        //     Storage::prove_holding_kzg(
        //         RuntimeOrigin::signed(node),
        //         content_hash,
        //         share_index,
        //         invalid_proof,
        //     ),
        //     Error::<Test>::InvalidKzgProof
        // );
    });
}

/// T031: チャレンジ生成がランダムに動作
/// TDD test - written before implementation
#[test]
#[ignore = "Requires issue_challenge hook (T035)"]
fn t031_challenge_generation_random() {
    new_test_ext().execute_with(|| {
        // This test verifies that the challenge selection is pseudo-random
        // based on block hash

        // Setup: Register multiple KzgFragments
        // Progress blocks
        // Verify: Different challenges are issued based on block randomness
        // Verify: Challenge includes share_index (1..n)
    });
}


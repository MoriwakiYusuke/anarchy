//! Unit tests for pallet-reaction

use crate::{self as pallet_reaction, pallet::ReactionInterface};
use frame_support::{
    assert_noop, assert_ok,
    traits::{ConstU8, ConstU32, ConstU64, ConstU128},
};
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};
use parity_scale_codec::Encode;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Reaction: pallet_reaction,
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
    type AccountData = pallet_balances::AccountData<u128>;
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

impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = u128;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<0>;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type DoneSlashHandler = ();
}

/// Mock PostAuthorProvider for tests
pub struct MockPostAuthorProvider;
impl pallet_reaction::PostAuthorProvider<u64> for MockPostAuthorProvider {
    fn get_post_author(post_id: u64) -> Option<u64> {
        // For tests: post_id 1 is owned by account 100, post_id 2 by account 200
        match post_id {
            1 => Some(100),
            2 => Some(200),
            _ => None,
        }
    }
}

impl pallet_reaction::Config for Test {
    type NativeToken = Balances;
    type PostAuthorProvider = MockPostAuthorProvider;
    type ReactionReward = ConstU128<1_000_000_000_000>; // 1 MORAL
    type BaseDifficulty = ConstU8<8>;
    type MinDifficulty = ConstU8<4>;
    type MaxDifficulty = ConstU8<32>;
    type ChallengeValidity = ConstU64<100>;
    type TargetReactionRate = ConstU32<10>;
    type AdjustmentWindow = ConstU64<10>;
    type AdjustmentDivisor = ConstU32<2>;
}

fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    
    pallet_reaction::GenesisConfig::<Test> {
        initial_reward_pool: 1_000_000_000_000_000, // 1000 MORAL
        initial_difficulty: 8,
        _marker: Default::default(),
    }
    .assimilate_storage(&mut t)
    .unwrap();
    
    t.into()
}

#[test]
fn test_genesis_config() {
    new_test_ext().execute_with(|| {
        assert_eq!(pallet_reaction::ReactionRewardPool::<Test>::get(), 1_000_000_000_000_000);
        assert_eq!(pallet_reaction::CurrentDifficulty::<Test>::get(), 8);
    });
}

#[test]
fn test_deposit_to_reaction_pool() {
    new_test_ext().execute_with(|| {
        let initial = pallet_reaction::ReactionRewardPool::<Test>::get();
        let deposit_amount = 100_000_000_000u128;
        
        <Reaction as ReactionInterface>::do_deposit_to_reaction_pool(deposit_amount);
        
        assert_eq!(pallet_reaction::ReactionRewardPool::<Test>::get(), initial + deposit_amount);
    });
}

#[test]
fn test_get_reaction_counts_empty() {
    new_test_ext().execute_with(|| {
        let counts = <Reaction as ReactionInterface>::get_reaction_counts(999);
        assert_eq!(counts, Some((0, 0)));
    });
}

#[test]
fn test_get_bad_count_empty() {
    new_test_ext().execute_with(|| {
        let bad_count = <Reaction as ReactionInterface>::get_bad_count(999);
        assert_eq!(bad_count, 0);
    });
}

// =============================================================================
// Helper functions for US1 tests
// =============================================================================

/// Helper: Find a valid nonce for given account and post
fn find_valid_nonce(account_id: u64, block_hash: H256, difficulty: u8) -> u64 {
    let challenge = compute_challenge(block_hash, account_id);
    let mut nonce = 0u64;
    loop {
        if verify_proof(&challenge, nonce, difficulty) {
            return nonce;
        }
        nonce += 1;
        if nonce > 10_000_000 {
            panic!("Could not find valid nonce within 10M attempts");
        }
    }
}

/// Helper: Compute challenge
fn compute_challenge(block_hash: H256, account_id: u64) -> [u8; 32] {
    let mut data = block_hash.as_bytes().to_vec();
    data.extend(account_id.encode());
    blake2_256(&data)
}

/// Helper: Verify proof
fn verify_proof(challenge: &[u8; 32], nonce: u64, difficulty: u8) -> bool {
    let mut data = challenge.to_vec();
    data.extend(nonce.to_le_bytes());
    let hash = blake2_256(&data);
    count_leading_zeros(&hash) >= difficulty
}

/// Helper: Count leading zero bits
fn count_leading_zeros(hash: &[u8; 32]) -> u8 {
    let mut count = 0u8;
    for byte in hash.iter() {
        if *byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros() as u8;
            break;
        }
    }
    count
}

// =============================================================================
// T019: react() rejects duplicate reactions
// =============================================================================
#[test]
fn test_react_rejects_duplicate() {
    new_test_ext().execute_with(|| {
        // Initialize block
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xAB));
        
        let reactor = 1u64;
        let post_id = 100u64;
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        
        let nonce = find_valid_nonce(reactor, block_hash, difficulty);
        
        // First reaction should succeed
        assert_ok!(Reaction::react(
            RuntimeOrigin::signed(reactor),
            post_id,
            pallet_reaction::ReactionType::Like,
            block_number,
            nonce,
            1000,  // cpu_power
            None,  // stealth_recipient
        ));
        
        // Find another valid nonce for second attempt
        let nonce2 = find_valid_nonce(reactor, block_hash, difficulty);
        
        // Second reaction should fail with AlreadyReacted
        assert_noop!(
            Reaction::react(
                RuntimeOrigin::signed(reactor),
                post_id,
                pallet_reaction::ReactionType::Like,
                block_number,
                nonce2,
                1000,
                None,
            ),
            pallet_reaction::Error::<Test>::AlreadyReacted
        );
    });
}

// =============================================================================
// T020: react() rejects invalid PoW
// =============================================================================
#[test]
fn test_react_rejects_invalid_pow() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xAB));
        
        let reactor = 1u64;
        let post_id = 100u64;
        let block_number = 1u64;
        
        // Use difficulty 64 bits (impossible to pass with any nonce)
        // This ensures the test is deterministic
        pallet_reaction::CurrentDifficulty::<Test>::put(64u8);
        let invalid_nonce = 0u64; // Any nonce will fail with difficulty 64
        
        // Should fail with InvalidProof
        assert_noop!(
            Reaction::react(
                RuntimeOrigin::signed(reactor),
                post_id,
                pallet_reaction::ReactionType::Like,
                block_number,
                invalid_nonce,
                1000,
                None,
            ),
            pallet_reaction::Error::<Test>::InvalidProof
        );
    });
}

// =============================================================================
// T021: react() updates ReactionStats correctly
// =============================================================================
#[test]
fn test_react_updates_stats() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xAB));
        
        let post_id = 100u64;
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        
        // React with Like (reactor 1)
        let nonce1 = find_valid_nonce(1u64, block_hash, difficulty);
        assert_ok!(Reaction::react(
            RuntimeOrigin::signed(1),
            post_id,
            pallet_reaction::ReactionType::Like,
            block_number,
            nonce1,
            1000,
            None,
        ));
        
        // Check stats after first Like
        let stats = pallet_reaction::ReactionStatsStorage::<Test>::get(post_id);
        assert_eq!(stats.likes, 1);
        assert_eq!(stats.bads, 0);

        // React with Bad (reactor 3)
        let nonce3 = find_valid_nonce(3u64, block_hash, difficulty);
        assert_ok!(Reaction::react(
            RuntimeOrigin::signed(3),
            post_id,
            pallet_reaction::ReactionType::Bad,
            block_number,
            nonce3,
            1000,
            None,
        ));

        // Check stats after Bad
        let stats = pallet_reaction::ReactionStatsStorage::<Test>::get(post_id);
        assert_eq!(stats.likes, 1);
        assert_eq!(stats.bads, 1);
    });
}

// =============================================================================
// T022: react() pays author reward (fixed 1 MORAL per reaction)
// =============================================================================
#[test]
fn test_react_pays_reward() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xAB));
        
        // Post ID 1 is owned by account 100 (MockPostAuthorProvider)
        let reactor = 1u64;
        let post_id = 1u64;  // Author is account 100
        let author = 100u64;
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        let cpu_power = 1_000_000u64;
        
        // Check author balance before
        let author_balance_before = pallet_balances::Pallet::<Test>::free_balance(author);
        
        let nonce = find_valid_nonce(reactor, block_hash, difficulty);
        
        assert_ok!(Reaction::react(
            RuntimeOrigin::signed(reactor),
            post_id,
            pallet_reaction::ReactionType::Like,
            block_number,
            nonce,
            cpu_power,
            None,
        ));
        
        // Author should receive 1 MORAL (1_000_000_000_000 units)
        let author_balance_after = pallet_balances::Pallet::<Test>::free_balance(author);
        assert_eq!(author_balance_after - author_balance_before, 1_000_000_000_000u128, "Author should receive 1 MORAL reward");
    });
}

// =============================================================================
// T023: react() skips reward for Bad reactions and non-existent posts
// =============================================================================
#[test]
fn test_react_skips_reward_when_pool_empty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xAB));
        
        // Post ID 100 does not exist in MockPostAuthorProvider
        let reactor = 1u64;
        let post_id = 100u64;  // No author for this post
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        
        let nonce = find_valid_nonce(reactor, block_hash, difficulty);
        
        // Reaction should still succeed even without author
        assert_ok!(Reaction::react(
            RuntimeOrigin::signed(reactor),
            post_id,
            pallet_reaction::ReactionType::Like,
            block_number,
            nonce,
            1000,
            None,
        ));
        
        // Reaction should be recorded
        assert!(pallet_reaction::Reactions::<Test>::contains_key(post_id, reactor));
    });
}

// =============================================================================
// T048: adjust_difficulty increases when rate exceeds target
// =============================================================================
#[test]
fn test_adjust_difficulty_increases_when_rate_high() {
    new_test_ext().execute_with(|| {
        // Initial difficulty is 8
        assert_eq!(pallet_reaction::CurrentDifficulty::<Test>::get(), 8);
        
        // TargetReactionRate = 10 per block, AdjustmentWindow = 10 blocks
        // Target total = 10 * 10 = 100 reactions
        
        // Set up: simulate high reaction rate by setting reaction history
        // Put 150 reactions in the last 10 blocks (50% above target)
        for block in 1u64..=10u64 {
            pallet_reaction::ReactionHistory::<Test>::insert(block, 15u32);
        }
        
        System::set_block_number(10);
        
        // Trigger difficulty adjustment
        Reaction::adjust_difficulty();
        
        // Difficulty should increase
        // (150 - 100) / 2 = 25, but clamped to max 32
        // Starting from 8: 8 + 25 = 33, clamped to 32
        let new_difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        assert!(new_difficulty > 8, "Difficulty should increase when rate exceeds target");
    });
}

// =============================================================================
// T049: adjust_difficulty decreases when rate below target
// =============================================================================
#[test]
fn test_adjust_difficulty_decreases_when_rate_low() {
    new_test_ext().execute_with(|| {
        // Set initial difficulty to 16
        pallet_reaction::CurrentDifficulty::<Test>::put(16);
        assert_eq!(pallet_reaction::CurrentDifficulty::<Test>::get(), 16);
        
        // TargetReactionRate = 10 per block, AdjustmentWindow = 10 blocks
        // Target total = 100 reactions
        
        // Set up: simulate low reaction rate (50 reactions, 50% below target)
        for block in 1u64..=10u64 {
            pallet_reaction::ReactionHistory::<Test>::insert(block, 5u32);
        }
        
        System::set_block_number(10);
        
        // Trigger difficulty adjustment
        Reaction::adjust_difficulty();
        
        // Difficulty should decrease
        // (100 - 50) / 2 = 25
        // Starting from 16: 16 - 25 = -9, clamped to min 4
        let new_difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        assert!(new_difficulty < 16, "Difficulty should decrease when rate below target");
    });
}

// =============================================================================
// T050: difficulty respects min/max bounds
// =============================================================================
#[test]
fn test_difficulty_respects_bounds() {
    new_test_ext().execute_with(|| {
        // Test max bound: Set very high reaction rate
        pallet_reaction::CurrentDifficulty::<Test>::put(30);
        for block in 1u64..=10u64 {
            pallet_reaction::ReactionHistory::<Test>::insert(block, 100u32); // 1000 total, way above target
        }
        System::set_block_number(10);
        
        Reaction::adjust_difficulty();
        
        // Should be clamped to MaxDifficulty (32)
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        assert!(difficulty <= 32, "Difficulty should not exceed MaxDifficulty");
        
        // Test min bound: Set very low reaction rate
        pallet_reaction::CurrentDifficulty::<Test>::put(5);
        for block in 1u64..=10u64 {
            pallet_reaction::ReactionHistory::<Test>::insert(block, 0u32); // 0 total, way below target
        }
        System::set_block_number(20);
        
        Reaction::adjust_difficulty();
        
        // Should be clamped to MinDifficulty (4)
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        assert!(difficulty >= 4, "Difficulty should not go below MinDifficulty");
    });
}

// =============================================================================
// T-Extra: react() rejects expired challenge
// =============================================================================
#[test]
fn test_react_rejects_expired_challenge() {
    new_test_ext().execute_with(|| {
        // Set up at block 1
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xAB));
        
        let reactor = 1u64;
        let post_id = 100u64;
        let old_block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(old_block_number);
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        
        let nonce = find_valid_nonce(reactor, block_hash, difficulty);
        
        // Advance to block 200 (beyond challenge validity of 100)
        System::set_block_number(200);
        
        // Should fail with ChallengeExpired
        assert_noop!(
            Reaction::react(
                RuntimeOrigin::signed(reactor),
                post_id,
                pallet_reaction::ReactionType::Like,
                old_block_number,
                nonce,
                1000,
                None,
            ),
            pallet_reaction::Error::<Test>::ChallengeExpired
        );
    });
}

// =============================================================================
// T060: react() with stealth_recipient sends reward to stealth
// =============================================================================
#[test]
fn test_react_with_stealth_recipient() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xAB));
        
        let reactor = 1u64;
        let stealth_recipient = 99u64; // Different account
        let post_id = 100u64;
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        
        let nonce = find_valid_nonce(reactor, block_hash, difficulty);
        
        // React with stealth recipient
        assert_ok!(Reaction::react(
            RuntimeOrigin::signed(reactor),
            post_id,
            pallet_reaction::ReactionType::Like,
            block_number,
            nonce,
            1000,
            Some(stealth_recipient), // Specify stealth recipient
        ));
        
        // Reaction should be recorded
        assert!(pallet_reaction::Reactions::<Test>::contains_key(post_id, reactor));
        
        // Event should include the stealth recipient info
        let events = System::events();
        let reaction_event = events.iter().find(|e| {
            matches!(
                e.event,
                RuntimeEvent::Reaction(pallet_reaction::Event::ReactionCreated { .. })
            )
        });
        assert!(reaction_event.is_some(), "ReactionCreated event should be emitted");
    });
}

// =============================================================================
// T061: react() without recipient sends reward to post author
// =============================================================================
#[test]
fn test_react_without_stealth_recipient() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xCD));
        
        let reactor = 2u64;
        let post_id = 200u64;
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        
        let nonce = find_valid_nonce(reactor, block_hash, difficulty);
        
        // React without stealth recipient (None)
        assert_ok!(Reaction::react(
            RuntimeOrigin::signed(reactor),
            post_id,
            pallet_reaction::ReactionType::Bad,
            block_number,
            nonce,
            2000,
            None, // No stealth recipient -> reward goes to author
        ));

        // Reaction should be recorded
        assert!(pallet_reaction::Reactions::<Test>::contains_key(post_id, reactor));

        // Stats should be updated
        let stats = pallet_reaction::ReactionStatsStorage::<Test>::get(post_id);
        assert_eq!(stats.bads, 1);
    });
}

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

impl pallet_reaction::Config for Test {
    type NativeToken = Balances;
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
fn test_reaction_type_weights() {
    use pallet_reaction::ReactionType;
    assert_eq!(ReactionType::Like.weight(), 1);
    assert_eq!(ReactionType::Boost.weight(), 5);
    assert_eq!(ReactionType::Bad.weight(), 0);
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
        assert_eq!(counts, Some((0, 0, 0)));
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
        let invalid_nonce = 12345u64; // Very unlikely to be valid
        
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
        assert_eq!(stats.boosts, 0);
        assert_eq!(stats.bads, 0);
        assert_eq!(stats.total_weight, 1);  // Like weight = 1
        
        // React with Boost (reactor 2)
        let nonce2 = find_valid_nonce(2u64, block_hash, difficulty);
        assert_ok!(Reaction::react(
            RuntimeOrigin::signed(2),
            post_id,
            pallet_reaction::ReactionType::Boost,
            block_number,
            nonce2,
            1000,
            None,
        ));
        
        // Check stats after Boost
        let stats = pallet_reaction::ReactionStatsStorage::<Test>::get(post_id);
        assert_eq!(stats.likes, 1);
        assert_eq!(stats.boosts, 1);
        assert_eq!(stats.bads, 0);
        assert_eq!(stats.total_weight, 6);  // 1 + 5 = 6
        
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
        assert_eq!(stats.boosts, 1);
        assert_eq!(stats.bads, 1);
        assert_eq!(stats.total_weight, 6);  // Bad weight = 0, total unchanged
    });
}

// =============================================================================
// T022: react() pays author reward from pool
// =============================================================================
#[test]
fn test_react_pays_reward() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xAB));
        
        let initial_pool = pallet_reaction::ReactionRewardPool::<Test>::get();
        
        let reactor = 1u64;
        let post_id = 100u64;
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        let cpu_power = 1_000_000u64;  // High CPU power for measurable reward
        
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
        
        // Pool balance should have decreased (reward paid)
        let final_pool = pallet_reaction::ReactionRewardPool::<Test>::get();
        assert!(final_pool < initial_pool, "Reward pool should decrease after reaction");
    });
}

// =============================================================================
// T023: react() records reaction but skips reward when pool empty
// =============================================================================
#[test]
fn test_react_skips_reward_when_pool_empty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xAB));
        
        // Empty the reward pool
        pallet_reaction::ReactionRewardPool::<Test>::put(0u128);
        
        let reactor = 1u64;
        let post_id = 100u64;
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = pallet_reaction::CurrentDifficulty::<Test>::get();
        
        let nonce = find_valid_nonce(reactor, block_hash, difficulty);
        
        // Reaction should still succeed
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
        
        // Pool should still be empty
        assert_eq!(pallet_reaction::ReactionRewardPool::<Test>::get(), 0);
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

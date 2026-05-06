//! Faucet Pallet Tests
//!
//! TDD: Tests written first based on spec.md requirements
//! T-001 to T-008 cover all functional requirements

use crate::{self as pallet_faucet, Error, Event, FaucetClaims, TotalClaims};
use frame_support::{
    assert_noop, assert_ok,
    traits::{ConstU128, ConstU32, ConstU64, ConstU8},
};
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

// Test runtime construction
frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Faucet: pallet_faucet,
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
    type AccountData = pallet_balances::AccountData<u128>;
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

impl pallet_balances::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Balance = u128;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type FreezeIdentifier = ();
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type MaxFreezes = ConstU32<0>;
    type DoneSlashHandler = ();
}

// Faucet constants for testing
// BaseDifficulty = 8 (easy for tests)
// ScalingFactor = 10 (quick difficulty increase for testing)
// MaxDifficulty = 16
// RewardAmount = 100_000_000_000_000 (100 MORAL)
// ChallengeValidity = 100 blocks
impl pallet_faucet::Config for Test {
    type NativeToken = Balances;
    type BaseDifficulty = ConstU8<8>;
    type DifficultyScalingFactor = ConstU64<10>;
    type MaxDifficulty = ConstU8<16>;
    type RewardAmount = ConstU128<100_000_000_000_000>;
    type ChallengeValidity = ConstU64<100>;
    /// Tests: 0 = cap 無効 (旧挙動互換)。capped tests は専用 mock を使う。
    type TotalCap = ConstU128<0>;
}

/// Build test externalities
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        // Initialize at block 1 (block 0 has no hash)
        System::set_block_number(1);
        // Set a deterministic block hash for block 1
        frame_system::BlockHash::<Test>::insert(1, H256::repeat_byte(0xAB));
    });
    ext
}

/// Helper: Find a valid nonce for given account and block
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

/// Helper: Compute challenge (same as pallet logic)
fn compute_challenge(block_hash: H256, account_id: u64) -> [u8; 32] {
    use parity_scale_codec::Encode;
    let mut data = block_hash.as_bytes().to_vec();
    data.extend(account_id.encode());
    blake2_256(&data)
}

/// Helper: Verify proof (same as pallet logic)
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
// T-001: 正しいPoW解でclaimが成功し残高増加
// =============================================================================
#[test]
fn test_claim_success() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = Faucet::calculate_difficulty();

        // Find a valid nonce
        let nonce = find_valid_nonce(account, block_hash, difficulty);

        // Verify initial state
        assert_eq!(Balances::free_balance(account), 0);
        assert!(FaucetClaims::<Test>::get(account).is_none());
        assert_eq!(TotalClaims::<Test>::get(), 0);

        // Execute claim (unsigned extrinsic)
        assert_ok!(Faucet::claim(RuntimeOrigin::none(), account, block_number, nonce));

        // Verify post-state
        assert_eq!(Balances::free_balance(account), 100_000_000_000_000u128); // 100 MORAL
        assert!(FaucetClaims::<Test>::get(account).is_some());
        assert_eq!(TotalClaims::<Test>::get(), 1);

        // Verify event
        System::assert_has_event(RuntimeEvent::Faucet(Event::FaucetClaimed {
            who: account,
            amount: 100_000_000_000_000u128,
            block_number,
        }));
    });
}

// =============================================================================
// T-002: AlreadyClaimed - 同一アカウントで2回目のclaimは拒否される
// =============================================================================
#[test]
fn test_already_claimed() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = Faucet::calculate_difficulty();
        let nonce = find_valid_nonce(account, block_hash, difficulty);

        // First claim succeeds
        assert_ok!(Faucet::claim(RuntimeOrigin::none(), account, block_number, nonce));

        // Second claim fails with AlreadyClaimed
        assert_noop!(
            Faucet::claim(RuntimeOrigin::none(), account, block_number, nonce),
            Error::<Test>::AlreadyClaimed
        );
    });
}

// =============================================================================
// T-003: ChallengeExpired - 期限切れブロック番号で拒否
// =============================================================================
#[test]
fn test_challenge_expired() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let old_block = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(old_block);
        let difficulty = Faucet::calculate_difficulty();
        let nonce = find_valid_nonce(account, block_hash, difficulty);

        // Fast forward beyond validity period (100 blocks)
        System::set_block_number(102);

        // Claim should fail with ChallengeExpired
        assert_noop!(
            Faucet::claim(RuntimeOrigin::none(), account, old_block, nonce),
            Error::<Test>::ChallengeExpired
        );
    });
}

// =============================================================================
// T-004: InvalidProof - 難易度を満たさないnonceは拒否
// =============================================================================
#[test]
fn test_invalid_proof() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let block_number = 1u64;
        let invalid_nonce = 12345u64; // Unlikely to be valid

        // Claim should fail with InvalidProof
        assert_noop!(
            Faucet::claim(RuntimeOrigin::none(), account, block_number, invalid_nonce),
            Error::<Test>::InvalidProof
        );
    });
}

// =============================================================================
// T-005: BlockNotFound - 存在しないブロック番号で拒否
// =============================================================================
#[test]
fn test_block_not_found() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let future_block = 999u64; // Block that doesn't exist yet
        let nonce = 0u64;

        // Claim should fail with BlockNotFound
        assert_noop!(
            Faucet::claim(RuntimeOrigin::none(), account, future_block, nonce),
            Error::<Test>::BlockNotFound
        );
    });
}

// =============================================================================
// T-006: 動的難易度 - TotalClaimsに応じて難易度が正しく計算される
// =============================================================================
#[test]
fn test_dynamic_difficulty() {
    new_test_ext().execute_with(|| {
        // BaseDifficulty = 8, ScalingFactor = 10
        // Formula: base + floor(log2(1 + claims/factor))

        // Initial: 0 claims -> difficulty = 8 + floor(log2(1)) = 8 + 0 = 8
        assert_eq!(Faucet::calculate_difficulty(), 8);

        // After 10 claims: difficulty = 8 + floor(log2(2)) = 8 + 1 = 9
        TotalClaims::<Test>::put(10);
        assert_eq!(Faucet::calculate_difficulty(), 9);

        // After 30 claims: difficulty = 8 + floor(log2(4)) = 8 + 2 = 10
        TotalClaims::<Test>::put(30);
        assert_eq!(Faucet::calculate_difficulty(), 10);

        // After 70 claims: difficulty = 8 + floor(log2(8)) = 8 + 3 = 11
        TotalClaims::<Test>::put(70);
        assert_eq!(Faucet::calculate_difficulty(), 11);
    });
}

// =============================================================================
// T-007: 難易度上限 - max_difficultyを超えないことを確認
// =============================================================================
#[test]
fn test_max_difficulty() {
    new_test_ext().execute_with(|| {
        // Set extremely high claims to test max cap
        // MaxDifficulty = 16
        TotalClaims::<Test>::put(1_000_000_000);

        // Should not exceed MaxDifficulty
        assert_eq!(Faucet::calculate_difficulty(), 16);
    });
}

// =============================================================================
// T-008: TotalClaimsカウンタ - claim成功時に+1される
// =============================================================================
#[test]
fn test_total_claims_counter() {
    new_test_ext().execute_with(|| {
        let block_number = 1u64;
        let block_hash = frame_system::BlockHash::<Test>::get(block_number);
        let difficulty = Faucet::calculate_difficulty();

        // Initial count = 0
        assert_eq!(TotalClaims::<Test>::get(), 0);

        // First claim
        let nonce1 = find_valid_nonce(1u64, block_hash, difficulty);
        assert_ok!(Faucet::claim(RuntimeOrigin::none(), 1u64, block_number, nonce1));
        assert_eq!(TotalClaims::<Test>::get(), 1);

        // Second claim (different account)
        let nonce2 = find_valid_nonce(2u64, block_hash, difficulty);
        assert_ok!(Faucet::claim(RuntimeOrigin::none(), 2u64, block_number, nonce2));
        assert_eq!(TotalClaims::<Test>::get(), 2);

        // Third claim (different account)
        let nonce3 = find_valid_nonce(3u64, block_hash, difficulty);
        assert_ok!(Faucet::claim(RuntimeOrigin::none(), 3u64, block_number, nonce3));
        assert_eq!(TotalClaims::<Test>::get(), 3);
    });
}

// =============================================================================
// Additional: Verify leading zeros counting
// =============================================================================
#[test]
fn test_count_leading_zeros() {
    // Test with known values
    let hash_0_zeros: [u8; 32] = [0xFF; 32];
    assert_eq!(count_leading_zeros(&hash_0_zeros), 0);

    let mut hash_8_zeros: [u8; 32] = [0xFF; 32];
    hash_8_zeros[0] = 0x00;
    assert_eq!(count_leading_zeros(&hash_8_zeros), 8);

    let mut hash_9_zeros: [u8; 32] = [0xFF; 32];
    hash_9_zeros[0] = 0x00;
    hash_9_zeros[1] = 0x7F; // 0111 1111 -> 1 leading zero
    assert_eq!(count_leading_zeros(&hash_9_zeros), 9); // First byte=0 (8 zeros), second=0x7F (1 zero)

    let mut hash_16_zeros: [u8; 32] = [0xFF; 32];
    hash_16_zeros[0] = 0x00;
    hash_16_zeros[1] = 0x00;
    assert_eq!(count_leading_zeros(&hash_16_zeros), 16);
}

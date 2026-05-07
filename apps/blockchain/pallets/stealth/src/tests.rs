//! Unit tests for the stealth pallet

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};

/// T037: send_to_stealth works correctly
#[test]
fn send_to_stealth_works() {
    new_test_ext().execute_with(|| {
        let ephemeral_pubkey = [1u8; 32];
        let amount = 100_000_000_000_000u128; // 100 MORAL

        // Execute stealth transfer
        assert_ok!(StealthPallet::send_to_stealth(
            RuntimeOrigin::signed(ALICE),
            STEALTH_ADDR,
            ephemeral_pubkey,
            amount,
        ));

        // Check balance updated
        assert_eq!(Balances::free_balance(STEALTH_ADDR), amount);
        assert_eq!(
            Balances::free_balance(ALICE),
            1_000_000_000_000_000 - amount
        );

        // Check event emitted
        System::assert_last_event(
            Event::<Test>::StealthTransfer {
                sender: ALICE,
                stealth_address: STEALTH_ADDR,
                amount,
            }
            .into(),
        );
    });
}

/// T038: send_to_stealth fails with zero amount
#[test]
fn send_to_stealth_fails_with_zero_amount() {
    new_test_ext().execute_with(|| {
        let ephemeral_pubkey = [1u8; 32];

        assert_noop!(
            StealthPallet::send_to_stealth(
                RuntimeOrigin::signed(ALICE),
                STEALTH_ADDR,
                ephemeral_pubkey,
                0,
            ),
            Error::<Test>::ZeroAmount
        );
    });
}

/// T039: ephemeral keys recorded correctly
#[test]
fn ephemeral_keys_recorded_correctly() {
    new_test_ext().execute_with(|| {
        let ephemeral_pubkey_1 = [1u8; 32];
        let ephemeral_pubkey_2 = [2u8; 32];
        let amount = 50_000_000_000_000u128; // 50 MORAL

        // First transfer
        assert_ok!(StealthPallet::send_to_stealth(
            RuntimeOrigin::signed(ALICE),
            STEALTH_ADDR,
            ephemeral_pubkey_1,
            amount,
        ));

        // Second transfer to different address
        let stealth_addr_2 = 101u64;
        assert_ok!(StealthPallet::send_to_stealth(
            RuntimeOrigin::signed(BOB),
            stealth_addr_2,
            ephemeral_pubkey_2,
            amount,
        ));

        // Verify ephemeral keys stored
        let block_number = System::block_number();
        let entries = StealthPallet::ephemeral_keys(block_number);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].ephemeral_pubkey, ephemeral_pubkey_1);
        assert_eq!(entries[0].stealth_address, STEALTH_ADDR);
        assert_eq!(entries[1].ephemeral_pubkey, ephemeral_pubkey_2);
        assert_eq!(entries[1].stealth_address, stealth_addr_2);
    });
}

/// Test insufficient balance
#[test]
fn send_to_stealth_fails_with_insufficient_balance() {
    new_test_ext().execute_with(|| {
        let ephemeral_pubkey = [1u8; 32];
        // Try to send more than BOB has
        let amount = 999_999_999_999_999u128;

        assert_noop!(
            StealthPallet::send_to_stealth(
                RuntimeOrigin::signed(BOB),
                STEALTH_ADDR,
                ephemeral_pubkey,
                amount,
            ),
            Error::<Test>::InsufficientBalance
        );
    });
}

// ─── TSTS F2: claim_stealth_reward extrinsic + compute_claim_amount ─────────────

#[test]
fn compute_claim_amount_proportional_to_unclaimed() {
    // unclaimed=10 / total=100 → 10% × pool=1000 = 100. cap=0 (無効)
    let r = StealthPallet::compute_claim_amount(10, 100, 1000, 0);
    assert_eq!(r, 100);
}

#[test]
fn compute_claim_amount_returns_zero_when_no_unclaimed() {
    let r = StealthPallet::compute_claim_amount(0, 100, 1000, 0);
    assert_eq!(r, 0);
}

#[test]
fn compute_claim_amount_returns_zero_when_pool_empty() {
    let r = StealthPallet::compute_claim_amount(10, 100, 0, 0);
    assert_eq!(r, 0);
}

#[test]
fn compute_claim_amount_capped_by_per_claim_cap() {
    // unclaimed=50 / total=100 = 50%, pool=1000 → 500. cap=10% (= 100_000 ppm) → 100
    let r = StealthPallet::compute_claim_amount(50, 100, 1000, 100_000);
    assert_eq!(r, 100);
}

#[test]
fn claim_stealth_reward_pays_caller_proportionally() {
    new_test_ext().execute_with(|| {
        // pool に 1000 MORAL, ALICE が 5 通受信した想定で claim
        let eph = [9u8; 32];
        StealthPallet::deposit_to_reward_pool(1_000_000_000_000_000); // 1000 MORAL
        for _ in 0..5 {
            StealthPallet::record_recipient_receive(eph);
        }
        // total_received=5 のうち unclaimed=5 → 100% × pool × cap(10%) = pool × 10% = 100 MORAL
        let alice_initial = Balances::free_balance(ALICE);
        assert_ok!(StealthPallet::claim_stealth_reward(
            RuntimeOrigin::signed(ALICE),
            eph,
        ));
        let alice_final = Balances::free_balance(ALICE);
        assert_eq!(alice_final - alice_initial, 100_000_000_000_000); // 100 MORAL (cap で頭打ち)
        // pool 残高: 1000 - 100 = 900 MORAL
        assert_eq!(StealthPallet::stealth_reward_pool(), 900_000_000_000_000);
        // claimed_count が received_count と同期
        assert_eq!(StealthPallet::claimed_receive_count(eph), 5);
    });
}

#[test]
fn claim_stealth_reward_fails_without_unclaimed() {
    new_test_ext().execute_with(|| {
        let eph = [9u8; 32];
        StealthPallet::deposit_to_reward_pool(1_000_000_000_000);
        // 受信 0 のまま claim → NoUnclaimedReceives
        assert_noop!(
            StealthPallet::claim_stealth_reward(RuntimeOrigin::signed(ALICE), eph),
            Error::<Test>::NoUnclaimedReceives
        );
    });
}

#[test]
fn claim_stealth_reward_fails_when_pool_empty() {
    new_test_ext().execute_with(|| {
        let eph = [9u8; 32];
        StealthPallet::record_recipient_receive(eph);
        // pool に何も deposit してない → StealthRewardPoolEmpty
        assert_noop!(
            StealthPallet::claim_stealth_reward(RuntimeOrigin::signed(ALICE), eph),
            Error::<Test>::StealthRewardPoolEmpty
        );
    });
}

#[test]
fn double_claim_returns_no_unclaimed_receives() {
    new_test_ext().execute_with(|| {
        let eph = [9u8; 32];
        StealthPallet::deposit_to_reward_pool(1_000_000_000_000_000);
        for _ in 0..5 {
            StealthPallet::record_recipient_receive(eph);
        }
        // 1 回目: 成功
        assert_ok!(StealthPallet::claim_stealth_reward(RuntimeOrigin::signed(ALICE), eph));
        // 2 回目: 新規受信なし → unclaimed=0
        assert_noop!(
            StealthPallet::claim_stealth_reward(RuntimeOrigin::signed(ALICE), eph),
            Error::<Test>::NoUnclaimedReceives
        );
    });
}

//! Unit tests for the stealth pallet

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::TokenError;

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
            TokenError::FundsUnavailable
        );
    });
}

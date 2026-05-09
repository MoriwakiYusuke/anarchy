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

// ─── TSTS F2 / F2.5: claim_stealth_reward extrinsic + compute_claim_amount ─────

use parity_scale_codec::Encode;
use sp_core::{ed25519, Pair};

/// Test helper: ed25519 鍵ペアを seed から決定的に生成し、
/// `(signer, ephemeral_pubkey)` のメッセージに署名する。
fn ed25519_sign_for_claim(
    seed: u8,
    signer: u64,
    ephemeral_pubkey: [u8; 32],
) -> ([u8; 32], [u8; 64]) {
    let pair = ed25519::Pair::from_seed_slice(&[seed; 32])
        .expect("seed length 32 → ed25519 Pair");
    let stealth_pubkey: [u8; 32] = pair.public().into();
    let message = (signer, ephemeral_pubkey).encode();
    let signature: [u8; 64] = pair.sign(&message).into();
    (stealth_pubkey, signature)
}

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
fn claim_stealth_reward_pays_caller_with_valid_signature() {
    new_test_ext().execute_with(|| {
        let eph = [9u8; 32];
        let (stealth_pk, sig) = ed25519_sign_for_claim(7, ALICE, eph);

        StealthPallet::deposit_to_reward_pool(1_000_000_000_000_000); // 1000 MORAL
        for _ in 0..5 {
            StealthPallet::record_recipient_receive(eph);
        }
        let alice_initial = Balances::free_balance(ALICE);
        assert_ok!(StealthPallet::claim_stealth_reward(
            RuntimeOrigin::signed(ALICE),
            eph,
            stealth_pk,
            sig,
            Vec::new(),
        ));
        let alice_final = Balances::free_balance(ALICE);
        // cap 10% で 100 MORAL を mint
        assert_eq!(alice_final - alice_initial, 100_000_000_000_000);
        assert_eq!(StealthPallet::stealth_reward_pool(), 900_000_000_000_000);
        // F2 修正 (Copilot #3199031147): cap で partial claim の場合、
        // claimed_count は比例分のみ進む (5 × 100/1000 = 0.5 → max(1, floor) = 1).
        // 残り 4 回分は次の claim で取れる。
        assert_eq!(StealthPallet::claimed_receive_count(eph), 1);
    });
}

#[test]
fn capped_claim_can_resume_remainder_in_next_call() {
    new_test_ext().execute_with(|| {
        // F2 修正検証 (Copilot #3199031147): cap で truncate された場合、
        // 残り unclaimed が永久ロックされず次回 claim で取れる。
        let eph = [9u8; 32];
        let (stealth_pk, sig) = ed25519_sign_for_claim(7, ALICE, eph);

        StealthPallet::deposit_to_reward_pool(1_000_000_000_000_000); // 1000 MORAL
        for _ in 0..5 {
            StealthPallet::record_recipient_receive(eph);
        }

        // 1 回目: cap 10% で 100 MORAL、claimed_count 1 まで進む
        assert_ok!(StealthPallet::claim_stealth_reward(
            RuntimeOrigin::signed(ALICE),
            eph,
            stealth_pk,
            sig,
            Vec::new(),
        ));
        assert_eq!(StealthPallet::claimed_receive_count(eph), 1);
        let after_first = StealthPallet::stealth_reward_pool();
        // 1000 - 100 = 900
        assert_eq!(after_first, 900_000_000_000_000);

        // 2 回目: 残 unclaimed = 4, 残 pool = 900
        //   proportional_full = 4/5 × 900 = 720, cap 10% × 900 = 90 → payout=90
        //   advanced_count = 4 × 90/720 = 0.5 → max(1, floor) = 1
        assert_ok!(StealthPallet::claim_stealth_reward(
            RuntimeOrigin::signed(ALICE),
            eph,
            stealth_pk,
            sig,
            Vec::new(),
        ));
        assert_eq!(StealthPallet::claimed_receive_count(eph), 2);
        // 2回目 payout = 90, pool = 810
        assert_eq!(StealthPallet::stealth_reward_pool(), 810_000_000_000_000);
    });
}

#[test]
fn claim_stealth_reward_rejects_invalid_signature() {
    new_test_ext().execute_with(|| {
        let eph = [9u8; 32];
        let (stealth_pk, _good_sig) = ed25519_sign_for_claim(7, ALICE, eph);
        let bad_sig = [0u8; 64]; // 全 0 → 検証失敗

        StealthPallet::deposit_to_reward_pool(1_000_000_000_000_000);
        for _ in 0..5 {
            StealthPallet::record_recipient_receive(eph);
        }
        assert_noop!(
            StealthPallet::claim_stealth_reward(
                RuntimeOrigin::signed(ALICE),
                eph,
                stealth_pk,
                bad_sig,
                Vec::new(),
            ),
            Error::<Test>::InvalidStealthSignature
        );
    });
}

#[test]
fn claim_stealth_reward_rejects_signature_for_different_signer() {
    new_test_ext().execute_with(|| {
        let eph = [9u8; 32];
        // BOB 用に署名を作る
        let (stealth_pk, sig_for_bob) = ed25519_sign_for_claim(7, BOB, eph);

        StealthPallet::deposit_to_reward_pool(1_000_000_000_000_000);
        StealthPallet::record_recipient_receive(eph);
        // ALICE が BOB 用署名を流用しようとしても message が違うので検証失敗
        assert_noop!(
            StealthPallet::claim_stealth_reward(
                RuntimeOrigin::signed(ALICE),
                eph,
                stealth_pk,
                sig_for_bob,
                Vec::new(),
            ),
            Error::<Test>::InvalidStealthSignature
        );
    });
}

#[test]
fn claim_stealth_reward_fails_without_unclaimed() {
    new_test_ext().execute_with(|| {
        let eph = [9u8; 32];
        let (stealth_pk, sig) = ed25519_sign_for_claim(7, ALICE, eph);
        StealthPallet::deposit_to_reward_pool(1_000_000_000_000);
        // 受信 0 のまま claim → NoUnclaimedReceives (署名は valid)
        assert_noop!(
            StealthPallet::claim_stealth_reward(
                RuntimeOrigin::signed(ALICE),
                eph,
                stealth_pk,
                sig,
                Vec::new(),
            ),
            Error::<Test>::NoUnclaimedReceives
        );
    });
}

#[test]
fn claim_stealth_reward_fails_when_pool_empty() {
    new_test_ext().execute_with(|| {
        let eph = [9u8; 32];
        let (stealth_pk, sig) = ed25519_sign_for_claim(7, ALICE, eph);
        StealthPallet::record_recipient_receive(eph);
        assert_noop!(
            StealthPallet::claim_stealth_reward(
                RuntimeOrigin::signed(ALICE),
                eph,
                stealth_pk,
                sig,
                Vec::new(),
            ),
            Error::<Test>::StealthRewardPoolEmpty
        );
    });
}

#[test]
fn double_claim_returns_no_unclaimed_receives() {
    // 旧 `double_claim_returns_no_unclaimed_receives` の改訂版.
    // F2 修正 (Copilot #3199031147) 以降、cap が効くと 1 回目で claimed_count が
    // 比例分しか進まないため、5 件受信から始めると 2 回目以降も成功してしまう。
    // 「全消費 → NoUnclaimed」シナリオを示すため、received_count を 1 にし、
    // partial claim でも advanced_count = max(1, floor) = 1 で全消費される条件にする。
    new_test_ext().execute_with(|| {
        let eph = [9u8; 32];
        let (stealth_pk, sig) = ed25519_sign_for_claim(7, ALICE, eph);
        StealthPallet::deposit_to_reward_pool(1_000_000_000_000_000);
        StealthPallet::record_recipient_receive(eph); // received=1

        // 1 回目: 成功 (claimed_count = 1)
        assert_ok!(StealthPallet::claim_stealth_reward(
            RuntimeOrigin::signed(ALICE),
            eph,
            stealth_pk,
            sig,
            Vec::new(),
        ));
        assert_eq!(StealthPallet::claimed_receive_count(eph), 1);

        // 2 回目: 新規受信なし → unclaimed=0 で NoUnclaimedReceives
        assert_noop!(
            StealthPallet::claim_stealth_reward(
                RuntimeOrigin::signed(ALICE),
                eph,
                stealth_pk,
                sig,
                Vec::new(),
            ),
            Error::<Test>::NoUnclaimedReceives
        );
    });
}

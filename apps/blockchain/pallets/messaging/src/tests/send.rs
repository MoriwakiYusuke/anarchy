//! T027: `send_dm` unit tests.
//!
//! Contract: [`specs/019-direct-messages/contracts/pallet-messaging-extrinsics.md`] §E3。

#![cfg(test)]

use crate as pallet_messaging;
use crate::mock::{
    new_test_ext, storage_pool_deposits, stealth_reward_deposits, AccountId, Balances, RuntimeEvent,
    RuntimeOrigin, System, Test, ALICE, BOB, POOR_SENDER, RICH_SENDER,
};
use crate::types::{DmContentRef, DmDispatch};
use frame_support::{assert_noop, assert_ok, traits::fungible::Inspect};

const MORAL: u128 = 1_000_000_000_000;
const DM_BASE_COST: u128 = MORAL; // 1 MORAL
const DM_BYTE_COST: u128 = 50_000_000_000; // 0.05 MORAL / byte
const BUCKETS: [u64; 5] = [1_024, 4_096, 16_384, 65_536, 262_144];

fn expected_cost(ciphertext_len: u64) -> u128 {
    DM_BASE_COST + (ciphertext_len as u128) * DM_BYTE_COST
}

fn has_dispatched_event(expected: &DmDispatch<AccountId>, expected_message_id: u64) -> bool {
    System::events().iter().any(|r| match &r.event {
        RuntimeEvent::Messaging(pallet_messaging::Event::DmDispatched {
            message_id,
            recipient_stealth,
            ephemeral_pubkey,
            content_hash,
            ..
        }) => {
            *message_id == expected_message_id
                && *recipient_stealth == expected.recipient_stealth
                && *ephemeral_pubkey == expected.ephemeral_pubkey
                && *content_hash == expected.content.root
        }
        _ => false,
    })
}

fn nonzero_eph(tag: u8) -> [u8; 32] {
    [tag.max(1); 32]
}

fn merkle(tag: u8) -> [u8; 32] {
    let mut r = [0u8; 32];
    r[0] = tag.max(1);
    r[31] = tag.max(1);
    r
}

#[test]
fn send_dm_succeeds_for_every_bucket_size() {
    new_test_ext().execute_with(|| {
        for (i, &bucket) in BUCKETS.iter().enumerate() {
            let tag = (i + 1) as u8;
            let before = Balances::balance(&RICH_SENDER);

            assert_ok!(pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(RICH_SENDER),
                BOB,
                nonzero_eph(tag),
                merkle(tag),
                2,
                3,
                bucket,
            ));

            let after = Balances::balance(&RICH_SENDER);
            assert_eq!(
                before - after,
                expected_cost(bucket),
                "bucket {bucket} withdrew wrong amount"
            );
        }
    });
}

#[test]
fn send_dm_splits_fee_50_20_30() {
    new_test_ext().execute_with(|| {
        let bucket = 4_096u64;
        let cost = expected_cost(bucket);

        assert_ok!(pallet_messaging::Pallet::<Test>::send_dm(
            RuntimeOrigin::signed(RICH_SENDER),
            BOB,
            nonzero_eph(7),
            merkle(7),
            1,
            1,
            bucket,
        ));

        // TSTS v1: 50% storage / 20% stealth / 30% burn
        let storage_share = cost * 50 / 100;
        let stealth_share = cost * 20 / 100;

        assert_eq!(storage_pool_deposits(), storage_share);
        assert_eq!(stealth_reward_deposits(), stealth_share);
    });
}

#[test]
fn send_dm_emits_event_and_writes_storage() {
    new_test_ext().execute_with(|| {
        let root = merkle(9);
        let eph = nonzero_eph(9);

        assert_ok!(pallet_messaging::Pallet::<Test>::send_dm(
            RuntimeOrigin::signed(RICH_SENDER),
            BOB,
            eph,
            root,
            1,
            1,
            1_024,
        ));

        let msg_id = pallet_messaging::DmMessagesByRoot::<Test>::get(root);
        assert!(msg_id.is_some(), "DmMessagesByRoot entry missing");
        assert_eq!(msg_id.unwrap(), 0u64, "first dispatch must be message_id = 0");
        assert_eq!(pallet_messaging::NextMessageId::<Test>::get(), 1u64);

        let entries = pallet_messaging::DmDispatchesByBlock::<Test>::get(1u64);
        assert_eq!(entries.len(), 1);
        let expected = DmDispatch {
            recipient_stealth: BOB,
            ephemeral_pubkey: eph,
            content: DmContentRef {
                root,
                k: 1,
                n: 1,
                ciphertext_len: 1_024,
            },
        };
        assert_eq!(entries[0], expected);
        assert!(has_dispatched_event(&expected, 0));
    });
}

#[test]
fn send_dm_rejects_k_zero() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(RICH_SENDER),
                BOB,
                nonzero_eph(1),
                merkle(1),
                0,
                3,
                1_024,
            ),
            pallet_messaging::Error::<Test>::InvalidKNParameters
        );
    });
}

#[test]
fn send_dm_rejects_k_greater_than_n() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(RICH_SENDER),
                BOB,
                nonzero_eph(1),
                merkle(1),
                5,
                3,
                1_024,
            ),
            pallet_messaging::Error::<Test>::InvalidKNParameters
        );
    });
}

#[test]
fn send_dm_rejects_n_over_255() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(RICH_SENDER),
                BOB,
                nonzero_eph(1),
                merkle(1),
                1,
                256,
                1_024,
            ),
            pallet_messaging::Error::<Test>::InvalidKNParameters
        );
    });
}

#[test]
fn send_dm_rejects_non_bucket_ciphertext_len() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(RICH_SENDER),
                BOB,
                nonzero_eph(1),
                merkle(1),
                1,
                1,
                500,
            ),
            pallet_messaging::Error::<Test>::InvalidPaddingBucket
        );
    });
}

#[test]
fn send_dm_rejects_all_zero_ephemeral_pubkey() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(RICH_SENDER),
                BOB,
                [0u8; 32],
                merkle(1),
                1,
                1,
                1_024,
            ),
            pallet_messaging::Error::<Test>::InvalidMetaAddress
        );
    });
}

#[test]
fn send_dm_rejects_duplicate_merkle_root() {
    new_test_ext().execute_with(|| {
        let root = merkle(42);

        assert_ok!(pallet_messaging::Pallet::<Test>::send_dm(
            RuntimeOrigin::signed(RICH_SENDER),
            BOB,
            nonzero_eph(42),
            root,
            1,
            1,
            1_024,
        ));

        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(RICH_SENDER),
                BOB,
                nonzero_eph(43),
                root,
                1,
                1,
                1_024,
            ),
            pallet_messaging::Error::<Test>::DuplicateContent
        );
    });
}

#[test]
fn send_dm_rejects_257th_dispatch_in_single_block() {
    new_test_ext().execute_with(|| {
        // MaxDispatchesPerBlock = 256 (mock と runtime で同値)。257 件目をブロック 1 で投入。
        for i in 0u32..256 {
            // 重複しない merkle_root を 32B で量産。
            let mut root = [0u8; 32];
            root[0..4].copy_from_slice(&i.to_le_bytes());
            root[31] = 1;
            let mut eph = [1u8; 32];
            eph[0..4].copy_from_slice(&i.to_le_bytes());

            assert_ok!(pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(RICH_SENDER),
                BOB,
                eph,
                root,
                1,
                1,
                1_024,
            ));
        }

        let mut root_extra = [0u8; 32];
        root_extra[0..4].copy_from_slice(&256u32.to_le_bytes());
        root_extra[31] = 1;
        let mut eph_extra = [1u8; 32];
        eph_extra[0..4].copy_from_slice(&256u32.to_le_bytes());

        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(RICH_SENDER),
                BOB,
                eph_extra,
                root_extra,
                1,
                1,
                1_024,
            ),
            pallet_messaging::Error::<Test>::TooManyDispatchesInBlock
        );
    });
}

#[test]
fn send_dm_rejects_insufficient_balance() {
    new_test_ext().execute_with(|| {
        // POOR_SENDER は 10 MORAL しか持っていないので 1K バケット (≈52 MORAL) でも失敗する。
        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(POOR_SENDER),
                BOB,
                nonzero_eph(1),
                merkle(1),
                1,
                1,
                1_024,
            ),
            pallet_messaging::Error::<Test>::InsufficientStealthBalance
        );

        // プール残高が不変であることを確認 (FR-005 / E3 §Test acceptance #6)。
        assert_eq!(storage_pool_deposits(), 0);
        assert_eq!(stealth_reward_deposits(), 0);
    });
}

#[test]
fn send_dm_rejects_unsigned_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::none(),
                BOB,
                nonzero_eph(1),
                merkle(1),
                1,
                1,
                1_024,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn send_dm_rejects_when_above_max_ciphertext_len() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(RICH_SENDER),
                BOB,
                nonzero_eph(1),
                merkle(1),
                1,
                1,
                // MaxDmCiphertextLen = 262_144。これを超える値はバケット非該当としても
                // 弾かれるが、念のため明示テスト。
                524_288,
            ),
            pallet_messaging::Error::<Test>::InvalidPaddingBucket
        );
        // ちなみに ALICE は 1000 MORAL しか持たないので 256K 送信は残高不足で落ちる。
        assert_noop!(
            pallet_messaging::Pallet::<Test>::send_dm(
                RuntimeOrigin::signed(ALICE),
                BOB,
                nonzero_eph(2),
                merkle(2),
                1,
                1,
                262_144,
            ),
            pallet_messaging::Error::<Test>::InsufficientStealthBalance
        );
    });
}

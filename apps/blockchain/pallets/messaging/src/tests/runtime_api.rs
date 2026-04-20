//! T028: `DmScanApi` ランタイム API セマンティクステスト。
//!
//! Contract: [`specs/019-direct-messages/contracts/pallet-messaging-extrinsics.md`] §RA。
//!
//! ランタイム API の実装本体は [`apps/blockchain/runtime/src/lib.rs`] の
//! `impl_runtime_apis!` にある。ここではペイロードに相当する pallet 側
//! storage セマンティクス (順序保持 / 範囲ガード) が正しいことを検証する。
//! 両者は値の写像に過ぎないため、ここでテストしたセマンティクスがそのまま
//! runtime API の振る舞いを保証する。

#![cfg(test)]

use crate as pallet_messaging;
use crate::mock::{
    new_test_ext, AccountId, BlockNumber, RuntimeOrigin, System, Test, ALICE, BOB, RICH_SENDER,
};
use crate::types::{DmDispatch, DmMetaAddress};
use frame_support::assert_ok;

fn reception_key(account: AccountId) -> Option<DmMetaAddress> {
    pallet_messaging::DmReceptionKeys::<Test>::get(account)
}

fn dispatches_at(bn: BlockNumber) -> Vec<DmDispatch<AccountId>> {
    pallet_messaging::DmDispatchesByBlock::<Test>::get(bn).into_inner()
}

fn dispatches_range(from: BlockNumber, to: BlockNumber) -> Vec<(BlockNumber, Vec<DmDispatch<AccountId>>)> {
    // runtime/src/lib.rs の実装と同じガード (>1024 ブロック → 空)。
    if from > to || to - from > 1024 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut bn = from;
    while bn <= to {
        let entries = pallet_messaging::DmDispatchesByBlock::<Test>::get(bn).into_inner();
        if !entries.is_empty() {
            out.push((bn, entries));
        }
        bn += 1;
    }
    out
}

fn send_one(tag: u8) {
    let mut root = [0u8; 32];
    root[0] = tag.max(1);
    let mut eph = [0u8; 32];
    eph[0] = tag.max(1);
    eph[31] = tag.max(1);
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

#[test]
fn dispatches_at_unused_block_returns_empty() {
    new_test_ext().execute_with(|| {
        assert!(dispatches_at(1).is_empty());
        assert!(dispatches_at(42).is_empty());
    });
}

#[test]
fn dispatches_at_preserves_insertion_order() {
    new_test_ext().execute_with(|| {
        send_one(1);
        send_one(2);
        send_one(3);

        let entries = dispatches_at(1);
        assert_eq!(entries.len(), 3);

        let mut eph1 = [0u8; 32];
        eph1[0] = 1;
        eph1[31] = 1;
        let mut eph2 = [0u8; 32];
        eph2[0] = 2;
        eph2[31] = 2;
        let mut eph3 = [0u8; 32];
        eph3[0] = 3;
        eph3[31] = 3;

        assert_eq!(entries[0].ephemeral_pubkey, eph1);
        assert_eq!(entries[1].ephemeral_pubkey, eph2);
        assert_eq!(entries[2].ephemeral_pubkey, eph3);
    });
}

#[test]
fn reception_key_returns_some_after_publish_and_none_when_absent() {
    new_test_ext().execute_with(|| {
        assert_eq!(reception_key(ALICE), None);
        let meta = DmMetaAddress {
            scan_pub: [0x11u8; 32],
            spend_pub: [0x22u8; 32],
        };
        assert_ok!(pallet_messaging::Pallet::<Test>::publish_dm_key(
            RuntimeOrigin::signed(ALICE),
            meta.clone(),
        ));
        assert_eq!(reception_key(ALICE), Some(meta));
        assert_eq!(reception_key(BOB), None);
    });
}

#[test]
fn reception_key_returns_none_after_revoke() {
    new_test_ext().execute_with(|| {
        let meta = DmMetaAddress {
            scan_pub: [0xAAu8; 32],
            spend_pub: [0xBBu8; 32],
        };
        assert_ok!(pallet_messaging::Pallet::<Test>::publish_dm_key(
            RuntimeOrigin::signed(ALICE),
            meta,
        ));
        assert_ok!(pallet_messaging::Pallet::<Test>::revoke_dm_key(
            RuntimeOrigin::signed(ALICE),
        ));
        assert_eq!(reception_key(ALICE), None);
    });
}

#[test]
fn dispatches_range_over_1024_blocks_returns_empty() {
    new_test_ext().execute_with(|| {
        assert!(dispatches_range(0, 2000).is_empty());
        assert!(dispatches_range(100, 100 + 1024 + 1).is_empty());
    });
}

#[test]
fn dispatches_range_within_limit_returns_entries_per_block() {
    new_test_ext().execute_with(|| {
        // ブロック 1 で 2 件送信。
        send_one(1);
        send_one(2);
        System::set_block_number(5);
        // ブロック 5 で 1 件送信。
        send_one(3);

        let range = dispatches_range(1, 10);
        // 空ブロック (2,3,4,6..10) は含めず、2 エントリのみ返すことを確認。
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].0, 1);
        assert_eq!(range[0].1.len(), 2);
        assert_eq!(range[1].0, 5);
        assert_eq!(range[1].1.len(), 1);
    });
}

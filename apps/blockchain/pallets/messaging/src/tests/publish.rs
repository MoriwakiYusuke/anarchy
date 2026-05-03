//! T025: `publish_dm_key` unit tests.
//!
//! Contract: [`specs/019-direct-messages/contracts/pallet-messaging-extrinsics.md`] §E1。

#![cfg(test)]

use crate as pallet_messaging;
use crate::mock::{new_test_ext, AccountId, RuntimeEvent, RuntimeOrigin, System, Test, ALICE};
use crate::types::DmMetaAddress;
use frame_support::{assert_noop, assert_ok};

fn valid_meta() -> DmMetaAddress {
    DmMetaAddress {
        scan_pub: [0xAAu8; 32],
        spend_pub: [0xBBu8; 32],
    }
}

fn has_event<F>(f: F) -> bool
where
    F: Fn(&pallet_messaging::Event<Test>) -> bool,
{
    System::events().iter().any(|r| match &r.event {
        RuntimeEvent::Messaging(ev) => f(ev),
        _ => false,
    })
}

#[test]
fn publish_dm_key_happy_path_inserts_and_emits() {
    new_test_ext().execute_with(|| {
        let meta = valid_meta();

        assert_ok!(pallet_messaging::Pallet::<Test>::publish_dm_key(
            RuntimeOrigin::signed(ALICE),
            meta.clone(),
        ));

        let stored = pallet_messaging::DmReceptionKeys::<Test>::get(ALICE);
        assert_eq!(stored, Some(meta));
        assert!(has_event(|ev| matches!(
            ev,
            pallet_messaging::Event::DmKeyPublished { account } if *account == ALICE as AccountId
        )));
    });
}

#[test]
fn publish_dm_key_overwrites_existing_entry() {
    new_test_ext().execute_with(|| {
        let first = valid_meta();
        let second = DmMetaAddress {
            scan_pub: [0x11u8; 32],
            spend_pub: [0x22u8; 32],
        };

        assert_ok!(pallet_messaging::Pallet::<Test>::publish_dm_key(
            RuntimeOrigin::signed(ALICE),
            first,
        ));
        assert_ok!(pallet_messaging::Pallet::<Test>::publish_dm_key(
            RuntimeOrigin::signed(ALICE),
            second.clone(),
        ));

        assert_eq!(
            pallet_messaging::DmReceptionKeys::<Test>::get(ALICE),
            Some(second),
        );
    });
}

#[test]
fn publish_dm_key_rejects_all_zero_scan_pub() {
    new_test_ext().execute_with(|| {
        let bad = DmMetaAddress {
            scan_pub: [0u8; 32],
            spend_pub: [0xBBu8; 32],
        };

        assert_noop!(
            pallet_messaging::Pallet::<Test>::publish_dm_key(RuntimeOrigin::signed(ALICE), bad),
            pallet_messaging::Error::<Test>::InvalidMetaAddress
        );
        assert!(pallet_messaging::DmReceptionKeys::<Test>::get(ALICE).is_none());
    });
}

#[test]
fn publish_dm_key_rejects_all_zero_spend_pub() {
    new_test_ext().execute_with(|| {
        let bad = DmMetaAddress {
            scan_pub: [0xAAu8; 32],
            spend_pub: [0u8; 32],
        };

        assert_noop!(
            pallet_messaging::Pallet::<Test>::publish_dm_key(RuntimeOrigin::signed(ALICE), bad),
            pallet_messaging::Error::<Test>::InvalidMetaAddress
        );
    });
}

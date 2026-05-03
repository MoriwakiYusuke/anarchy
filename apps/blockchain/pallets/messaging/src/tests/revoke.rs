//! T026: `revoke_dm_key` unit tests.
//!
//! Contract: [`specs/019-direct-messages/contracts/pallet-messaging-extrinsics.md`] §E2。

#![cfg(test)]

use crate as pallet_messaging;
use crate::mock::{new_test_ext, AccountId, RuntimeEvent, RuntimeOrigin, System, Test, ALICE};
use crate::types::DmMetaAddress;
use frame_support::{assert_noop, assert_ok};

fn publish_ok() -> DmMetaAddress {
    let meta = DmMetaAddress {
        scan_pub: [0xAAu8; 32],
        spend_pub: [0xBBu8; 32],
    };
    assert_ok!(pallet_messaging::Pallet::<Test>::publish_dm_key(
        RuntimeOrigin::signed(ALICE),
        meta.clone(),
    ));
    meta
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
fn revoke_dm_key_happy_path_removes_and_emits() {
    new_test_ext().execute_with(|| {
        let _meta = publish_ok();
        assert!(pallet_messaging::DmReceptionKeys::<Test>::contains_key(ALICE));

        assert_ok!(pallet_messaging::Pallet::<Test>::revoke_dm_key(
            RuntimeOrigin::signed(ALICE),
        ));

        assert!(!pallet_messaging::DmReceptionKeys::<Test>::contains_key(ALICE));
        assert!(has_event(|ev| matches!(
            ev,
            pallet_messaging::Event::DmKeyRevoked { account } if *account == ALICE as AccountId
        )));
    });
}

#[test]
fn revoke_dm_key_rejects_when_not_published() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            pallet_messaging::Pallet::<Test>::revoke_dm_key(RuntimeOrigin::signed(ALICE)),
            pallet_messaging::Error::<Test>::ReceptionKeyNotPublished
        );
    });
}

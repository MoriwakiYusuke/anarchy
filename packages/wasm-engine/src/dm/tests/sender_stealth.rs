//! T030: W3 (`dm_generate_sender_stealth`) uniqueness/reproducibility と
//! W5 (`dm_derive_recipient_stealth`) と既存 `derive_stealth_address` のパリティ。
//!
//! Contract: [`specs/019-direct-messages/contracts/wasm-engine-dm-api.md`] §W3 / §W5。

#![cfg(test)]

use crate::dm::encrypt::{dm_derive_recipient_stealth, dm_generate_sender_stealth};
use crate::stealth::address::{derive_stealth_address, format_meta_address};
use crate::stealth::scan::scan_transaction;
use curve25519_dalek::{constants::ED25519_BASEPOINT_TABLE, Scalar};
use rand_core::{OsRng, RngCore};
use schnorrkel::{ExpansionMode, MiniSecretKey};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

// -----------------------------------------------------------------------------
// W3: dm_generate_sender_stealth
// -----------------------------------------------------------------------------

#[test]
fn w3_two_calls_produce_distinct_seeds_and_accounts() {
    let a = dm_generate_sender_stealth();
    let b = dm_generate_sender_stealth();

    assert_eq!(a.account_id().len(), 32);
    assert_eq!(a.secret_seed().len(), 32);
    assert_eq!(b.account_id().len(), 32);
    assert_eq!(b.secret_seed().len(), 32);

    assert_ne!(
        a.secret_seed(),
        b.secret_seed(),
        "W3: two consecutive calls must produce distinct seeds"
    );
    assert_ne!(
        a.account_id(),
        b.account_id(),
        "W3: two consecutive calls must produce distinct account_ids"
    );
}

#[test]
fn w3_account_id_is_reproducible_from_seed() {
    let s = dm_generate_sender_stealth();
    let seed = s.secret_seed();
    let account_id = s.account_id();

    let mut seed_arr = [0u8; 32];
    seed_arr.copy_from_slice(&seed);

    // 公開鍵を seed から復元 (schnorrkel Sr25519)。
    let mini = MiniSecretKey::from_bytes(&seed_arr).expect("MiniSecretKey from 32B seed");
    let kp = mini.expand_to_keypair(ExpansionMode::Ed25519);
    let reconstructed = kp.public.to_bytes().to_vec();

    assert_eq!(
        reconstructed, account_id,
        "W3: pubkey reconstructed from seed must equal account_id"
    );
}

#[test]
fn w3_seed_is_nonzero() {
    // getrandom がゼロを返すのは天文学的確率なので、もしゼロが出るなら entropy
    // ソースが壊れている可能性が高い。ここではスモークテストとして検査する。
    let s = dm_generate_sender_stealth();
    let seed = s.secret_seed();
    assert!(seed.iter().any(|b| *b != 0), "W3: seed must not be all-zero");
}

// -----------------------------------------------------------------------------
// W5: dm_derive_recipient_stealth が既存 derive_stealth_address と数学的に等価
// -----------------------------------------------------------------------------

/// 受信者鍵の生成 (テスト用)。
struct RecipientFixture {
    scan_priv: StaticSecret,
    scan_pub: [u8; 32],
    spend_pub: [u8; 32],
}

fn make_recipient() -> RecipientFixture {
    let scan_priv = StaticSecret::random_from_rng(OsRng);
    let scan_pub_pt = X25519PublicKey::from(&scan_priv);

    let mut spend_scalar_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut spend_scalar_bytes);
    let spend_scalar = Scalar::from_bytes_mod_order(spend_scalar_bytes);
    let spend_point = ED25519_BASEPOINT_TABLE * &spend_scalar;

    RecipientFixture {
        scan_priv,
        scan_pub: *scan_pub_pt.as_bytes(),
        spend_pub: spend_point.compress().to_bytes(),
    }
}

#[test]
fn w5_dm_derive_output_is_owned_by_recipient_scan_priv() {
    let r = make_recipient();
    let mut eph_priv = [0u8; 32];
    OsRng.fill_bytes(&mut eph_priv);

    let derived =
        dm_derive_recipient_stealth(&r.scan_pub, &r.spend_pub, &eph_priv).expect("W5 ok");
    let stealth_pub = derived.stealth_pubkey();
    let eph_pub = derived.ephemeral_pubkey();

    // 受信者は scan_transaction で自分宛 DM だと検出できなければならない。
    // これは既存 derive_stealth_address の出力と「同じ走査関数」で受信できるという
    // 意味で W5 と既存 EIP-5564 ロジックがパリティであることを示す。
    let owned = scan_transaction(
        r.scan_priv.as_bytes(),
        &eph_pub,
        &stealth_pub,
        &r.spend_pub,
    );
    assert!(owned, "recipient must recognise W5-derived stealth as their own");
}

#[test]
fn w5_existing_derive_stealth_address_remains_owned() {
    // 既存パスで生成した stealth も同じ scan ロジックで検出できる
    // (パリティの基準)。
    let r = make_recipient();
    let meta = format_meta_address(&r.spend_pub, &r.scan_pub);

    let result = derive_stealth_address(&meta).expect("legacy derive ok");
    let eph_pub = result.ephemeral_pubkey();
    let stealth_pub = result.stealth_pubkey();

    let owned = scan_transaction(
        r.scan_priv.as_bytes(),
        &eph_pub,
        &stealth_pub,
        &r.spend_pub,
    );
    assert!(owned, "legacy derive_stealth_address must remain scan-compatible");
}

#[test]
fn w5_foreign_recipient_cannot_claim_dm_stealth() {
    let alice = make_recipient();
    let bob = make_recipient();
    let mut eph_priv = [0u8; 32];
    OsRng.fill_bytes(&mut eph_priv);

    let derived =
        dm_derive_recipient_stealth(&alice.scan_pub, &alice.spend_pub, &eph_priv).expect("W5 ok");

    let owned_by_bob = scan_transaction(
        bob.scan_priv.as_bytes(),
        &derived.ephemeral_pubkey(),
        &derived.stealth_pubkey(),
        // bob が自分の spend で走査しても合致しない (たとえ alice の spend で
        // 走査したとしても scan_priv がずれていれば s が一致しない)。
        &bob.spend_pub,
    );
    assert!(!owned_by_bob, "bob must not match alice-targeted stealth");
}

#[test]
fn w5_distinct_eph_priv_produces_distinct_stealth() {
    let r = make_recipient();
    let mut eph1 = [0u8; 32];
    let mut eph2 = [0u8; 32];
    OsRng.fill_bytes(&mut eph1);
    OsRng.fill_bytes(&mut eph2);

    let d1 = dm_derive_recipient_stealth(&r.scan_pub, &r.spend_pub, &eph1).unwrap();
    let d2 = dm_derive_recipient_stealth(&r.scan_pub, &r.spend_pub, &eph2).unwrap();

    assert_ne!(d1.ephemeral_pubkey(), d2.ephemeral_pubkey());
    assert_ne!(d1.stealth_pubkey(), d2.stealth_pubkey());
    assert_ne!(d1.shared_secret(), d2.shared_secret());
}

//! T029: `dm_encrypt_and_pad` ↔ `dm_decrypt_scan` ラウンドトリップテスト。
//!
//! Contract: [`specs/019-direct-messages/contracts/wasm-engine-dm-api.md`] §W1 / §W2。

#![cfg(test)]

use crate::dm::decrypt::dm_decrypt_scan;
use crate::dm::encrypt::{
    dm_derive_recipient_stealth, dm_encrypt_and_pad, dm_generate_sender_stealth,
};
use crate::dm::envelope::compute_inner_signed_hash;
use curve25519_dalek::{constants::ED25519_BASEPOINT_TABLE, Scalar};
use rand_core::{OsRng, RngCore};
use schnorrkel::{signing_context, ExpansionMode, Keypair, MiniSecretKey};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

const SR25519_CONTEXT: &[u8] = b"substrate";

/// 受信者鍵材料 (scan_priv + spend_priv + 公開鍵) をテスト用にひとまとめに。
struct RecipientKeys {
    scan_priv: StaticSecret,
    scan_pub: [u8; 32],
    spend_pub: [u8; 32],
}

fn make_recipient() -> RecipientKeys {
    let scan_priv = StaticSecret::random_from_rng(OsRng);
    let scan_pub = X25519PublicKey::from(&scan_priv);

    let mut spend_scalar_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut spend_scalar_bytes);
    let spend_scalar = Scalar::from_bytes_mod_order(spend_scalar_bytes);
    let spend_point = ED25519_BASEPOINT_TABLE * &spend_scalar;

    RecipientKeys {
        scan_priv,
        scan_pub: *scan_pub.as_bytes(),
        spend_pub: spend_point.compress().to_bytes(),
    }
}

struct PreparedSender {
    eph_priv: [u8; 32],
    eph_pub: [u8; 32],
    recipient_stealth: [u8; 32],
    sender_account: [u8; 32],
    signature: [u8; 64],
}

/// W5 で stealth / eph_pub を先取りし、外部署名 (ウォレット相当) を組み立てる。
fn prepare_sender(
    recipient: &RecipientKeys,
    body: &[u8],
    timestamp_ms: u64,
) -> PreparedSender {
    let mut eph_priv = [0u8; 32];
    OsRng.fill_bytes(&mut eph_priv);

    let derived =
        dm_derive_recipient_stealth(&recipient.scan_pub, &recipient.spend_pub, &eph_priv)
            .expect("derive ok");
    let eph_pub_vec = derived.ephemeral_pubkey();
    let stealth_vec = derived.stealth_pubkey();

    let mut eph_pub = [0u8; 32];
    eph_pub.copy_from_slice(&eph_pub_vec);
    let mut recipient_stealth = [0u8; 32];
    recipient_stealth.copy_from_slice(&stealth_vec);

    // 送信者 Sr25519 keypair。
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let kp: Keypair = MiniSecretKey::from_bytes(&seed)
        .expect("mini key")
        .expand_to_keypair(ExpansionMode::Ed25519);
    let sender_account = kp.public.to_bytes();

    let inner_hash = compute_inner_signed_hash(
        &sender_account,
        &recipient_stealth,
        &eph_pub,
        timestamp_ms,
        body,
    );
    let sig = kp.sign(signing_context(SR25519_CONTEXT).bytes(&inner_hash));
    let signature = sig.to_bytes();

    PreparedSender {
        eph_priv,
        eph_pub,
        recipient_stealth,
        sender_account,
        signature,
    }
}

#[test]
fn roundtrip_recovers_plaintext_body_and_signature() {
    let recipient = make_recipient();
    let body = b"hello, direct message!".to_vec();
    let ts = 1_714_500_000_000u64;
    let p = prepare_sender(&recipient, &body, ts);

    let out = dm_encrypt_and_pad(
        &recipient.scan_pub,
        &recipient.spend_pub,
        &p.sender_account,
        &p.signature,
        &body,
        ts,
        &p.eph_priv,
    )
    .expect("encrypt ok");

    assert_eq!(out.recipient_stealth(), p.recipient_stealth.to_vec());
    assert_eq!(out.ephemeral_pubkey(), p.eph_pub.to_vec());
    assert!([1024u32, 4096, 16384, 65536, 262144].contains(&out.padding_bucket()));
    assert_eq!(out.ciphertext().len() as u32, out.padding_bucket());

    let decrypted = dm_decrypt_scan(
        recipient.scan_priv.as_bytes(),
        &recipient.spend_pub,
        &out.ephemeral_pubkey(),
        &out.recipient_stealth(),
        &out.ciphertext(),
    )
    .expect("decrypt ok (my DM)");

    assert_eq!(decrypted.body(), body);
    assert_eq!(decrypted.timestamp_ms(), ts);
    assert_eq!(decrypted.sender_main_account(), p.sender_account.to_vec());
    assert!(decrypted.signature_valid());
}

#[test]
fn foreign_recipient_cannot_decrypt_returns_none() {
    let alice = make_recipient();
    let bob = make_recipient();
    let body = b"secret".to_vec();
    let ts = 1_714_500_000_001u64;
    let p = prepare_sender(&alice, &body, ts);

    let out = dm_encrypt_and_pad(
        &alice.scan_pub,
        &alice.spend_pub,
        &p.sender_account,
        &p.signature,
        &body,
        ts,
        &p.eph_priv,
    )
    .expect("encrypt ok");

    let attempt = dm_decrypt_scan(
        bob.scan_priv.as_bytes(),
        &bob.spend_pub,
        &out.ephemeral_pubkey(),
        &out.recipient_stealth(),
        &out.ciphertext(),
    );
    assert!(attempt.is_none(), "foreign recipient must not decrypt");
}

#[test]
fn ciphertext_bit_flip_returns_none() {
    let r = make_recipient();
    let body = b"payload".to_vec();
    let ts = 1_714_500_000_002u64;
    let p = prepare_sender(&r, &body, ts);

    let out = dm_encrypt_and_pad(
        &r.scan_pub,
        &r.spend_pub,
        &p.sender_account,
        &p.signature,
        &body,
        ts,
        &p.eph_priv,
    )
    .expect("encrypt ok");

    let mut corrupted = out.ciphertext();
    corrupted[0] ^= 0x01;

    let attempt = dm_decrypt_scan(
        r.scan_priv.as_bytes(),
        &r.spend_pub,
        &out.ephemeral_pubkey(),
        &out.recipient_stealth(),
        &corrupted,
    );
    assert!(attempt.is_none(), "AES-GCM must reject a bit-flipped ciphertext");
}

#[test]
fn empty_body_fits_1kb_bucket() {
    let r = make_recipient();
    let body = Vec::<u8>::new();
    let ts = 1_714_500_000_003u64;
    let p = prepare_sender(&r, &body, ts);

    let out = dm_encrypt_and_pad(
        &r.scan_pub,
        &r.spend_pub,
        &p.sender_account,
        &p.signature,
        &body,
        ts,
        &p.eph_priv,
    )
    .expect("encrypt ok");

    assert_eq!(out.padding_bucket(), 1024);
    assert_eq!(out.ciphertext().len(), 1024);

    let decrypted = dm_decrypt_scan(
        r.scan_priv.as_bytes(),
        &r.spend_pub,
        &out.ephemeral_pubkey(),
        &out.recipient_stealth(),
        &out.ciphertext(),
    )
    .expect("decrypt ok");
    assert!(decrypted.body().is_empty());
    assert!(decrypted.signature_valid());
}

#[test]
fn w3_generate_sender_stealth_is_plumbed() {
    // DmSenderStealth を実体化できるだけ確認 (詳細テストは T030)。
    let s = dm_generate_sender_stealth();
    assert_eq!(s.account_id().len(), 32);
    assert_eq!(s.secret_seed().len(), 32);
}

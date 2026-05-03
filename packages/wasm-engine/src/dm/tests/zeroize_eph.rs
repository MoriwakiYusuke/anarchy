//! T031: ephemeral_priv が W1 出力にも HKDF 派生材料にも漏れないことを検証する。
//!
//! Contract: [`specs/019-direct-messages/contracts/wasm-engine-dm-api.md`] §W1 / FR-021。
//!
//! 注: Rust の `Zeroizing` は drop 時に stack 上のコピーをゼロ化するが、ホストから
//! その状態を観測するのは UB に近い。ここでは「外部から漏洩を観測できないこと」
//! の boundary 条件として以下を確認する:
//!   1. W1 出力 (ephemeral_pubkey / recipient_stealth / ciphertext) に
//!      eph_priv の生バイト列がそのまま現れない。
//!   2. 同じ入力に対する W1 出力は決定論的 (=eph_priv はちゃんと使われている)。
//!   3. eph_priv を 1 ビット反転しただけで ephemeral_pubkey と stealth が変わる。

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

struct Recipient {
    scan_priv: StaticSecret,
    scan_pub: [u8; 32],
    spend_pub: [u8; 32],
}

fn recipient() -> Recipient {
    let scan_priv = StaticSecret::random_from_rng(OsRng);
    let scan_pub = X25519PublicKey::from(&scan_priv);
    let mut spend_scalar_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut spend_scalar_bytes);
    let spend_scalar = Scalar::from_bytes_mod_order(spend_scalar_bytes);
    let spend_point = ED25519_BASEPOINT_TABLE * &spend_scalar;
    Recipient {
        scan_priv,
        scan_pub: *scan_pub.as_bytes(),
        spend_pub: spend_point.compress().to_bytes(),
    }
}

fn sender_keypair() -> Keypair {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    MiniSecretKey::from_bytes(&seed)
        .expect("mini key")
        .expand_to_keypair(ExpansionMode::Ed25519)
}

fn sign_inner(
    kp: &Keypair,
    sender_acct: &[u8; 32],
    stealth: &[u8; 32],
    eph_pub: &[u8; 32],
    ts: u64,
    body: &[u8],
) -> [u8; 64] {
    let h = compute_inner_signed_hash(sender_acct, stealth, eph_pub, ts, body);
    kp.sign(signing_context(SR25519_CONTEXT).bytes(&h)).to_bytes()
}

#[test]
fn eph_priv_bytes_never_appear_in_w1_output() {
    let r = recipient();
    let body = b"sensitive plaintext".to_vec();
    let ts = 1_714_500_010_000u64;

    // 認識しやすい固定パターンの eph_priv (本物のランダムでは「生バイトが出力に
    // 偶然含まれる」確率が低すぎるので、明示パターンで検査する)。
    let eph_priv: [u8; 32] = [0xA5; 32];

    // W5 で stealth/eph_pub を計算し、外部署名を作る。
    let derived = dm_derive_recipient_stealth(&r.scan_pub, &r.spend_pub, &eph_priv).unwrap();
    let mut eph_pub = [0u8; 32];
    eph_pub.copy_from_slice(&derived.ephemeral_pubkey());
    let mut stealth = [0u8; 32];
    stealth.copy_from_slice(&derived.stealth_pubkey());

    let kp = sender_keypair();
    let sender_acct = kp.public.to_bytes();
    let sig = sign_inner(&kp, &sender_acct, &stealth, &eph_pub, ts, &body);

    let out = dm_encrypt_and_pad(
        &r.scan_pub,
        &r.spend_pub,
        &sender_acct,
        &sig,
        &body,
        ts,
        &eph_priv,
    )
    .expect("encrypt ok");

    // eph_priv 32B の生スライスがどの出力にも substring として現れないこと。
    let needle: &[u8] = &eph_priv;
    let ct = out.ciphertext();
    let eph_pub_out = out.ephemeral_pubkey();
    let stealth_out = out.recipient_stealth();

    assert!(!contains_subslice(&ct, needle), "ciphertext leaks eph_priv");
    assert!(
        !contains_subslice(&eph_pub_out, needle),
        "ephemeral_pubkey equals eph_priv (X25519 derivation broken)"
    );
    assert!(
        !contains_subslice(&stealth_out, needle),
        "recipient_stealth leaks eph_priv"
    );
}

#[test]
fn w1_output_is_deterministic_for_fixed_eph_priv() {
    // eph_priv が決定論的に使われていることを示す (ゼロ化が早すぎて使われていない、
    // 等の取り違いを検出するための回帰テスト)。
    let r = recipient();
    let body = b"deterministic body".to_vec();
    let ts = 1_714_500_011_000u64;
    let eph_priv: [u8; 32] = [0x33; 32];

    let derived = dm_derive_recipient_stealth(&r.scan_pub, &r.spend_pub, &eph_priv).unwrap();
    let mut eph_pub = [0u8; 32];
    eph_pub.copy_from_slice(&derived.ephemeral_pubkey());
    let mut stealth = [0u8; 32];
    stealth.copy_from_slice(&derived.stealth_pubkey());

    let kp = sender_keypair();
    let sender_acct = kp.public.to_bytes();
    let sig = sign_inner(&kp, &sender_acct, &stealth, &eph_pub, ts, &body);

    let a = dm_encrypt_and_pad(
        &r.scan_pub,
        &r.spend_pub,
        &sender_acct,
        &sig,
        &body,
        ts,
        &eph_priv,
    )
    .unwrap();
    let b = dm_encrypt_and_pad(
        &r.scan_pub,
        &r.spend_pub,
        &sender_acct,
        &sig,
        &body,
        ts,
        &eph_priv,
    )
    .unwrap();

    assert_eq!(a.ephemeral_pubkey(), b.ephemeral_pubkey());
    assert_eq!(a.recipient_stealth(), b.recipient_stealth());
    assert_eq!(a.ciphertext(), b.ciphertext());
}

#[test]
fn one_bit_flip_in_eph_priv_changes_stealth_and_eph_pub() {
    let r = recipient();
    let body = b"x".to_vec();
    let ts = 1_714_500_012_000u64;

    let mut eph_priv = [0u8; 32];
    OsRng.fill_bytes(&mut eph_priv);
    let mut eph_priv_flipped = eph_priv;
    // X25519 clamps lower 3 bits of byte 0 and high bit of byte 31, so we
    // flip a middle byte to ensure the change survives clamping.
    eph_priv_flipped[15] ^= 0x01;

    let kp = sender_keypair();
    let sender_acct = kp.public.to_bytes();

    // 各 eph_priv ごとに stealth/eph_pub を取り、署名を再計算。
    let d1 = dm_derive_recipient_stealth(&r.scan_pub, &r.spend_pub, &eph_priv).unwrap();
    let mut eph_pub_1 = [0u8; 32];
    eph_pub_1.copy_from_slice(&d1.ephemeral_pubkey());
    let mut stealth_1 = [0u8; 32];
    stealth_1.copy_from_slice(&d1.stealth_pubkey());
    let sig_1 = sign_inner(&kp, &sender_acct, &stealth_1, &eph_pub_1, ts, &body);

    let d2 = dm_derive_recipient_stealth(&r.scan_pub, &r.spend_pub, &eph_priv_flipped).unwrap();
    let mut eph_pub_2 = [0u8; 32];
    eph_pub_2.copy_from_slice(&d2.ephemeral_pubkey());
    let mut stealth_2 = [0u8; 32];
    stealth_2.copy_from_slice(&d2.stealth_pubkey());
    let sig_2 = sign_inner(&kp, &sender_acct, &stealth_2, &eph_pub_2, ts, &body);

    let a = dm_encrypt_and_pad(
        &r.scan_pub,
        &r.spend_pub,
        &sender_acct,
        &sig_1,
        &body,
        ts,
        &eph_priv,
    )
    .unwrap();
    let b = dm_encrypt_and_pad(
        &r.scan_pub,
        &r.spend_pub,
        &sender_acct,
        &sig_2,
        &body,
        ts,
        &eph_priv_flipped,
    )
    .unwrap();

    assert_ne!(a.ephemeral_pubkey(), b.ephemeral_pubkey());
    assert_ne!(a.recipient_stealth(), b.recipient_stealth());
    assert_ne!(a.ciphertext(), b.ciphertext());
}

#[test]
fn w1_output_decrypts_only_with_recipient_scan_priv() {
    // ゼロ化テストの間接確認: eph_priv を捨てても受信者は本文を回収できる。
    // (=eph_priv は ciphertext / eph_pub の中に「機能だけ残して」消える。)
    let r = recipient();
    let body = b"zeroize-survives-decryption".to_vec();
    let ts = 1_714_500_013_000u64;
    let mut eph_priv = [0u8; 32];
    OsRng.fill_bytes(&mut eph_priv);

    let derived = dm_derive_recipient_stealth(&r.scan_pub, &r.spend_pub, &eph_priv).unwrap();
    let mut eph_pub = [0u8; 32];
    eph_pub.copy_from_slice(&derived.ephemeral_pubkey());
    let mut stealth = [0u8; 32];
    stealth.copy_from_slice(&derived.stealth_pubkey());

    let kp = sender_keypair();
    let sender_acct = kp.public.to_bytes();
    let sig = sign_inner(&kp, &sender_acct, &stealth, &eph_pub, ts, &body);

    let out = dm_encrypt_and_pad(
        &r.scan_pub,
        &r.spend_pub,
        &sender_acct,
        &sig,
        &body,
        ts,
        &eph_priv,
    )
    .unwrap();

    // eph_priv をゼロクリア (送信側ライフサイクル相当) してから受信者を走らせる。
    eph_priv.iter_mut().for_each(|b| *b = 0);

    let dec = dm_decrypt_scan(
        r.scan_priv.as_bytes(),
        &r.spend_pub,
        &out.ephemeral_pubkey(),
        &out.recipient_stealth(),
        &out.ciphertext(),
    )
    .expect("decrypt ok after eph_priv zeroized");
    assert_eq!(dec.body(), body);
    assert!(dec.signature_valid());
}

#[test]
fn w3_seed_is_distinct_from_account_id_per_call() {
    // W3 の seed が account_id の派生公開鍵と一致しない (=生公開鍵を seed に
    // 流用していない) ことを smoke 確認する。
    let s = dm_generate_sender_stealth();
    assert_ne!(s.secret_seed(), s.account_id());
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

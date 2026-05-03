//! W2. `dm_decrypt_scan` — DmDispatch エントリを受信者鍵で走査・復号する。
//!
//! Contract: [`specs/019-direct-messages/contracts/wasm-engine-dm-api.md`] §W2。

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use curve25519_dalek::constants::ED25519_BASEPOINT_TABLE;
use curve25519_dalek::{edwards::CompressedEdwardsY, Scalar};
use parity_scale_codec::Decode;
use schnorrkel::{signing_context, PublicKey, Signature};
use wasm_bindgen::prelude::*;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::Zeroizing;

use super::encrypt::{hkdf_okm, HKDF_SALT};
use super::envelope::{compute_inner_signed_hash, DmEnvelope, DM_PROTOCOL_VERSION};
use super::padding::{strip_iso7816_4, AES_GCM_TAG_LEN};
use super::super::stealth::hash::blake2b_256;

const SR25519_CONTEXT: &[u8] = b"substrate";

/// W2 が成功したときに返す復号後 envelope。
#[wasm_bindgen]
pub struct DmDecryptedEnvelope {
    sender_main_account: [u8; 32],
    timestamp_ms: u64,
    body: Vec<u8>,
    signature_valid: bool,
}

#[wasm_bindgen]
impl DmDecryptedEnvelope {
    /// Sr25519 AccountId32 (32B)。
    #[wasm_bindgen(getter)]
    pub fn sender_main_account(&self) -> Vec<u8> {
        self.sender_main_account.to_vec()
    }
    /// 送信者ローカルの UNIX ms。
    #[wasm_bindgen(getter)]
    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }
    /// 平文本文。
    #[wasm_bindgen(getter)]
    pub fn body(&self) -> Vec<u8> {
        self.body.clone()
    }
    /// 内部署名 (W6) の検証結果。
    #[wasm_bindgen(getter)]
    pub fn signature_valid(&self) -> bool {
        self.signature_valid
    }
}

/// AES-GCM AAD: `recipient_stealth || eph_pub || (padded_len as u32 BE) || version`。
/// W1 と必ず同じ内容を組み立てなければならない。
fn build_aad(recipient_stealth: &[u8; 32], eph_pub: &[u8; 32], padded_len: u32) -> Vec<u8> {
    let mut aad = Vec::with_capacity(32 + 32 + 4 + 1);
    aad.extend_from_slice(recipient_stealth);
    aad.extend_from_slice(eph_pub);
    aad.extend_from_slice(&padded_len.to_be_bytes());
    aad.push(DM_PROTOCOL_VERSION);
    aad
}

/// W2. `dm_decrypt_scan`
///
/// 1 件の `DmDispatch` を受け、自分宛 (= stealth identity が一致) なら復号して
/// envelope を返す。stealth 不一致 / AES-GCM 失敗 / SCALE decode 失敗はすべて
/// `None` を返す (タイミングサイドチャネル緩和)。
#[wasm_bindgen]
pub fn dm_decrypt_scan(
    own_scan_priv: &[u8],
    own_spend_pub: &[u8],
    ephemeral_pubkey: &[u8],
    purported_recipient_stealth: &[u8],
    ciphertext: &[u8],
) -> Option<DmDecryptedEnvelope> {
    if own_scan_priv.len() != 32
        || own_spend_pub.len() != 32
        || ephemeral_pubkey.len() != 32
        || purported_recipient_stealth.len() != 32
    {
        return None;
    }
    if ciphertext.len() < AES_GCM_TAG_LEN {
        return None;
    }

    // ---- Step 1: stealth identity check (K_spend + H(s)*G == purported) ----
    let mut scan_priv_bytes = [0u8; 32];
    scan_priv_bytes.copy_from_slice(own_scan_priv);
    let scan_secret = Zeroizing::new(StaticSecret::from(scan_priv_bytes));

    let mut eph_pub_bytes = [0u8; 32];
    eph_pub_bytes.copy_from_slice(ephemeral_pubkey);
    let eph_pub = X25519PublicKey::from(eph_pub_bytes);

    let shared = scan_secret.diffie_hellman(&eph_pub);
    let h = blake2b_256(shared.as_bytes());
    let h_scalar = Scalar::from_bytes_mod_order(h);

    let mut spend_pub_bytes = [0u8; 32];
    spend_pub_bytes.copy_from_slice(own_spend_pub);
    let spend_point = CompressedEdwardsY(spend_pub_bytes).decompress()?;
    let h_g = ED25519_BASEPOINT_TABLE * &h_scalar;
    let expected_stealth: [u8; 32] = (spend_point + h_g).compress().to_bytes();

    let mut purported_stealth_bytes = [0u8; 32];
    purported_stealth_bytes.copy_from_slice(purported_recipient_stealth);
    if expected_stealth != purported_stealth_bytes {
        return None;
    }

    // ---- Step 2: HKDF + AES-GCM 復号 ----
    let mut shared_bytes = Zeroizing::new([0u8; 32]);
    shared_bytes.copy_from_slice(shared.as_bytes());

    let mut info = Vec::with_capacity(64);
    info.extend_from_slice(&purported_stealth_bytes);
    info.extend_from_slice(&eph_pub_bytes);
    let okm = Zeroizing::new(hkdf_okm(&shared_bytes, HKDF_SALT, &info));

    let key = Key::<Aes256Gcm>::from_slice(&okm[..32]);
    let nonce = Nonce::from_slice(&okm[32..44]);
    let cipher = Aes256Gcm::new(key);

    // padded_plaintext_len = ciphertext.len() - tag (16B)。
    let padded_len = (ciphertext.len() - AES_GCM_TAG_LEN) as u32;
    let aad = build_aad(&purported_stealth_bytes, &eph_pub_bytes, padded_len);

    let padded_plain = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .ok()?;

    // ---- Step 3: ISO 7816-4 strip → SCALE decode ----
    let scale_bytes = strip_iso7816_4(&padded_plain)?;
    let envelope = DmEnvelope::decode(&mut &scale_bytes[..]).ok()?;
    if envelope.version != DM_PROTOCOL_VERSION {
        return None;
    }

    // ---- Step 4: Sr25519 inner signature 検証 ----
    let sig_hash = compute_inner_signed_hash(
        &envelope.sender_account,
        &purported_stealth_bytes,
        &eph_pub_bytes,
        envelope.timestamp_ms,
        &envelope.body,
    );

    let signature_valid = match (
        PublicKey::from_bytes(&envelope.sender_account),
        Signature::from_bytes(&envelope.signature),
    ) {
        (Ok(pk), Ok(sig)) => pk
            .verify(signing_context(SR25519_CONTEXT).bytes(&sig_hash), &sig)
            .is_ok(),
        _ => false,
    };

    Some(DmDecryptedEnvelope {
        sender_main_account: envelope.sender_account,
        timestamp_ms: envelope.timestamp_ms,
        body: envelope.body,
        signature_valid,
    })
}

//! DM media attachment encryption.
//!
//! 各 DM 添付ファイルは独立した K_media (32B random) で AES-256-GCM 暗号化する。
//! K_media は DM 本文 (envelope) に格納され、E2E 担保される。出力フォーマットは
//! `nonce(12B) || ciphertext || tag(16B)` を 1 本の `Vec<u8>` で返す。
//!
//! AAD は使わない (DM 本文側に root が記録され HKDF + AES-GCM で integrity 担保
//! 済みのため、ここでは ciphertext 単体の confidentiality / integrity だけを担う)。

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand_core::{OsRng, RngCore};
use wasm_bindgen::prelude::*;

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

/// 添付ファイル暗号化。`key` は呼出側で `crypto.getRandomValues(32)` 由来の 32B。
///
/// 返り値: `nonce(12) || ciphertext || tag(16)`。
#[wasm_bindgen]
pub fn dm_media_encrypt(key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, JsError> {
    if key.len() != 32 {
        return Err(JsError::new("dm_media_encrypt: key must be 32 bytes"));
    }
    if plaintext.is_empty() {
        return Err(JsError::new("dm_media_encrypt: plaintext must not be empty"));
    }

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| JsError::new("dm_media_encrypt: AES-GCM encrypt failed"))?;

    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// 添付ファイル復号。`blob` は `dm_media_encrypt` の出力 (nonce || ct || tag)。
#[wasm_bindgen]
pub fn dm_media_decrypt(key: &[u8], blob: &[u8]) -> Result<Vec<u8>, JsError> {
    if key.len() != 32 {
        return Err(JsError::new("dm_media_decrypt: key must be 32 bytes"));
    }
    if blob.len() < NONCE_LEN + TAG_LEN {
        return Err(JsError::new("dm_media_decrypt: blob too short"));
    }

    let (nonce_slice, ct_slice) = blob.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(nonce_slice), ct_slice)
        .map_err(|_| JsError::new("dm_media_decrypt: AES-GCM decrypt failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic() {
        let key = [0x42u8; 32];
        let pt = b"hello media".to_vec();
        let ct = dm_media_encrypt(&key, &pt).unwrap();
        assert!(ct.len() >= NONCE_LEN + TAG_LEN + pt.len());
        let out = dm_media_decrypt(&key, &ct).unwrap();
        assert_eq!(out, pt);
    }

    #[test]
    fn nonce_is_unique() {
        let key = [0x42u8; 32];
        let pt = b"same plaintext".to_vec();
        let a = dm_media_encrypt(&key, &pt).unwrap();
        let b = dm_media_encrypt(&key, &pt).unwrap();
        // ランダム nonce のため同一鍵+同一平文でも出力は異なる。
        assert_ne!(&a[..NONCE_LEN], &b[..NONCE_LEN]);
    }

    // エラー分岐 (JsError 構築) は `wasm-pack test` でのみ検証可能。
    // 既存 dm::encrypt と同パターン。
}

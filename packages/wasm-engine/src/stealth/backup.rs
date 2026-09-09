//! Backup encryption and decryption

use super::types::StealthKeyPairJs;
use super::address::format_meta_address;
use super::hash::blake2b_256;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use wasm_bindgen::prelude::*;
use zeroize::Zeroizing;

/// PBKDF2のイテレーション回数
/// (OWASP 推奨の 600,000 回以上。spend/view 鍵を保持するファイルのため)
const PBKDF2_ITERATIONS: u32 = 600_000;

/// バックアップフォーマットバージョン
const BACKUP_VERSION: u8 = 1;

/// AES-GCM nonce 長 (バイト)
const NONCE_LEN: usize = 12;

/// PBKDF2 salt の最小長 (バイト)。encrypt_backup は 16 バイトを生成する。
const MIN_SALT_LEN: usize = 16;

/// 鍵ペアをパスワードで暗号化してバックアップ用バイナリを生成 (Wasm export)
///
/// # Arguments
/// * `spend_key` - Spend秘密鍵 (32 bytes)
/// * `view_key` - View秘密鍵 (32 bytes)
/// * `password` - 暗号化パスワード
///
/// # Returns
/// 暗号化されたバックアップデータ (JSON形式)
#[wasm_bindgen]
pub fn encrypt_backup(
    spend_key: &[u8],
    view_key: &[u8],
    password: &str,
) -> Result<Vec<u8>, JsError> {
    use getrandom::getrandom;

    // 入力検証
    if spend_key.len() != 32 {
        return Err(JsError::new("InvalidSpendKey: must be 32 bytes"));
    }
    if view_key.len() != 32 {
        return Err(JsError::new("InvalidViewKey: must be 32 bytes"));
    }
    if password.is_empty() {
        return Err(JsError::new("Password cannot be empty"));
    }

    // ソルトとノンスを生成
    let mut salt = [0u8; MIN_SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom(&mut salt).map_err(|e| JsError::new(&format!("Random generation failed: {}", e)))?;
    getrandom(&mut nonce_bytes).map_err(|e| JsError::new(&format!("Random generation failed: {}", e)))?;

    // PBKDF2でパスワードから鍵を導出 (Zeroizing: drop 時にゼロクリア)
    let derived_key = derive_key_from_password(password, &salt);

    // ペイロードを作成 (Zeroizing: spend/view 鍵を含むため drop 時にゼロクリア)
    #[cfg(target_arch = "wasm32")]
    let timestamp = js_sys::Date::now() as u64;
    #[cfg(not(target_arch = "wasm32"))]
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut payload = Zeroizing::new(Vec::with_capacity(104));
    payload.extend_from_slice(spend_key);
    payload.extend_from_slice(view_key);
    payload.extend_from_slice(&timestamp.to_le_bytes());

    // チェックサムを追加
    let checksum = blake2b_256(&payload);
    payload.extend_from_slice(&checksum);

    // AES-256-GCMで暗号化
    let cipher = Aes256Gcm::new_from_slice(&*derived_key)
        .map_err(|e| JsError::new(&format!("Cipher initialization failed: {}", e)))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, payload.as_slice())
        .map_err(|e| JsError::new(&format!("Encryption failed: {}", e)))?;

    // バックアップ構造をJSON形式で出力
    let backup = serde_json::json!({
        "version": BACKUP_VERSION,
        "crypto": {
            "algorithm": "AES-256-GCM",
            "kdf": "PBKDF2-SHA256",
            "iterations": PBKDF2_ITERATIONS,
            "salt": base64_encode(&salt),
            "nonce": base64_encode(&nonce_bytes),
        },
        "ciphertext": base64_encode(&ciphertext),
    });

    let json_bytes = serde_json::to_vec(&backup)
        .map_err(|e| JsError::new(&format!("JSON serialization failed: {}", e)))?;

    Ok(json_bytes)
}

/// 暗号化されたバックアップから鍵ペアを復元 (Wasm export)
///
/// # Arguments
/// * `encrypted` - 暗号化されたバックアップデータ
/// * `password` - 復号パスワード
///
/// # Returns
/// 復元された鍵ペア
#[wasm_bindgen]
pub fn decrypt_backup(encrypted: &[u8], password: &str) -> Result<StealthKeyPairJs, JsError> {
    decrypt_backup_impl(encrypted, password).map_err(|e| JsError::new(&e))
}

/// `decrypt_backup` の実体。JsError はホスト (非 wasm) では構築できないため、
/// ユニットテスト可能なように String エラーで返す内部関数に分離している。
fn decrypt_backup_impl(encrypted: &[u8], password: &str) -> Result<StealthKeyPairJs, String> {
    use x25519_dalek::PublicKey as X25519PublicKey;
    use ed25519_dalek::SigningKey;

    // JSONをパース
    let backup: serde_json::Value = serde_json::from_slice(encrypted)
        .map_err(|e| format!("InvalidBackupFormat: {}", e))?;

    // バージョン確認
    let version = backup["version"]
        .as_u64()
        .ok_or_else(|| "InvalidBackupFormat: missing version".to_string())?;
    if version != BACKUP_VERSION as u64 {
        return Err(format!("Unsupported backup version: {}", version));
    }

    // 暗号化パラメータを取得
    let crypto = &backup["crypto"];
    let salt = base64_decode(
        crypto["salt"]
            .as_str()
            .ok_or_else(|| "InvalidBackupFormat: missing salt".to_string())?,
    )?;
    let nonce_bytes = base64_decode(
        crypto["nonce"]
            .as_str()
            .ok_or_else(|| "InvalidBackupFormat: missing nonce".to_string())?,
    )?;
    let ciphertext = base64_decode(
        backup["ciphertext"]
            .as_str()
            .ok_or_else(|| "InvalidBackupFormat: missing ciphertext".to_string())?,
    )?;

    // パラメータ長を検証。`Nonce::from_slice` は長さ不一致で panic し wasm
    // インスタンスごと abort してしまうため、事前に必ずチェックする。
    if nonce_bytes.len() != NONCE_LEN {
        return Err(format!(
            "InvalidBackupFormat: nonce must be {} bytes, got {}",
            NONCE_LEN,
            nonce_bytes.len()
        ));
    }
    if salt.len() < MIN_SALT_LEN {
        return Err(format!(
            "InvalidBackupFormat: salt must be at least {} bytes, got {}",
            MIN_SALT_LEN,
            salt.len()
        ));
    }

    // PBKDF2でパスワードから鍵を導出 (Zeroizing: drop 時にゼロクリア)
    let derived_key = derive_key_from_password(password, &salt);

    // AES-256-GCMで復号
    let cipher = Aes256Gcm::new_from_slice(&*derived_key)
        .map_err(|e| format!("Cipher initialization failed: {}", e))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    // payload は spend/view 鍵を含むため Zeroizing でラップ (drop 時にゼロクリア)
    let payload = Zeroizing::new(
        cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| "DecryptionFailed: incorrect password or corrupted data".to_string())?,
    );

    // ペイロードを検証
    if payload.len() != 104 {
        // 32 + 32 + 8 + 32
        return Err("InvalidBackupFormat: unexpected payload size".to_string());
    }

    let spend_key = &payload[0..32];
    let view_key = &payload[32..64];
    let _timestamp = u64::from_le_bytes(payload[64..72].try_into().unwrap());
    let stored_checksum = &payload[72..104];

    // チェックサムを検証
    let computed_checksum = blake2b_256(&payload[0..72]);
    if stored_checksum != computed_checksum {
        return Err("ChecksumMismatch: backup data is corrupted".to_string());
    }

    // 公開鍵を導出
    // spend_keyはEd25519シード、view_keyはX25519秘密鍵
    let spend_signing_key = SigningKey::from_bytes(
        &<[u8; 32]>::try_from(spend_key)
            .map_err(|_| "InvalidSpendKey".to_string())?
    );
    let spend_pubkey = spend_signing_key.verifying_key().to_bytes();

    let view_secret = x25519_dalek::StaticSecret::from(<[u8; 32]>::try_from(view_key).unwrap());
    let view_pubkey = X25519PublicKey::from(&view_secret);

    // メタアドレスを生成
    let meta_address = format_meta_address(&spend_pubkey, view_pubkey.as_bytes());

    Ok(StealthKeyPairJs::new(
        spend_key.to_vec(),
        view_key.to_vec(),
        spend_pubkey.to_vec(),
        view_pubkey.as_bytes().to_vec(),
        meta_address,
    ))
}

/// PBKDF2-SHA256でパスワードから鍵を導出
///
/// 戻り値は `Zeroizing` でラップされ、drop 時にゼロクリアされる。
fn derive_key_from_password(password: &str, salt: &[u8]) -> Zeroizing<[u8; 32]> {
    use sha2::Sha256;
    use hmac::Hmac;
    use pbkdf2::pbkdf2;

    let mut key = Zeroizing::new([0u8; 32]);
    pbkdf2::<Hmac<Sha256>>(
        password.as_bytes(),
        salt,
        PBKDF2_ITERATIONS,
        &mut *key,
    ).expect("PBKDF2 should not fail with valid inputs");
    key
}

/// Base64エンコード
fn base64_encode(data: &[u8]) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD.encode(data)
}

/// Base64デコード
fn base64_decode(data: &str) -> Result<Vec<u8>, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD
        .decode(data)
        .map_err(|e| format!("Base64 decode error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 有効なバックアップ JSON を生成し、指定フィールドを差し替えるヘルパー
    fn make_tampered_backup(field: &str, value: serde_json::Value) -> Vec<u8> {
        let spend_key = [0x11u8; 32];
        let view_key = [0x22u8; 32];
        let encrypted = encrypt_backup(&spend_key, &view_key, "password").unwrap();

        let mut backup: serde_json::Value = serde_json::from_slice(&encrypted).unwrap();
        backup["crypto"][field] = value;
        serde_json::to_vec(&backup).unwrap()
    }

    #[test]
    fn decrypt_backup_with_malformed_nonce_returns_err_not_panic() {
        // nonce が 12 バイトでない (8 バイト) backup → panic ではなく Err
        let tampered = make_tampered_backup("nonce", serde_json::json!(base64_encode(&[0u8; 8])));
        let result = decrypt_backup_impl(&tampered, "password");
        let err = result.err().expect("malformed nonce must yield Err");
        assert!(err.contains("nonce"), "error should mention nonce: {}", err);
    }

    #[test]
    fn decrypt_backup_with_short_salt_returns_err_not_panic() {
        // salt が 16 バイト未満 → Err
        let tampered = make_tampered_backup("salt", serde_json::json!(base64_encode(&[0u8; 4])));
        let result = decrypt_backup_impl(&tampered, "password");
        let err = result.err().expect("short salt must yield Err");
        assert!(err.contains("salt"), "error should mention salt: {}", err);
    }

    #[test]
    fn decrypt_backup_roundtrip_via_impl() {
        let spend_key = [0x33u8; 32];
        let view_key = [0x44u8; 32];
        let encrypted = encrypt_backup(&spend_key, &view_key, "password").unwrap();

        let restored = decrypt_backup_impl(&encrypted, "password").unwrap();
        assert_eq!(restored.spend_key(), spend_key.to_vec());
        assert_eq!(restored.view_key(), view_key.to_vec());

        // 間違ったパスワードは Err (JsError を経由しないので host でも検証可能)
        assert!(decrypt_backup_impl(&encrypted, "wrong").is_err());
    }
}

//! Backup encryption and decryption

use super::types::StealthKeyPairJs;
use super::address::format_meta_address;
use super::hash::blake2b_256;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use wasm_bindgen::prelude::*;

/// PBKDF2のイテレーション回数
const PBKDF2_ITERATIONS: u32 = 100_000;

/// バックアップフォーマットバージョン
const BACKUP_VERSION: u8 = 1;

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
    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];
    getrandom(&mut salt).map_err(|e| JsError::new(&format!("Random generation failed: {}", e)))?;
    getrandom(&mut nonce_bytes).map_err(|e| JsError::new(&format!("Random generation failed: {}", e)))?;

    // PBKDF2でパスワードから鍵を導出
    let derived_key = derive_key_from_password(password, &salt);

    // ペイロードを作成
    #[cfg(target_arch = "wasm32")]
    let timestamp = js_sys::Date::now() as u64;
    #[cfg(not(target_arch = "wasm32"))]
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut payload = Vec::with_capacity(72);
    payload.extend_from_slice(spend_key);
    payload.extend_from_slice(view_key);
    payload.extend_from_slice(&timestamp.to_le_bytes());

    // チェックサムを追加
    let checksum = blake2b_256(&payload);
    payload.extend_from_slice(&checksum);

    // AES-256-GCMで暗号化
    let cipher = Aes256Gcm::new_from_slice(&derived_key)
        .map_err(|e| JsError::new(&format!("Cipher initialization failed: {}", e)))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, payload.as_ref())
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
    use x25519_dalek::PublicKey as X25519PublicKey;
    use ed25519_dalek::SigningKey;

    // JSONをパース
    let backup: serde_json::Value = serde_json::from_slice(encrypted)
        .map_err(|e| JsError::new(&format!("InvalidBackupFormat: {}", e)))?;

    // バージョン確認
    let version = backup["version"]
        .as_u64()
        .ok_or_else(|| JsError::new("InvalidBackupFormat: missing version"))?;
    if version != BACKUP_VERSION as u64 {
        return Err(JsError::new(&format!(
            "Unsupported backup version: {}",
            version
        )));
    }

    // 暗号化パラメータを取得
    let crypto = &backup["crypto"];
    let salt = base64_decode(
        crypto["salt"]
            .as_str()
            .ok_or_else(|| JsError::new("InvalidBackupFormat: missing salt"))?,
    )?;
    let nonce_bytes = base64_decode(
        crypto["nonce"]
            .as_str()
            .ok_or_else(|| JsError::new("InvalidBackupFormat: missing nonce"))?,
    )?;
    let ciphertext = base64_decode(
        backup["ciphertext"]
            .as_str()
            .ok_or_else(|| JsError::new("InvalidBackupFormat: missing ciphertext"))?,
    )?;

    // PBKDF2でパスワードから鍵を導出
    let derived_key = derive_key_from_password(password, &salt);

    // AES-256-GCMで復号
    let cipher = Aes256Gcm::new_from_slice(&derived_key)
        .map_err(|e| JsError::new(&format!("Cipher initialization failed: {}", e)))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let payload = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| JsError::new("DecryptionFailed: incorrect password or corrupted data"))?;

    // ペイロードを検証
    if payload.len() != 104 {
        // 32 + 32 + 8 + 32
        return Err(JsError::new("InvalidBackupFormat: unexpected payload size"));
    }

    let spend_key = &payload[0..32];
    let view_key = &payload[32..64];
    let _timestamp = u64::from_le_bytes(payload[64..72].try_into().unwrap());
    let stored_checksum = &payload[72..104];

    // チェックサムを検証
    let computed_checksum = blake2b_256(&payload[0..72]);
    if stored_checksum != computed_checksum {
        return Err(JsError::new("ChecksumMismatch: backup data is corrupted"));
    }

    // 公開鍵を導出
    // spend_keyはEd25519シード、view_keyはX25519秘密鍵
    let spend_signing_key = SigningKey::from_bytes(
        &<[u8; 32]>::try_from(spend_key)
            .map_err(|_| JsError::new("InvalidSpendKey"))?
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
fn derive_key_from_password(password: &str, salt: &[u8]) -> [u8; 32] {
    use sha2::Sha256;
    use hmac::Hmac;
    use pbkdf2::pbkdf2;
    
    let mut key = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(
        password.as_bytes(),
        salt,
        PBKDF2_ITERATIONS,
        &mut key,
    ).expect("PBKDF2 should not fail with valid inputs");
    key
}

/// Base64エンコード
fn base64_encode(data: &[u8]) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD.encode(data)
}

/// Base64デコード
fn base64_decode(data: &str) -> Result<Vec<u8>, JsError> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD
        .decode(data)
        .map_err(|e| JsError::new(&format!("Base64 decode error: {}", e)))
}

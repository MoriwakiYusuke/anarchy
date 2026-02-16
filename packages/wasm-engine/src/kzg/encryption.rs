//! AES-256-GCM Encryption Module
//!
//! 投稿データの暗号化・復号を提供。
//! ハイブリッドアプローチの第1段階として、平文を暗号化する。

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use getrandom::getrandom;

/// AES-256-GCM鍵のバイト長
pub const KEY_SIZE: usize = 32;

/// AES-GCM NonceのバイトLength (96-bit)
pub const NONCE_SIZE: usize = 12;

/// 暗号化エラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptionError {
    /// 鍵サイズが不正
    InvalidKeySize,
    /// 暗号化失敗
    EncryptionFailed,
    /// 復号失敗
    DecryptionFailed,
    /// 暗号文が短すぎる（Nonceを含まない）
    CiphertextTooShort,
    /// ランダム生成失敗
    RandomGenerationFailed,
}

impl core::fmt::Display for EncryptionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EncryptionError::InvalidKeySize => {
                write!(f, "Invalid key size: expected {} bytes", KEY_SIZE)
            }
            EncryptionError::EncryptionFailed => write!(f, "Encryption failed"),
            EncryptionError::DecryptionFailed => write!(f, "Decryption failed: invalid ciphertext or key"),
            EncryptionError::CiphertextTooShort => {
                write!(f, "Ciphertext too short: must include nonce ({} bytes)", NONCE_SIZE)
            }
            EncryptionError::RandomGenerationFailed => write!(f, "Random number generation failed"),
        }
    }
}

/// Generate a cryptographically secure random encryption key.
///
/// # Security Properties
/// - Uses the platform's CSPRNG via `getrandom` crate
/// - Produces 256 bits of entropy suitable for AES-256-GCM
/// - Each call generates an independent, unpredictable key
///
/// # Usage Guidelines
/// - Generate a **unique key for each post**
/// - **Never reuse keys** across different posts or encryption operations
/// - Keys should be immediately split via VSS and discarded from memory
/// - The key is used with [`encrypt`] for AES-256-GCM authenticated encryption
///
/// # Returns
/// 32-byte cryptographically secure random key
pub fn generate_key() -> Result<[u8; KEY_SIZE], EncryptionError> {
    let mut key = [0u8; KEY_SIZE];
    getrandom(&mut key).map_err(|_| EncryptionError::RandomGenerationFailed)?;
    Ok(key)
}

/// AES-256-GCMでデータを暗号化
///
/// # Arguments
/// * `plaintext` - 暗号化する平文
/// * `key` - 32バイトのAES-256鍵
///
/// # Returns
/// `nonce || ciphertext || tag` 形式の暗号文
/// （12バイトnonce + 暗号文 + 16バイトタグ）
pub fn encrypt(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>, EncryptionError> {
    if key.len() != KEY_SIZE {
        return Err(EncryptionError::InvalidKeySize);
    }

    // ランダムnonceを生成
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    getrandom(&mut nonce_bytes).map_err(|_| EncryptionError::RandomGenerationFailed)?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    // 暗号化
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| EncryptionError::InvalidKeySize)?;
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| EncryptionError::EncryptionFailed)?;

    // nonce || ciphertext の形式で返す
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// AES-256-GCMでデータを復号
///
/// # Arguments
/// * `ciphertext` - `nonce || ciphertext || tag` 形式の暗号文
/// * `key` - 32バイトのAES-256鍵
///
/// # Returns
/// 復号された平文
pub fn decrypt(ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>, EncryptionError> {
    if key.len() != KEY_SIZE {
        return Err(EncryptionError::InvalidKeySize);
    }

    if ciphertext.len() < NONCE_SIZE {
        return Err(EncryptionError::CiphertextTooShort);
    }

    // nonceと暗号文を分離
    let (nonce_bytes, encrypted) = ciphertext.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    // 復号
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| EncryptionError::InvalidKeySize)?;
    let plaintext = cipher
        .decrypt(nonce, encrypted)
        .map_err(|_| EncryptionError::DecryptionFailed)?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_key() {
        let key1 = generate_key().unwrap();
        let key2 = generate_key().unwrap();

        assert_eq!(key1.len(), KEY_SIZE);
        assert_eq!(key2.len(), KEY_SIZE);
        // ランダム鍵は異なるはず
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_key().unwrap();
        let plaintext = b"Hello, Anarchy!";

        let ciphertext = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&ciphertext, &key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let key = generate_key().unwrap();
        let plaintext = b"";

        let ciphertext = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&ciphertext, &key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_large_data() {
        let key = generate_key().unwrap();
        let plaintext: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();

        let ciphertext = encrypt(&plaintext, &key).unwrap();
        let decrypted = decrypt(&ciphertext, &key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_invalid_key_size() {
        let short_key = [0u8; 16];
        let plaintext = b"test";

        let result = encrypt(plaintext, &short_key);
        assert_eq!(result, Err(EncryptionError::InvalidKeySize));
    }

    #[test]
    fn test_decrypt_wrong_key() {
        let key1 = generate_key().unwrap();
        let key2 = generate_key().unwrap();
        let plaintext = b"secret message";

        let ciphertext = encrypt(plaintext, &key1).unwrap();
        let result = decrypt(&ciphertext, &key2);

        assert_eq!(result, Err(EncryptionError::DecryptionFailed));
    }

    #[test]
    fn test_ciphertext_too_short() {
        let key = generate_key().unwrap();
        let short_ciphertext = [0u8; 5]; // < NONCE_SIZE

        let result = decrypt(&short_ciphertext, &key);
        assert_eq!(result, Err(EncryptionError::CiphertextTooShort));
    }

    #[test]
    fn test_tampered_ciphertext() {
        let key = generate_key().unwrap();
        let plaintext = b"sensitive data";

        let mut ciphertext = encrypt(plaintext, &key).unwrap();
        // タンパリング: 最後のバイトを変更
        if let Some(last) = ciphertext.last_mut() {
            *last ^= 0xFF;
        }

        let result = decrypt(&ciphertext, &key);
        assert_eq!(result, Err(EncryptionError::DecryptionFailed));
    }

    #[test]
    fn test_ciphertext_format() {
        let key = generate_key().unwrap();
        let plaintext = b"test";

        let ciphertext = encrypt(plaintext, &key).unwrap();
        // nonce (12) + plaintext (4) + tag (16) = 32
        assert_eq!(ciphertext.len(), NONCE_SIZE + plaintext.len() + 16);
    }
}

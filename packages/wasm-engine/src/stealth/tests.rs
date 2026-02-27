//! Unit tests for stealth module

use super::*;

#[test]
fn test_generate_stealth_keys() {
    let keys = generate_stealth_keys();
    
    // 鍵長を確認
    assert_eq!(keys.spend_key().len(), 32);
    assert_eq!(keys.view_key().len(), 32);
    assert_eq!(keys.spend_pubkey().len(), 32);
    assert_eq!(keys.view_pubkey().len(), 32);
    
    // メタアドレス形式を確認
    assert!(keys.meta_address().starts_with("st:anarchy:"));
}

#[test]
fn test_format_and_parse_meta_address() {
    let spend_pubkey = [1u8; 32];
    let view_pubkey = [2u8; 32];
    
    let meta_address = format_meta_address(&spend_pubkey, &view_pubkey);
    assert!(meta_address.starts_with("st:anarchy:"));
    
    let parts = parse_meta_address(&meta_address).unwrap();
    assert_eq!(parts.spend_pubkey(), spend_pubkey.to_vec());
    assert_eq!(parts.view_pubkey(), view_pubkey.to_vec());
}

#[test]
fn test_derive_stealth_address() {
    // 鍵ペアを生成
    let keys = generate_stealth_keys();
    
    // ステルスアドレスを導出
    let result = derive_stealth_address(&keys.meta_address()).unwrap();
    
    // ステルスアドレスがSS58形式であることを確認
    assert!(!result.stealth_address().is_empty());
    
    // エフェメラル公開鍵が32バイトであることを確認
    assert_eq!(result.ephemeral_pubkey().len(), 32);
}

#[test]
fn test_scan_transaction_detects_own() {
    // 鍵ペアを生成
    let keys = generate_stealth_keys();
    
    // ステルスアドレスを導出
    let result = derive_stealth_address(&keys.meta_address()).unwrap();
    
    // 自分宛トランザクションをスキャン
    let is_ours = scan_transaction(
        &keys.view_key(),
        &result.ephemeral_pubkey(),
        &result.stealth_address(),
        &keys.spend_pubkey(),
    );
    
    assert!(is_ours, "Should detect own transaction");
}

#[test]
fn test_scan_transaction_rejects_others() {
    // 受信者の鍵ペアを生成
    let recipient_keys = generate_stealth_keys();
    
    // 別のユーザーの鍵ペアを生成
    let other_keys = generate_stealth_keys();
    
    // 受信者宛テルスアドレスを導出
    let result = derive_stealth_address(&recipient_keys.meta_address()).unwrap();
    
    // 別のユーザーがスキャン
    let is_ours = scan_transaction(
        &other_keys.view_key(),
        &result.ephemeral_pubkey(),
        &result.stealth_address(),
        &other_keys.spend_pubkey(),
    );
    
    assert!(!is_ours, "Should not detect other's transaction");
}

#[test]
fn test_encrypt_decrypt_backup() {
    let keys = generate_stealth_keys();
    let password = "test-password-123";
    
    // バックアップを暗号化
    let encrypted = encrypt_backup(
        &keys.spend_key(),
        &keys.view_key(),
        password,
    ).unwrap();
    
    // バックアップを復号
    let restored = decrypt_backup(&encrypted, password).unwrap();
    
    // 鍵が一致することを確認
    assert_eq!(keys.spend_key(), restored.spend_key());
    assert_eq!(keys.view_key(), restored.view_key());
    assert_eq!(keys.meta_address(), restored.meta_address());
}

#[test]
#[cfg(target_arch = "wasm32")]
fn test_decrypt_backup_wrong_password() {
    let keys = generate_stealth_keys();
    
    let encrypted = encrypt_backup(
        &keys.spend_key(),
        &keys.view_key(),
        "correct-password",
    ).unwrap();
    
    // 間違ったパスワードで復号を試みる
    let result = decrypt_backup(&encrypted, "wrong-password");
    
    assert!(result.is_err(), "Should fail with wrong password");
}

#[test]
fn test_derive_stealth_private_key() {
    let keys = generate_stealth_keys();
    
    // ステルスアドレスを導出
    let result = derive_stealth_address(&keys.meta_address()).unwrap();
    
    // ステルス秘密鍵を導出
    let stealth_private_key = derive_stealth_private_key(
        &keys.spend_key(),
        &keys.view_key(),
        &result.ephemeral_pubkey(),
    ).unwrap();
    
    // 秘密鍵が32バイトであることを確認
    assert_eq!(stealth_private_key.len(), 32);
}

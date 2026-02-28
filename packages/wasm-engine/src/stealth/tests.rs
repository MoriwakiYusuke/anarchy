//! Unit tests for stealth module

use super::*;

/// SS58チェックサムを検証するヘルパー関数
/// polkadot-apiと同じ方式でチェックサムを検証
fn verify_ss58_checksum(address: &str) -> bool {
    use blake2::{Blake2b512, Digest};
    
    // Base58デコード
    let decoded = match bs58::decode(address).into_vec() {
        Ok(d) => d,
        Err(_) => return false,
    };
    
    // 最小長チェック (prefix 1byte + pubkey 32bytes + checksum 2bytes = 35)
    if decoded.len() < 35 {
        return false;
    }
    
    // チェックサム検証
    let payload_len = decoded.len() - 2;
    let payload = &decoded[..payload_len];
    let checksum = &decoded[payload_len..];
    
    // Blake2b-512でハッシュ
    let mut hasher = Blake2b512::new();
    hasher.update(b"SS58PRE");
    hasher.update(payload);
    let hash = hasher.finalize();
    
    // 最初の2バイトがチェックサムと一致するか確認
    hash[0] == checksum[0] && hash[1] == checksum[1]
}

/// SS58アドレスから公開鍵を抽出
fn extract_pubkey_from_ss58(address: &str) -> Option<[u8; 32]> {
    let decoded = bs58::decode(address).into_vec().ok()?;
    
    // prefix(1) + pubkey(32) + checksum(2) = 35
    if decoded.len() != 35 {
        return None;
    }
    
    let mut pubkey = [0u8; 32];
    pubkey.copy_from_slice(&decoded[1..33]);
    Some(pubkey)
}

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

/// Test that meta_address contains the correct pubkeys from generated keys
#[test]
fn test_meta_address_contains_correct_pubkeys() {
    let keys = generate_stealth_keys();
    
    // Parse the meta_address
    let parts = parse_meta_address(&keys.meta_address()).unwrap();
    
    // Verify spend_pubkey matches
    assert_eq!(
        parts.spend_pubkey(),
        keys.spend_pubkey(),
        "spend_pubkey in meta_address must match the generated spend_pubkey"
    );
    
    // Verify view_pubkey matches  
    assert_eq!(
        parts.view_pubkey(),
        keys.view_pubkey(),
        "view_pubkey in meta_address must match the generated view_pubkey"
    );
    
    println!("Generated spend_pubkey (first 8): {:?}", &keys.spend_pubkey()[0..8]);
    println!("Parsed spend_pubkey (first 8): {:?}", &parts.spend_pubkey()[0..8]);
    println!("Generated view_pubkey (first 8): {:?}", &keys.view_pubkey()[0..8]);
    println!("Parsed view_pubkey (first 8): {:?}", &parts.view_pubkey()[0..8]);
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
    
    // ステルス公開鍵が32バイトであることを確認
    assert_eq!(result.stealth_pubkey().len(), 32);
}

#[test]
fn test_ss58_checksum_validity() {
    // 複数の鍵ペアでテスト
    for _ in 0..10 {
        let keys = generate_stealth_keys();
        let result = derive_stealth_address(&keys.meta_address()).unwrap();
        
        let address = result.stealth_address();
        
        // SS58アドレスが正しい形式であることを確認
        assert!(
            address.starts_with('5'),
            "SS58 address with prefix 42 should start with '5', got: {}",
            address
        );
        
        // チェックサムが正しいことを確認
        assert!(
            verify_ss58_checksum(&address),
            "SS58 checksum should be valid for address: {}",
            address
        );
        
        // SS58アドレスから公開鍵を抽出し、stealth_pubkeyと一致することを確認
        let extracted_pubkey = extract_pubkey_from_ss58(&address)
            .expect("Should be able to extract pubkey from SS58 address");
        assert_eq!(
            extracted_pubkey.to_vec(),
            result.stealth_pubkey(),
            "Extracted pubkey should match stealth_pubkey"
        );
    }
}

#[test]
fn test_ss58_address_length() {
    let keys = generate_stealth_keys();
    let result = derive_stealth_address(&keys.meta_address()).unwrap();
    
    let address = result.stealth_address();
    
    // SS58アドレスの長さ確認 (prefix 42, 32バイト公開鍵の場合は47-48文字)
    assert!(
        address.len() >= 47 && address.len() <= 48,
        "SS58 address length should be 47-48, got: {} (address: {})",
        address.len(),
        address
    );
}

#[test]
fn test_stealth_pubkey_matches_address() {
    let keys = generate_stealth_keys();
    let result = derive_stealth_address(&keys.meta_address()).unwrap();
    
    // stealth_pubkeyからSS58アドレスを再生成
    let pubkey: [u8; 32] = result.stealth_pubkey().try_into().unwrap();
    let regenerated_address = {
        use blake2::{Blake2b512, Digest};
        
        const SS58_PREFIX: u8 = 42;
        let mut payload = Vec::with_capacity(35);
        payload.push(SS58_PREFIX);
        payload.extend_from_slice(&pubkey);
        
        let mut hasher = Blake2b512::new();
        hasher.update(b"SS58PRE");
        hasher.update(&payload);
        let hash = hasher.finalize();
        
        payload.extend_from_slice(&hash[0..2]);
        bs58::encode(payload).into_string()
    };
    
    assert_eq!(
        result.stealth_address(),
        regenerated_address,
        "Regenerated SS58 address should match original"
    );
}

#[test]
fn test_scan_transaction_detects_own() {
    // 鍵ペアを生成
    let keys = generate_stealth_keys();
    
    // ステルスアドレスを導出
    let result = derive_stealth_address(&keys.meta_address()).unwrap();
    
    // 自分宛トランザクションをスキャン（pubkeyバイト列を直接比較）
    let is_ours = scan_transaction(
        &keys.view_key(),
        &result.ephemeral_pubkey(),
        &result.stealth_pubkey(),
        &keys.spend_pubkey(),
    );
    
    assert!(is_ours, "Should detect own transaction");
}

/// Test ECDH commutativity explicitly
/// This verifies that sender and receiver compute the same shared secret
#[test]
fn test_ecdh_commutativity() {
    use x25519_dalek::{PublicKey, StaticSecret};
    use super::hash::blake2b_256;
    
    // Generate receiver's key pair (view_key, view_pubkey)
    let view_secret = StaticSecret::random_from_rng(rand_core::OsRng);
    let view_pubkey = PublicKey::from(&view_secret);
    
    // Generate sender's ephemeral key pair
    let ephemeral_secret = StaticSecret::random_from_rng(rand_core::OsRng);
    let ephemeral_pubkey = PublicKey::from(&ephemeral_secret);
    
    // Sender computes: s = ephemeral_secret * view_pubkey
    let sender_shared = ephemeral_secret.diffie_hellman(&view_pubkey);
    
    // Receiver computes: s = view_secret * ephemeral_pubkey
    let receiver_shared = view_secret.diffie_hellman(&ephemeral_pubkey);
    
    // Both should be equal
    assert_eq!(
        sender_shared.as_bytes(),
        receiver_shared.as_bytes(),
        "ECDH should be commutative"
    );
    
    // Hash should also be equal
    let sender_hash = blake2b_256(sender_shared.as_bytes());
    let receiver_hash = blake2b_256(receiver_shared.as_bytes());
    
    assert_eq!(sender_hash, receiver_hash, "Hash of shared secret should match");
}

/// End-to-end test simulating the exact frontend flow
#[test]
fn test_e2e_stealth_flow_with_ss58() {
    use bs58;
    use blake2::{Blake2b512, Digest};
    
    // Step 1: Receiver generates stealth keys
    let receiver_keys = generate_stealth_keys();
    let meta_address = receiver_keys.meta_address();
    
    println!("Receiver meta_address: {}", meta_address);
    println!("Receiver view_key (first 8): {:?}", &receiver_keys.view_key()[0..8]);
    println!("Receiver spend_pubkey (first 8): {:?}", &receiver_keys.spend_pubkey()[0..8]);
    
    // Step 2: Sender derives stealth address from meta_address
    let derived = derive_stealth_address(&meta_address).unwrap();
    let ephemeral_pubkey = derived.ephemeral_pubkey();
    let stealth_pubkey = derived.stealth_pubkey();
    let stealth_address_wasm = derived.stealth_address();
    
    println!("Derived ephemeral_pubkey (first 8): {:?}", &ephemeral_pubkey[0..8]);
    println!("Derived stealth_pubkey (first 8): {:?}", &stealth_pubkey[0..8]);
    println!("Derived stealth_address (wasm): {}", stealth_address_wasm);
    
    // Step 3: Simulate frontend SS58 encoding (like polkadot-api does)
    // This mirrors: fromBufferToBase58(42)(stealthPubkey)
    fn encode_ss58(pubkey: &[u8]) -> String {
        const SS58_PREFIX: u8 = 42;
        let mut payload = Vec::with_capacity(35);
        payload.push(SS58_PREFIX);
        payload.extend_from_slice(pubkey);
        
        let mut hasher = Blake2b512::new();
        hasher.update(b"SS58PRE");
        hasher.update(&payload);
        let hash = hasher.finalize();
        
        payload.extend_from_slice(&hash[0..2]);
        bs58::encode(payload).into_string()
    }
    
    let frontend_stealth_address = encode_ss58(&stealth_pubkey);
    println!("Frontend-style SS58 address: {}", frontend_stealth_address);
    
    // Verify addresses match
    assert_eq!(
        stealth_address_wasm, frontend_stealth_address,
        "Wasm and frontend SS58 encoding should match"
    );
    
    // Step 4: Simulate SS58 decode (like getSs58AddressInfo does)
    fn decode_ss58(address: &str) -> Option<[u8; 32]> {
        let decoded = bs58::decode(address).into_vec().ok()?;
        if decoded.len() != 35 {
            return None;
        }
        // Verify checksum
        let payload = &decoded[..33];
        let checksum = &decoded[33..35];
        
        let mut hasher = Blake2b512::new();
        hasher.update(b"SS58PRE");
        hasher.update(payload);
        let hash = hasher.finalize();
        
        if hash[0] != checksum[0] || hash[1] != checksum[1] {
            return None;
        }
        
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&decoded[1..33]);
        Some(pubkey)
    }
    
    let decoded_pubkey = decode_ss58(&frontend_stealth_address)
        .expect("SS58 decode should succeed");
    
    println!("Decoded stealth_pubkey (first 8): {:?}", &decoded_pubkey[0..8]);
    
    // Verify round-trip preserves bytes
    assert_eq!(
        &decoded_pubkey[..], &stealth_pubkey[..],
        "SS58 round-trip should preserve pubkey bytes"
    );
    
    // Step 5: Scan the transaction using decoded pubkey
    let is_ours = scan_transaction(
        &receiver_keys.view_key(),
        &ephemeral_pubkey,
        &decoded_pubkey,  // Use decoded pubkey (as scanner does)
        &receiver_keys.spend_pubkey(),
    );
    
    println!("scan_transaction result: {}", is_ours);
    
    assert!(is_ours, "Should detect own transaction through full SS58 round-trip");
}

#[test]
fn test_scan_transaction_rejects_others() {
    // 受信者の鍵ペアを生成
    let recipient_keys = generate_stealth_keys();
    
    // 別のユーザーの鍵ペアを生成
    let other_keys = generate_stealth_keys();
    
    // 受信者宛テルスアドレスを導出
    let result = derive_stealth_address(&recipient_keys.meta_address()).unwrap();
    
    // 別のユーザーがスキャン（pubkeyバイト列を直接比較）
    let is_ours = scan_transaction(
        &other_keys.view_key(),
        &result.ephemeral_pubkey(),
        &result.stealth_pubkey(),
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

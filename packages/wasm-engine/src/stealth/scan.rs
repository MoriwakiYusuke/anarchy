//! Transaction scanning for stealth address detection

use super::hash::blake2b_256;
use super::address::derive_stealth_pubkey;
use wasm_bindgen::prelude::*;
use x25519_dalek::{PublicKey, StaticSecret};

/// View鍵を使ってトランザクションが自分宛かどうかを判定 (Wasm export)
///
/// # Arguments
/// * `view_key` - 自分のView秘密鍵 (32 bytes)
/// * `ephemeral_pubkey` - トランザクションのエフェメラル公開鍵 (32 bytes)
/// * `stealth_address` - トランザクションの宛先ステルスアドレス (SS58)
/// * `spend_pubkey` - 自分のSpend公開鍵 (32 bytes)
///
/// # Returns
/// `true` if the transaction is addressed to us, `false` otherwise
#[wasm_bindgen]
pub fn scan_transaction(
    view_key: &[u8],
    ephemeral_pubkey: &[u8],
    stealth_address: &str,
    spend_pubkey: &[u8],
) -> bool {
    // 入力検証
    if view_key.len() != 32 || ephemeral_pubkey.len() != 32 || spend_pubkey.len() != 32 {
        return false;
    }

    // View秘密鍵とエフェメラル公開鍵から共有シークレットを計算
    let view_secret = StaticSecret::from(<[u8; 32]>::try_from(view_key).unwrap());
    let ephemeral_pub = PublicKey::from(<[u8; 32]>::try_from(ephemeral_pubkey).unwrap());
    let shared_secret = view_secret.diffie_hellman(&ephemeral_pub);

    // ハッシュ化: h = H(s)
    let h = blake2b_256(shared_secret.as_bytes());

    // 期待されるステルス公開鍵を計算
    let expected_stealth_pubkey = derive_stealth_pubkey(spend_pubkey, &h);

    // SS58アドレスに変換
    let expected_address = pubkey_to_ss58(&expected_stealth_pubkey);

    // アドレスを比較
    expected_address == stealth_address
}

/// 公開鍵をSS58アドレスに変換 (address.rsと同じ実装)
fn pubkey_to_ss58(pubkey: &[u8; 32]) -> String {
    const SS58_PREFIX: u8 = 42;
    
    let mut payload = Vec::with_capacity(35);
    payload.push(SS58_PREFIX);
    payload.extend_from_slice(pubkey);
    
    use blake2::{Blake2b512, Digest};
    let mut hasher = Blake2b512::new();
    hasher.update(b"SS58PRE");
    hasher.update(&payload);
    let hash = hasher.finalize();
    
    payload.extend_from_slice(&hash[0..2]);
    
    bs58::encode(payload).into_string()
}

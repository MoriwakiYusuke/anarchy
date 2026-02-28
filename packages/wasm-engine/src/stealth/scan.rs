//! Transaction scanning for stealth address detection

use super::hash::blake2b_256;
use wasm_bindgen::prelude::*;
use x25519_dalek::{PublicKey, StaticSecret};
use curve25519_dalek::{edwards::CompressedEdwardsY, Scalar};
use curve25519_dalek::constants::ED25519_BASEPOINT_TABLE;

#[cfg(target_arch = "wasm32")]
use web_sys::console;

/// View鍵を使ってトランザクションが自分宛かどうかを判定 (Wasm export)
///
/// # Arguments
/// * `view_key` - 自分のView秘密鍵 (32 bytes)
/// * `ephemeral_pubkey` - トランザクションのエフェメラル公開鍵 (32 bytes)
/// * `stealth_pubkey` - トランザクションの宛先ステルス公開鍵 (32 bytes)
/// * `spend_pubkey` - 自分のSpend公開鍵 (32 bytes)
///
/// # Returns
/// `true` if the transaction is addressed to us, `false` otherwise
#[wasm_bindgen]
pub fn scan_transaction(
    view_key: &[u8],
    ephemeral_pubkey: &[u8],
    stealth_pubkey: &[u8],
    spend_pubkey: &[u8],
) -> bool {
    // 入力検証
    if view_key.len() != 32 || ephemeral_pubkey.len() != 32 || stealth_pubkey.len() != 32 || spend_pubkey.len() != 32 {
        #[cfg(target_arch = "wasm32")]
        console::log_1(&"[scan_transaction] Invalid input lengths".into());
        return false;
    }

    // View秘密鍵とエフェメラル公開鍵から共有シークレットを計算
    let view_secret = StaticSecret::from(<[u8; 32]>::try_from(view_key).unwrap());
    let ephemeral_pub = PublicKey::from(<[u8; 32]>::try_from(ephemeral_pubkey).unwrap());
    let shared_secret = view_secret.diffie_hellman(&ephemeral_pub);

    // ハッシュ化: h = H(s)
    let h = blake2b_256(shared_secret.as_bytes());

    // 期待されるステルス公開鍵を計算: P_stealth = K_spend + H(s)*G
    let spend_bytes: [u8; 32] = spend_pubkey.try_into().unwrap();
    let spend_point = match CompressedEdwardsY(spend_bytes).decompress() {
        Some(p) => p,
        None => {
            #[cfg(target_arch = "wasm32")]
            console::log_1(&"[scan_transaction] Invalid spend pubkey point".into());
            return false;
        }
    };
    let h_scalar = Scalar::from_bytes_mod_order(h);
    let h_g = ED25519_BASEPOINT_TABLE * &h_scalar;
    let expected_point = spend_point + h_g;
    let expected_stealth_pubkey: [u8; 32] = expected_point.compress().to_bytes();

    // 公開鍵のバイト列を直接比較（SS58エンコーディングの差異を回避）
    let matches = expected_stealth_pubkey == stealth_pubkey;
    
    // Debug output (only in wasm)
    #[cfg(target_arch = "wasm32")]
    if !matches {
        console::log_1(&format!(
            "[scan_transaction] MISMATCH: expected first 8 bytes: {:?}, got: {:?}",
            &expected_stealth_pubkey[..8],
            &stealth_pubkey[..8]
        ).into());
    }
    
    matches
}

/// Debug: derive expected stealth pubkey without comparing (Wasm export)
/// Used to verify the derivation matches the stored stealth address
#[wasm_bindgen]
pub fn debug_derive_expected_pubkey(
    view_key: &[u8],
    ephemeral_pubkey: &[u8],
    spend_pubkey: &[u8],
) -> Option<Vec<u8>> {
    if view_key.len() != 32 || ephemeral_pubkey.len() != 32 || spend_pubkey.len() != 32 {
        return None;
    }

    let view_secret = StaticSecret::from(<[u8; 32]>::try_from(view_key).unwrap());
    let ephemeral_pub = PublicKey::from(<[u8; 32]>::try_from(ephemeral_pubkey).unwrap());
    let shared_secret = view_secret.diffie_hellman(&ephemeral_pub);
    let h = blake2b_256(shared_secret.as_bytes());
    
    // P_stealth = K_spend + H(s)*G
    let spend_bytes: [u8; 32] = spend_pubkey.try_into().ok()?;
    let spend_point = CompressedEdwardsY(spend_bytes).decompress()?;
    let h_scalar = Scalar::from_bytes_mod_order(h);
    let h_g = ED25519_BASEPOINT_TABLE * &h_scalar;
    let expected_point = spend_point + h_g;
    let expected_stealth_pubkey: [u8; 32] = expected_point.compress().to_bytes();
    
    Some(expected_stealth_pubkey.to_vec())
}

//! EIP-5564 stealth address derivation

use super::hash::blake2b_256;
use super::types::{MetaAddressParts, StealthAddressResult};
use wasm_bindgen::prelude::*;
use x25519_dalek::{PublicKey, StaticSecret};
use rand_core::OsRng;

/// メタアドレスのプレフィックス
const META_ADDRESS_PREFIX: &str = "st:anarchy:";

/// メタアドレス文字列を生成
pub fn format_meta_address(spend_pubkey: &[u8], view_pubkey: &[u8]) -> String {
    // spend_pubkey || view_pubkey を結合してBase58エンコード
    let mut combined = Vec::with_capacity(64);
    combined.extend_from_slice(spend_pubkey);
    combined.extend_from_slice(view_pubkey);
    
    let encoded = bs58::encode(&combined).into_string();
    format!("{}{}", META_ADDRESS_PREFIX, encoded)
}

/// メタアドレス文字列を生成 (Wasm export)
#[wasm_bindgen]
pub fn format_meta_address_wasm(spend_pubkey: &[u8], view_pubkey: &[u8]) -> String {
    format_meta_address(spend_pubkey, view_pubkey)
}

/// メタアドレスをパースして公開鍵を抽出 (Wasm export)
#[wasm_bindgen]
pub fn parse_meta_address(meta_address: &str) -> Result<MetaAddressParts, JsError> {
    // プレフィックスを検証
    if !meta_address.starts_with(META_ADDRESS_PREFIX) {
        return Err(JsError::new("InvalidMetaAddress: must start with 'st:anarchy:'"));
    }

    // Base58デコード
    let encoded = &meta_address[META_ADDRESS_PREFIX.len()..];
    let decoded = bs58::decode(encoded)
        .into_vec()
        .map_err(|e| JsError::new(&format!("DecodingError: {}", e)))?;

    // 64バイトであることを確認 (spend_pubkey 32 + view_pubkey 32)
    if decoded.len() != 64 {
        return Err(JsError::new("InvalidMetaAddress: decoded length must be 64 bytes"));
    }

    let spend_pubkey = decoded[0..32].to_vec();
    let view_pubkey = decoded[32..64].to_vec();

    Ok(MetaAddressParts::new(spend_pubkey, view_pubkey))
}

/// メタアドレスからワンタイムステルスアドレスを導出 (Wasm export)
///
/// 送信者がこの関数を使用して、受信者のメタアドレスから
/// ワンタイムステルスアドレスとエフェメラル公開鍵を生成する。
#[wasm_bindgen]
pub fn derive_stealth_address(meta_address: &str) -> Result<StealthAddressResult, JsError> {
    // メタアドレスをパース
    let parts = parse_meta_address(meta_address)?;
    
    // 公開鍵を復元
    let spend_pubkey = PublicKey::from(
        <[u8; 32]>::try_from(parts.spend_pubkey().as_slice())
            .map_err(|_| JsError::new("InvalidSpendPubkey"))?
    );
    let view_pubkey = PublicKey::from(
        <[u8; 32]>::try_from(parts.view_pubkey().as_slice())
            .map_err(|_| JsError::new("InvalidViewPubkey"))?
    );

    // ランダムなエフェメラル秘密鍵を生成
    let ephemeral_secret = StaticSecret::random_from_rng(OsRng);
    let ephemeral_pubkey = PublicKey::from(&ephemeral_secret);

    // 共有シークレットを計算: s = r * K_view
    let shared_secret = ephemeral_secret.diffie_hellman(&view_pubkey);

    // ハッシュ化: h = H(s)
    let h = blake2b_256(shared_secret.as_bytes());

    // ステルス公開鍵を計算: P_stealth = K_spend + h * G
    // Note: X25519では点のスカラー乗算が直接できないため、
    // ここでは簡易的にハッシュとspend_pubkeyを組み合わせてアドレスを生成
    let stealth_pubkey = derive_stealth_pubkey(spend_pubkey.as_bytes(), &h);

    // SS58アドレスに変換
    let stealth_address = pubkey_to_ss58(&stealth_pubkey);

    Ok(StealthAddressResult::new(
        stealth_address,
        ephemeral_pubkey.as_bytes().to_vec(),
    ))
}

/// ステルス公開鍵を導出
/// P_stealth = K_spend XOR H(s) (簡易実装)
pub fn derive_stealth_pubkey(spend_pubkey: &[u8], hash: &[u8; 32]) -> [u8; 32] {
    let mut result = [0u8; 32];
    for i in 0..32 {
        result[i] = spend_pubkey[i] ^ hash[i];
    }
    result
}

/// 公開鍵をSS58アドレスに変換
fn pubkey_to_ss58(pubkey: &[u8; 32]) -> String {
    // SS58 prefix for Anarchy (42 = Substrate Generic)
    const SS58_PREFIX: u8 = 42;
    
    // SS58 checksum calculation
    let mut payload = Vec::with_capacity(35);
    payload.push(SS58_PREFIX);
    payload.extend_from_slice(pubkey);
    
    // Blake2b-512 hash for checksum
    use blake2::{Blake2b512, Digest};
    let mut hasher = Blake2b512::new();
    hasher.update(b"SS58PRE");
    hasher.update(&payload);
    let hash = hasher.finalize();
    
    // Append first 2 bytes of hash as checksum
    payload.extend_from_slice(&hash[0..2]);
    
    // Base58 encode
    bs58::encode(payload).into_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

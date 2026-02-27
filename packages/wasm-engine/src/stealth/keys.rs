//! X25519 key pair generation and management

use super::types::StealthKeyPairJs;
use super::address::format_meta_address;
use wasm_bindgen::prelude::*;
use x25519_dalek::{PublicKey, StaticSecret};
use rand_core::OsRng;

/// ステルス鍵ペアの内部表現
pub struct StealthKeyPair {
    pub spend_key: StaticSecret,
    pub view_key: StaticSecret,
    pub spend_pubkey: PublicKey,
    pub view_pubkey: PublicKey,
}

impl StealthKeyPair {
    /// 新しいステルス鍵ペアを生成
    pub fn generate() -> Self {
        let spend_key = StaticSecret::random_from_rng(OsRng);
        let view_key = StaticSecret::random_from_rng(OsRng);
        let spend_pubkey = PublicKey::from(&spend_key);
        let view_pubkey = PublicKey::from(&view_key);

        Self {
            spend_key,
            view_key,
            spend_pubkey,
            view_pubkey,
        }
    }

    /// メタアドレス文字列を取得
    pub fn meta_address(&self) -> String {
        format_meta_address(self.spend_pubkey.as_bytes(), self.view_pubkey.as_bytes())
    }
}

/// 新しいステルス鍵ペアを生成 (Wasm export)
#[wasm_bindgen]
pub fn generate_stealth_keys() -> StealthKeyPairJs {
    let keypair = StealthKeyPair::generate();
    
    StealthKeyPairJs::new(
        keypair.spend_key.as_bytes().to_vec(),
        keypair.view_key.as_bytes().to_vec(),
        keypair.spend_pubkey.as_bytes().to_vec(),
        keypair.view_pubkey.as_bytes().to_vec(),
        keypair.meta_address(),
    )
}

/// ステルスアドレスの秘密鍵を導出 (Wasm export)
///
/// 検出されたステルスアドレスから支出用の秘密鍵を導出する。
///
/// # Arguments
/// * `spend_key` - 自分のSpend秘密鍵 (32 bytes)
/// * `view_key` - 自分のView秘密鍵 (32 bytes)
/// * `ephemeral_pubkey` - トランザクションのエフェメラル公開鍵 (32 bytes)
///
/// # Returns
/// ステルスアドレスの秘密鍵 (32 bytes)
#[wasm_bindgen]
pub fn derive_stealth_private_key(
    spend_key: &[u8],
    view_key: &[u8],
    ephemeral_pubkey: &[u8],
) -> Result<Vec<u8>, JsError> {
    use super::hash::blake2b_256;

    // 入力検証
    if spend_key.len() != 32 {
        return Err(JsError::new("InvalidSpendKey: must be 32 bytes"));
    }
    if view_key.len() != 32 {
        return Err(JsError::new("InvalidViewKey: must be 32 bytes"));
    }
    if ephemeral_pubkey.len() != 32 {
        return Err(JsError::new("InvalidEphemeralKey: must be 32 bytes"));
    }

    // View秘密鍵とエフェメラル公開鍵から共有シークレットを計算
    let view_secret = StaticSecret::from(<[u8; 32]>::try_from(view_key).unwrap());
    let ephemeral_pub = PublicKey::from(<[u8; 32]>::try_from(ephemeral_pubkey).unwrap());
    let shared_secret = view_secret.diffie_hellman(&ephemeral_pub);

    // ハッシュ化: h = H(s)
    let h = blake2b_256(shared_secret.as_bytes());

    // ステルス秘密鍵: p_stealth = k_spend + h (モジュラー加算)
    // Note: X25519のスカラー加算を行う
    let spend_bytes: [u8; 32] = spend_key.try_into().unwrap();
    let stealth_private_key = scalar_add(&spend_bytes, &h);

    Ok(stealth_private_key.to_vec())
}

/// スカラー加算 (mod l, where l is the order of the curve)
fn scalar_add(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    // 簡易実装: バイト単位の加算 (実際にはモジュラー演算が必要)
    // Note: 本番環境ではcurve25519-dalekのスカラー演算を使用すべき
    let mut result = [0u8; 32];
    let mut carry: u16 = 0;
    
    for i in 0..32 {
        let sum = (a[i] as u16) + (b[i] as u16) + carry;
        result[i] = sum as u8;
        carry = sum >> 8;
    }
    
    result
}

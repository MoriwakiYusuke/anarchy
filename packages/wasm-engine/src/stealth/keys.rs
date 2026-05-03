//! Ed25519/X25519 key pair generation and management
//! 
//! spend鍵: Ed25519（署名と正しい公開鍵導出用）
//! view鍵: X25519（ECDHによる共有シークレット計算用）

use super::types::StealthKeyPairJs;
use super::address::format_meta_address;
use wasm_bindgen::prelude::*;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use ed25519_dalek::SigningKey;
use rand_core::OsRng;

/// ステルス鍵ペアの内部表現
pub struct StealthKeyPair {
    /// Ed25519 spend秘密鍵（32バイトシード）
    pub spend_key: [u8; 32],
    /// X25519 view秘密鍵
    pub view_key: StaticSecret,
    /// Ed25519 spend公開鍵
    pub spend_pubkey: [u8; 32],
    /// X25519 view公開鍵
    pub view_pubkey: X25519PublicKey,
}

impl StealthKeyPair {
    /// 新しいステルス鍵ペアを生成
    pub fn generate() -> Self {
        // Ed25519 spend鍵を生成
        let spend_signing_key = SigningKey::generate(&mut OsRng);
        let spend_key = spend_signing_key.to_bytes();
        let spend_pubkey = spend_signing_key.verifying_key().to_bytes();
        
        // X25519 view鍵を生成（ECDH用）
        let view_key = StaticSecret::random_from_rng(OsRng);
        let view_pubkey = X25519PublicKey::from(&view_key);

        Self {
            spend_key,
            view_key,
            spend_pubkey,
            view_pubkey,
        }
    }

    /// メタアドレス文字列を取得
    pub fn meta_address(&self) -> String {
        format_meta_address(&self.spend_pubkey, self.view_pubkey.as_bytes())
    }
}

/// 新しいステルス鍵ペアを生成 (Wasm export)
#[wasm_bindgen]
pub fn generate_stealth_keys() -> StealthKeyPairJs {
    let keypair = StealthKeyPair::generate();

    StealthKeyPairJs::new(
        keypair.spend_key.to_vec(),
        keypair.view_key.as_bytes().to_vec(),
        keypair.spend_pubkey.to_vec(),
        keypair.view_pubkey.as_bytes().to_vec(),
        keypair.meta_address(),
    )
}

/// 既存の (spend_seed, view_seed) からステルス鍵ペア + メタアドレスを再構築 (Wasm export)。
///
/// バックアップ復元時 (DM keyManager の loadFromBackup 経由) に使う。
/// generate_stealth_keys と同じ JS 形を返すので、呼出側のロジックを共通化できる。
///
/// # Arguments
/// * `spend_seed` - Ed25519 spend 秘密鍵 (32 bytes)
/// * `view_seed`  - X25519 view 秘密鍵 (32 bytes)
#[wasm_bindgen]
pub fn restore_stealth_keys(
    spend_seed: &[u8],
    view_seed: &[u8],
) -> Result<StealthKeyPairJs, JsError> {
    if spend_seed.len() != 32 {
        return Err(JsError::new("spend_seed must be 32 bytes"));
    }
    if view_seed.len() != 32 {
        return Err(JsError::new("view_seed must be 32 bytes"));
    }

    let mut spend_arr = [0u8; 32];
    spend_arr.copy_from_slice(spend_seed);
    let signing = SigningKey::from_bytes(&spend_arr);
    let spend_pubkey: [u8; 32] = signing.verifying_key().to_bytes();

    let mut view_arr = [0u8; 32];
    view_arr.copy_from_slice(view_seed);
    let view_secret = StaticSecret::from(view_arr);
    let view_pubkey = X25519PublicKey::from(&view_secret);

    let meta = format_meta_address(&spend_pubkey, view_pubkey.as_bytes());

    Ok(StealthKeyPairJs::new(
        spend_arr.to_vec(),
        view_seed.to_vec(),
        spend_pubkey.to_vec(),
        view_pubkey.as_bytes().to_vec(),
        meta,
    ))
}

/// ステルスアドレスの秘密鍵を導出 (Wasm export)
///
/// 検出されたステルスアドレスから支出用の秘密鍵を導出する。
///
/// # Arguments
/// * `spend_key` - 自分のSpend秘密鍵 (32 bytes, Ed25519 seed)
/// * `view_key` - 自分のView秘密鍵 (32 bytes, X25519 secret)
/// * `ephemeral_pubkey` - トランザクションのエフェメラル公開鍵 (32 bytes)
///
/// # Returns
/// ステルスアドレスの秘密鍵 (64 bytes: scalar || nonce_prefix for direct signing)
#[wasm_bindgen]
pub fn derive_stealth_private_key(
    spend_key: &[u8],
    view_key: &[u8],
    ephemeral_pubkey: &[u8],
) -> Result<Vec<u8>, JsError> {
    use super::hash::blake2b_256;
    use x25519_dalek::PublicKey as X25519PublicKey;

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
    let ephemeral_pub = X25519PublicKey::from(<[u8; 32]>::try_from(ephemeral_pubkey).unwrap());
    let shared_secret = view_secret.diffie_hellman(&ephemeral_pub);

    // ハッシュ化: h = H(s)
    let h = blake2b_256(shared_secret.as_bytes());

    // Ed25519のスカラー演算でステルス秘密鍵を導出
    use curve25519_dalek::Scalar;
    
    // Ed25519シードからスカラーとノンスプレフィックスを導出
    use sha2::{Sha512, Digest};
    let mut hasher = Sha512::new();
    hasher.update(spend_key);
    let expanded = hasher.finalize();
    
    // 前半32バイト: スカラー (clamped)
    let mut spend_scalar_bytes = [0u8; 32];
    spend_scalar_bytes.copy_from_slice(&expanded[..32]);
    spend_scalar_bytes[0] &= 248;
    spend_scalar_bytes[31] &= 63;
    spend_scalar_bytes[31] |= 64;
    
    // 後半32バイト: ノンスプレフィックス (署名の決定論的ノンス生成用)
    let mut nonce_prefix = [0u8; 32];
    nonce_prefix.copy_from_slice(&expanded[32..64]);
    
    // hをスカラーに変換（mod l）
    let h_scalar = Scalar::from_bytes_mod_order(h);
    let spend_scalar = Scalar::from_bytes_mod_order(spend_scalar_bytes);
    
    // スカラー加算: p_stealth = k_spend + h
    let stealth_scalar = spend_scalar + h_scalar;
    
    // ノンスプレフィックスもhで更新（決定論的だが異なるノンスを生成）
    let mut stealth_nonce_hasher = Sha512::new();
    stealth_nonce_hasher.update(&nonce_prefix);
    stealth_nonce_hasher.update(&h);
    let stealth_nonce_expanded = stealth_nonce_hasher.finalize();
    let mut stealth_nonce_prefix = [0u8; 32];
    stealth_nonce_prefix.copy_from_slice(&stealth_nonce_expanded[..32]);
    
    // 64バイト返却: scalar (32) || nonce_prefix (32)
    let mut result = Vec::with_capacity(64);
    result.extend_from_slice(&stealth_scalar.to_bytes());
    result.extend_from_slice(&stealth_nonce_prefix);
    
    Ok(result)
}

/// ステルス秘密鍵から公開鍵を導出 (Wasm export)
///
/// # Arguments
/// * `expanded_key` - 64バイトの拡張秘密鍵 (scalar || nonce_prefix)
///
/// # Returns
/// 32バイトの公開鍵
#[wasm_bindgen]
pub fn stealth_get_public_key(expanded_key: &[u8]) -> Result<Vec<u8>, JsError> {
    use curve25519_dalek::Scalar;
    use curve25519_dalek::constants::ED25519_BASEPOINT_TABLE;
    
    if expanded_key.len() != 64 {
        return Err(JsError::new("InvalidExpandedKey: must be 64 bytes"));
    }
    
    let mut scalar_bytes = [0u8; 32];
    scalar_bytes.copy_from_slice(&expanded_key[..32]);
    let scalar = Scalar::from_bytes_mod_order(scalar_bytes);
    
    // P = s * G
    let public_point = ED25519_BASEPOINT_TABLE * &scalar;
    let public_key = public_point.compress().to_bytes();
    
    Ok(public_key.to_vec())
}

/// ステルス秘密鍵でEd25519署名 (Wasm export)
///
/// # Arguments
/// * `expanded_key` - 64バイトの拡張秘密鍵 (scalar || nonce_prefix)
/// * `message` - 署名対象のメッセージ
///
/// # Returns
/// 64バイトのEd25519署名
#[wasm_bindgen]
pub fn stealth_sign(expanded_key: &[u8], message: &[u8]) -> Result<Vec<u8>, JsError> {
    use curve25519_dalek::Scalar;
    use curve25519_dalek::constants::ED25519_BASEPOINT_TABLE;
    use sha2::{Sha512, Digest};
    
    if expanded_key.len() != 64 {
        return Err(JsError::new("InvalidExpandedKey: must be 64 bytes"));
    }
    
    // 拡張鍵をパース
    let mut scalar_bytes = [0u8; 32];
    scalar_bytes.copy_from_slice(&expanded_key[..32]);
    let scalar = Scalar::from_bytes_mod_order(scalar_bytes);
    
    let mut nonce_prefix = [0u8; 32];
    nonce_prefix.copy_from_slice(&expanded_key[32..64]);
    
    // 公開鍵を計算
    let public_point = ED25519_BASEPOINT_TABLE * &scalar;
    let public_key = public_point.compress().to_bytes();
    
    // Ed25519署名アルゴリズム (RFC 8032)
    // 1. r = H(nonce_prefix || message) mod l
    let mut r_hasher = Sha512::new();
    r_hasher.update(&nonce_prefix);
    r_hasher.update(message);
    let r_hash = r_hasher.finalize();
    let r = Scalar::from_bytes_mod_order_wide(&r_hash.into());
    
    // 2. R = r * G
    let r_point = ED25519_BASEPOINT_TABLE * &r;
    let r_bytes = r_point.compress().to_bytes();
    
    // 3. k = H(R || A || message) mod l
    let mut k_hasher = Sha512::new();
    k_hasher.update(&r_bytes);
    k_hasher.update(&public_key);
    k_hasher.update(message);
    let k_hash = k_hasher.finalize();
    let k = Scalar::from_bytes_mod_order_wide(&k_hash.into());
    
    // 4. s = r + k * scalar
    let s = r + k * scalar;
    
    // 署名 = R || s
    let mut signature = Vec::with_capacity(64);
    signature.extend_from_slice(&r_bytes);
    signature.extend_from_slice(&s.to_bytes());
    
    Ok(signature)
}

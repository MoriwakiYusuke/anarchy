//! Type definitions for stealth address module

use wasm_bindgen::prelude::*;
use zeroize::Zeroizing;

/// ステルス鍵ペア (Wasm export用)
///
/// **Zeroization** (Constitution II / FR-021):
/// `spend_key` / `view_key` は `Zeroizing<Vec<u8>>` で保持し、Drop 時に
/// memset(0) する。wasm-bindgen の `free()` が Drop を呼ぶので、JS 側で
/// `wasmKeys.free()` を呼べば Rust 線形メモリの秘密は確実に消える。
/// pubkeys と meta_address は公開情報なので `Vec<u8>` のまま保持する。
#[wasm_bindgen]
pub struct StealthKeyPairJs {
    /// Spend秘密鍵 (32 bytes)
    spend_key: Zeroizing<Vec<u8>>,
    /// View秘密鍵 (32 bytes)
    view_key: Zeroizing<Vec<u8>>,
    /// Spend公開鍵 (32 bytes)
    spend_pubkey: Vec<u8>,
    /// View公開鍵 (32 bytes)
    view_pubkey: Vec<u8>,
    /// メタアドレス文字列
    meta_address: String,
}

#[wasm_bindgen]
impl StealthKeyPairJs {
    /// Spend秘密鍵を取得 (JS 側にコピー — JS 側で fill(0) + free() の責務)
    #[wasm_bindgen(getter)]
    pub fn spend_key(&self) -> Vec<u8> {
        (*self.spend_key).clone()
    }

    /// View秘密鍵を取得 (JS 側にコピー — JS 側で fill(0) + free() の責務)
    #[wasm_bindgen(getter)]
    pub fn view_key(&self) -> Vec<u8> {
        (*self.view_key).clone()
    }

    /// Spend公開鍵を取得
    #[wasm_bindgen(getter)]
    pub fn spend_pubkey(&self) -> Vec<u8> {
        self.spend_pubkey.clone()
    }

    /// View公開鍵を取得
    #[wasm_bindgen(getter)]
    pub fn view_pubkey(&self) -> Vec<u8> {
        self.view_pubkey.clone()
    }

    /// メタアドレスを取得
    #[wasm_bindgen(getter)]
    pub fn meta_address(&self) -> String {
        self.meta_address.clone()
    }
}

impl StealthKeyPairJs {
    /// 新しいStealthKeyPairJsを作成。秘密鍵は Zeroizing でラップする。
    pub fn new(
        spend_key: Vec<u8>,
        view_key: Vec<u8>,
        spend_pubkey: Vec<u8>,
        view_pubkey: Vec<u8>,
        meta_address: String,
    ) -> Self {
        Self {
            spend_key: Zeroizing::new(spend_key),
            view_key: Zeroizing::new(view_key),
            spend_pubkey,
            view_pubkey,
            meta_address,
        }
    }
}

/// ステルスアドレス導出結果 (Wasm export用)
#[wasm_bindgen]
#[derive(Clone)]
pub struct StealthAddressResult {
    /// ステルスアドレス (SS58形式)
    stealth_address: String,
    /// エフェメラル公開鍵 (32 bytes)
    ephemeral_pubkey: Vec<u8>,
    /// ステルス公開鍵 (32 bytes) - フロントエンドでSS58エンコードに使用
    stealth_pubkey: Vec<u8>,
}

#[wasm_bindgen]
impl StealthAddressResult {
    /// ステルスアドレスを取得
    #[wasm_bindgen(getter)]
    pub fn stealth_address(&self) -> String {
        self.stealth_address.clone()
    }

    /// エフェメラル公開鍵を取得
    #[wasm_bindgen(getter)]
    pub fn ephemeral_pubkey(&self) -> Vec<u8> {
        self.ephemeral_pubkey.clone()
    }

    /// ステルス公開鍵を取得 (32 bytes)
    #[wasm_bindgen(getter)]
    pub fn stealth_pubkey(&self) -> Vec<u8> {
        self.stealth_pubkey.clone()
    }
}

impl StealthAddressResult {
    /// 新しいStealthAddressResultを作成
    pub fn new(stealth_address: String, ephemeral_pubkey: Vec<u8>, stealth_pubkey: Vec<u8>) -> Self {
        Self {
            stealth_address,
            ephemeral_pubkey,
            stealth_pubkey,
        }
    }
}

/// メタアドレスパーツ (Wasm export用)
#[wasm_bindgen]
#[derive(Clone)]
pub struct MetaAddressParts {
    /// Spend公開鍵 (32 bytes)
    spend_pubkey: Vec<u8>,
    /// View公開鍵 (32 bytes)
    view_pubkey: Vec<u8>,
}

#[wasm_bindgen]
impl MetaAddressParts {
    /// Spend公開鍵を取得
    #[wasm_bindgen(getter)]
    pub fn spend_pubkey(&self) -> Vec<u8> {
        self.spend_pubkey.clone()
    }

    /// View公開鍵を取得
    #[wasm_bindgen(getter)]
    pub fn view_pubkey(&self) -> Vec<u8> {
        self.view_pubkey.clone()
    }
}

impl MetaAddressParts {
    /// 新しいMetaAddressPartsを作成
    pub fn new(spend_pubkey: Vec<u8>, view_pubkey: Vec<u8>) -> Self {
        Self {
            spend_pubkey,
            view_pubkey,
        }
    }
}

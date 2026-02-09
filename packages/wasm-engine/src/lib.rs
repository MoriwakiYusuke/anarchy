//! Anarchy Wasm Engine
//!
//! ブラウザで実行可能なWasm暗号エンジン。
//! - SSS (Shamir's Secret Sharing): k-of-n 閾値分割・復元
//! - MerkleTree: Blake2b ベースのマークルツリー構築・検証

mod merkle;
mod sss;

use wasm_bindgen::prelude::*;

// パニック時のコンソール出力を有効化
#[cfg(feature = "console_error_panic_hook")]
pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Wasm初期化
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    set_panic_hook();
}

// Re-export public APIs
pub use merkle::*;
pub use sss::*;

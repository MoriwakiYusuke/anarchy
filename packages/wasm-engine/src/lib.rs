//! Anarchy Wasm Engine
//!
//! ブラウザで実行可能なWasm暗号エンジン。
//! - KZG-VSS: BLS12-381曲線上の検証可能秘密分散
//! - Hybrid: AES-256-GCM暗号 + KZG-VSS鍵分散
//! - Stealth: EIP-5564互換ステルスアドレス

pub mod dm;
pub mod kzg;
mod merkle;
pub mod stealth;

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

// Re-export Merkle tree APIs (legacy)
pub use merkle::*;

// Re-export KZG-VSS APIs
pub use kzg::{
    compress, decompress, init_srs, is_srs_initialized, regenerate_share, verify_kzg_proof,
    vss_prove, vss_recover, vss_split, KzgCommitment, KzgError, KzgProof, VssShare,
    VssSplitResult, BYTES_PER_SCALAR,
};

// Re-export Wasm bindings
pub use kzg::wasm::{
    hybrid_recover, hybrid_split, kzg_compress, kzg_decompress, kzg_generate_proof,
    kzg_init_srs, kzg_is_srs_initialized, kzg_verify_proof, kzg_vss_recover, kzg_vss_split,
    WasmHybridShard, WasmHybridSplitResult, WasmVssShare, WasmVssSplitResult,
};

// Re-export Stealth APIs
pub use stealth::{
    derive_stealth_address, derive_stealth_private_key, encrypt_backup, decrypt_backup,
    format_meta_address_wasm, generate_stealth_keys, parse_meta_address, scan_transaction,
    MetaAddressParts, StealthAddressResult, StealthKeyPairJs,
};

// Re-export DM APIs (feature 019)
pub use dm::decrypt::{dm_decrypt_scan, DmDecryptedEnvelope};
pub use dm::encrypt::{
    dm_derive_recipient_stealth, dm_encrypt_and_pad, dm_fragment_ciphertext,
    dm_generate_sender_stealth, DmEncryptedOutput, DmFragmentedOutput, DmSenderStealth,
    DmStealthDerivation,
};
pub use dm::envelope::dm_compute_inner_signed_hash;
pub use dm::media::{dm_media_decrypt, dm_media_encrypt};

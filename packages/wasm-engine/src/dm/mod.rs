//! Direct Message (DM) Module
//!
//! DM 専用の暗号処理を提供するモジュール。feature 019-direct-messages の
//! wasm-engine-dm-api.md を実装する。
//!
//! - X25519 ECDH + HKDF-SHA256: エフェメラル鍵交換からの共通秘密導出
//! - AES-256-GCM: 本文暗号化
//! - ISO 7816-4 padding: バケット化 (1K/4K/16K/64K/256K)
//! - Sr25519 inner signature: 送信者ステルスからの署名
//!
//! フェーズ 1 ではモジュールスケルトンのみ。暗号処理と wasm binding は
//! フェーズ 2 以降 (T014-T017, T032-T037) で追加する。

pub mod decrypt;
pub mod encrypt;
pub mod envelope;
pub mod media;
pub mod padding;
pub mod types;

#[cfg(test)]
mod tests;

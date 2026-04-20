//! Pallet-messaging unit tests.
//!
//! Phase 3 で `publish` / `revoke` / `send` / `runtime_api` を追加。
//! `tx_failure` (T090) と `sender_stealth_zeroize` は別タスクで追加予定。

pub mod publish;
pub mod revoke;
pub mod runtime_api;
pub mod send;
pub mod stealth_integration;

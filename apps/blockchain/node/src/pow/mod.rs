//! Node-side PoW glue.
//!
//! Phase A では service.rs に配線せず crate 内モジュールとして公開のみ。
//! Phase B で service.rs から `RandomXAlgorithm` / `PowAuthor` を消費する。

pub mod author;
pub mod difficulty;
pub mod randomx_algo;

pub use author::{PowAuthor, POW_ENGINE_ID};
pub use difficulty::DifficultyClient;
pub use randomx_algo::RandomXAlgorithm;

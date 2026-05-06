//! Node-side PoW glue.
//!
//! - `randomx_algo`: `sc_consensus_pow::PowAlgorithm` の RandomX 実装と PoW seal 型
//! - `author`: PreRuntime digest decoder (FindAuthor for runtime-side block reward)
//! - `difficulty`: runtime API `DifficultyApi` への client 経由アクセスラッパ
//!
//! `service.rs` は `crate::pow::RandomXAlgorithm` 形式で短く参照したいので主要型は
//! ここで re-export する。`PowAuthor` / `DifficultyClient` は storage_node や CLI tool
//! から参照する想定の API 表層。

pub mod author;
pub mod difficulty;
pub mod randomx_algo;

pub use author::{PowAuthor, POW_ENGINE_ID};
pub use difficulty::DifficultyClient;
pub use randomx_algo::RandomXAlgorithm;

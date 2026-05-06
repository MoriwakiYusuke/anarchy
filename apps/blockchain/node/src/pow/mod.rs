//! Node-side PoW glue.
//!
//! - `randomx_algo`: `sc_consensus_pow::PowAlgorithm` の RandomX 実装と PoW seal 型
//! - `author`: PreRuntime digest decoder (FindAuthor for runtime-side block reward)
//! - `difficulty`: runtime API `DifficultyApi` への client 経由アクセスラッパ
//!
//! `service.rs` は `randomx_algo::RandomXAlgorithm` を直接 import して使う。
//! `PowAuthor` / `DifficultyClient` は将来 storage_node や CLI tool から参照する
//! 想定の public re-export — 現状未配線。

pub mod author;
pub mod difficulty;
pub mod randomx_algo;

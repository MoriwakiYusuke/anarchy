//! Node-side PoW glue.
//!
//! - `randomx_algo`: `sc_consensus_pow::PowAlgorithm` の RandomX 実装と PoW seal 型
//! - `difficulty`: runtime API `DifficultyApi` への client 経由アクセスラッパ
//!   (`randomx_algo` 内部からのみ参照される)
//!
//! `service.rs` は `crate::pow::RandomXAlgorithm` 形式で短く参照したいので主要型は
//! ここで re-export する。

pub mod difficulty;
pub mod randomx_algo;

pub use randomx_algo::RandomXAlgorithm;

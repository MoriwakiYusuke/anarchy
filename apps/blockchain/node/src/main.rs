//! Anarchy Node
//!
//! メインエントリーポイント

mod chain_spec;
mod cli;
mod command;
mod gossip;
mod rpc;
mod service;
mod storage;

fn main() -> sc_cli::Result<()> {
    command::run()
}

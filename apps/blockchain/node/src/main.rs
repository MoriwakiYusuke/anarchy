//! Anarchy Node
//!
//! メインエントリーポイント

mod chain_spec;
mod cli;
mod command;
mod gossip;
mod pow;
mod rpc;
mod service;

fn main() -> sc_cli::Result<()> {
    command::run()
}

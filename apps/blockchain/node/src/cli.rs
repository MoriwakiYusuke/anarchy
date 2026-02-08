//! CLI定義

use clap::{Parser, ValueEnum};
use sc_cli::RunCmd;

/// Tor usage mode for anonymous network participation
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum TorMode {
    /// No Tor - direct TCP connections (development only)
    #[default]
    Off,
    /// Outbound through Tor, inbound direct (WARNING: inbound IP exposed)
    OutboundOnly,
    /// Full anonymity - Tor required for both directions
    Forced,
}

impl std::fmt::Display for TorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TorMode::Off => write!(f, "off"),
            TorMode::OutboundOnly => write!(f, "outbound-only"),
            TorMode::Forced => write!(f, "forced"),
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "anarchy-node")]
#[command(about = "Anarchy: 匿名分散型SNSノード")]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[command(flatten)]
    pub run: RunCmd,

    /// Tor mode for network anonymity
    /// 
    /// - off: No Tor (development only)
    /// - outbound-only: Outbound via Tor, inbound exposed (WARNING)
    /// - forced: Full anonymity via Tor
    #[arg(long, value_enum, default_value_t = TorMode::Off)]
    pub tor_mode: TorMode,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// キーを管理
    #[command(subcommand)]
    Key(sc_cli::KeySubcommand),

    /// チェーンスペックを出力
    BuildSpec(sc_cli::BuildSpecCmd),

    /// ノードのランタイム情報を検証
    CheckBlock(sc_cli::CheckBlockCmd),

    /// データベースをエクスポート
    ExportBlocks(sc_cli::ExportBlocksCmd),

    /// ブロックステートをエクスポート
    ExportState(sc_cli::ExportStateCmd),

    /// ブロックをインポート
    ImportBlocks(sc_cli::ImportBlocksCmd),

    /// データベースを削除
    PurgeChain(sc_cli::PurgeChainCmd),

    /// ブロックをリバート
    Revert(sc_cli::RevertCmd),
}

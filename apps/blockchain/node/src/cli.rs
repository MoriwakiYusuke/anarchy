//! CLI定義

use clap::Parser;
use sc_cli::RunCmd;

#[derive(Debug, Parser)]
#[command(name = "anarchy-node")]
#[command(about = "Anarchy: 匿名分散型SNSノード")]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[command(flatten)]
    pub run: RunCmd,
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

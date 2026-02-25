#!/bin/bash
# フロントエンド起動前にchainspec.jsonを自動更新するスクリプト
# 1. ノードから最新のchainspecを生成
# 2. Peer IDを取得してbootNodesを設定

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$(dirname "$SCRIPT_DIR")"
BLOCKCHAIN_DIR="$FRONTEND_DIR/../blockchain"
CHAINSPEC_FILE="$FRONTEND_DIR/src/lib/chainspec.json"
NODE_BINARY="$BLOCKCHAIN_DIR/target/release/anarchy-node"

# カラー出力
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[chainspec]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[chainspec]${NC} $1"; }
log_error() { echo -e "${RED}[chainspec]${NC} $1"; }

# anarchy-nodeバイナリが存在するか確認
if [ ! -f "$NODE_BINARY" ]; then
    log_warn "anarchy-nodeバイナリが見つかりません: $NODE_BINARY"
    log_warn "ビルドするには: pnpm build:blockchain"
    exit 0
fi

# anarchy-nodeが起動しているか確認
if ! pgrep -f "anarchy-node" > /dev/null 2>&1; then
    log_warn "anarchy-nodeが起動していません。bootNodesの更新をスキップします。"
    log_warn "起動するには: pnpm testnet:start"
    exit 0
fi

# Peer IDを取得（優先順位: ログファイル → psコマンドのbootnodes）
PEER_ID=""
ALICE_LOG="$BLOCKCHAIN_DIR/logs/alice.log"

# 1. ログファイルから取得を試みる
if [ -f "$ALICE_LOG" ]; then
    PEER_ID=$(grep -m1 "Local node identity" "$ALICE_LOG" | grep -oP '12D3KooW[A-Za-z0-9]+' || true)
fi

# 2. ログから取得できない場合、psコマンドのbootnodesアーグメントから取得
if [ -z "$PEER_ID" ]; then
    PEER_ID=$(ps aux | grep "anarchy-node" | grep -oP '12D3KooW[A-Za-z0-9]+' | head -1 || true)
fi

if [ -z "$PEER_ID" ]; then
    log_warn "Peer IDを取得できませんでした。bootNodesの更新をスキップします。"
    exit 0
fi

log_info "検出されたPeer ID: $PEER_ID"

# 新しいchainspecを生成（最新のgenesisを含む）
log_info "最新のchainspecを生成中..."
TMP_CHAINSPEC="/tmp/anarchy_chainspec_$$.json"
"$NODE_BINARY" build-spec --chain local --raw 2>/dev/null > "$TMP_CHAINSPEC"

if [ ! -s "$TMP_CHAINSPEC" ]; then
    log_error "chainspec生成に失敗しました"
    rm -f "$TMP_CHAINSPEC"
    exit 1
fi

# bootNodesを設定
WS_PORT=30833
NEW_BOOTNODE="/ip4/127.0.0.1/tcp/${WS_PORT}/ws/p2p/${PEER_ID}"

if command -v jq &> /dev/null; then
    jq --arg bootnode "$NEW_BOOTNODE" '.bootNodes = [$bootnode]' "$TMP_CHAINSPEC" > "$CHAINSPEC_FILE"
else
    # jqがない場合はsedで置換
    sed "s|\"bootNodes\": \[[^]]*\]|\"bootNodes\": [\"${NEW_BOOTNODE}\"]|" "$TMP_CHAINSPEC" > "$CHAINSPEC_FILE"
fi

rm -f "$TMP_CHAINSPEC"

log_info "chainspec.jsonを更新しました（genesis + bootNodes）"
log_info "bootNodes: $NEW_BOOTNODE"

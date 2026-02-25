#!/bin/bash
# フロントエンド起動前にchainspec.jsonのbootNodesを自動更新するスクリプト
# aliceノードのログからPeer IDを取得し、WebSocketエンドポイントに更新

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$(dirname "$SCRIPT_DIR")"
BLOCKCHAIN_DIR="$FRONTEND_DIR/../blockchain"
CHAINSPEC_FILE="$FRONTEND_DIR/src/lib/chainspec.json"
ALICE_LOG="$BLOCKCHAIN_DIR/logs/alice.log"

# カラー出力
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[chainspec]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[chainspec]${NC} $1"; }
log_error() { echo -e "${RED}[chainspec]${NC} $1"; }

# aliceノードが起動しているか確認
if ! pgrep -f "anarchy-node.*--alice" > /dev/null 2>&1; then
    log_warn "aliceノードが起動していません。bootNodesの更新をスキップします。"
    log_warn "起動するには: cd apps/blockchain && ./scripts/run-multi-node.sh start"
    exit 0
fi

# ログファイルが存在するか確認
if [ ! -f "$ALICE_LOG" ]; then
    log_warn "aliceのログファイルが見つかりません: $ALICE_LOG"
    exit 0
fi

# Peer IDを取得
PEER_ID=$(grep -m1 "Local node identity" "$ALICE_LOG" | grep -oP '12D3KooW[A-Za-z0-9]+' || true)

if [ -z "$PEER_ID" ]; then
    log_warn "Peer IDを取得できませんでした。bootNodesの更新をスキップします。"
    exit 0
fi

log_info "検出されたPeer ID: $PEER_ID"

# WebSocketポート（aliceのWSポート: 30833）
WS_PORT=30833

# 新しいbootNodesエントリ
NEW_BOOTNODE="/ip4/127.0.0.1/tcp/${WS_PORT}/ws/p2p/${PEER_ID}"

# 現在のbootNodesを確認（jqで正確に取得）
if command -v jq &> /dev/null; then
    CURRENT_BOOTNODE=$(jq -r '.bootNodes[0] // ""' "$CHAINSPEC_FILE")
else
    CURRENT_BOOTNODE=$(grep -oP '"bootNodes":\s*\[\s*"\K[^"]+' "$CHAINSPEC_FILE" || true)
fi

if [ "$CURRENT_BOOTNODE" = "$NEW_BOOTNODE" ]; then
    log_info "bootNodesは最新です。更新の必要はありません。"
    exit 0
fi

# chainspec.jsonを更新（jqを使用してJSON形式を維持）
if command -v jq &> /dev/null; then
    jq --arg bootnode "$NEW_BOOTNODE" '.bootNodes = [$bootnode]' "$CHAINSPEC_FILE" > "$CHAINSPEC_FILE.tmp" && mv "$CHAINSPEC_FILE.tmp" "$CHAINSPEC_FILE"
else
    # jqがない場合はsedで置換（一行で）
    sed -i "s|\"bootNodes\": \[[^]]*\]|\"bootNodes\": [\"${NEW_BOOTNODE}\"]|" "$CHAINSPEC_FILE"
fi

log_info "bootNodesを更新しました: $NEW_BOOTNODE"

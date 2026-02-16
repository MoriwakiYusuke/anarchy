#!/bin/bash
# Storage Node管理スクリプト
# 使用方法: ./run-storage-nodes.sh [start|stop|status|purge] [ノード数]
#   start [N]  - N個のStorage Nodeを起動（デフォルト: 5）
#   stop       - 全Storage Nodeを停止
#   status     - 稼働状況を表示
#   purge      - 全データを削除

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/target/release/anarchy-storage-node"
DATA_DIR="$PROJECT_DIR/data"
LOGS_DIR="$PROJECT_DIR/logs"
CONFIG_TEMPLATE="$PROJECT_DIR/config.example.toml"

# カラー出力
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# ベースポート
BASE_RPC_PORT=3030
BASE_LIBP2P_PORT=4001

# Dev用署名シード (WARNING: 本番環境では使用しないこと!)
# https://github.com/polkadot-js/common/blob/master/packages/keyring/src/testing.ts
DEV_SIGNER_SEEDS=(
    "e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a"  # Alice
    "398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89"  # Bob
    "bc1ede780f784bb6991a585e4f6e61522c14e1cae6324f92e34dd3db81a39a12"  # Charlie
    "868020ae0687dda7d57565093a69090211449845a7e11453612800b663307246"  # Dave
    "786ad0e2df456fe43dd1f91ebca22e235bc162e0bb8d53c633e8c85b2af68b7a"  # Eve
    # 5ノード以上の場合はAliceから再利用
)

# ノード番号に対応するseedを取得
get_signer_seed() {
    local node_num=$1
    local seed_index=$(( (node_num - 1) % ${#DEV_SIGNER_SEEDS[@]} ))
    echo "${DEV_SIGNER_SEEDS[$seed_index]}"
}

start_nodes() {
    local num_nodes=${1:-5}
    
    if [[ ! -f "$BINARY" ]]; then
        echo -e "${RED}Error: バイナリが見つかりません。先にビルドしてください:${NC}"
        echo "  cd $PROJECT_DIR && cargo build --release"
        exit 1
    fi
    
    mkdir -p "$LOGS_DIR"
    
    echo -e "${GREEN}=== Storage Node起動 (${num_nodes}ノード) ===${NC}"
    
    for i in $(seq 1 "$num_nodes"); do
        local rpc_port=$((BASE_RPC_PORT + i - 1))
        local libp2p_port=$((BASE_LIBP2P_PORT + i - 1))
        local node_data="$DATA_DIR/node$i"
        local config_file="$PROJECT_DIR/node$i.toml"
        local log_file="$LOGS_DIR/node$i.log"
        local pid_file="$DATA_DIR/node$i.pid"
        
        # 既に起動中ならスキップ
        if [[ -f "$pid_file" ]] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
            echo -e "${YELLOW}Node $i: 既に起動中 (PID: $(cat "$pid_file"))${NC}"
            continue
        fi
        
        # 設定ファイル生成
        local signer_seed=$(get_signer_seed $i)
        mkdir -p "$node_data"
        cat > "$config_file" << EOF
# Auto-generated config for Storage Node $i
data_dir = "$node_data"
capacity = 10737418240
chain_url = "ws://127.0.0.1:9944"
listen_addr = "/ip4/0.0.0.0/tcp/$libp2p_port"
declare_rate_limit = 10
rpc_port = $rpc_port

# Dev signer seed (Node $i)
# WARNING: For development only! Generate unique seed for production.
signer_seed = "$signer_seed"
EOF
        
        # ノード起動
        "$BINARY" --config "$config_file" > "$log_file" 2>&1 &
        local pid=$!
        echo "$pid" > "$pid_file"
        
        echo -e "${GREEN}Node $i: 起動 (PID: $pid, RPC: $rpc_port, P2P: $libp2p_port)${NC}"
    done
    
    echo ""
    echo "ログ: $LOGS_DIR/nodeN.log"
    echo "確認: ./scripts/run-storage-nodes.sh status"
}

stop_nodes() {
    echo -e "${YELLOW}=== Storage Node停止 ===${NC}"
    
    local stopped=0
    for pid_file in "$DATA_DIR"/node*.pid; do
        [[ -f "$pid_file" ]] || continue
        local pid=$(cat "$pid_file")
        local node_name=$(basename "$pid_file" .pid)
        
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
            echo -e "${GREEN}${node_name}: 停止 (PID: $pid)${NC}"
            stopped=$((stopped + 1))
        fi
        rm -f "$pid_file"
    done
    
    if [[ $stopped -eq 0 ]]; then
        echo "稼働中のノードはありません"
    else
        echo -e "${GREEN}${stopped}ノードを停止しました${NC}"
    fi
}

show_status() {
    echo -e "${GREEN}=== Storage Node稼働状況 ===${NC}"
    
    local running=0
    local total=0
    
    for pid_file in "$DATA_DIR"/node*.pid; do
        [[ -f "$pid_file" ]] || continue
        total=$((total + 1))
        local pid=$(cat "$pid_file")
        local node_num=$(basename "$pid_file" .pid | sed 's/node//')
        local rpc_port=$((BASE_RPC_PORT + node_num - 1))
        
        if kill -0 "$pid" 2>/dev/null; then
            echo -e "${GREEN}Node $node_num: 稼働中 (PID: $pid, RPC: http://localhost:$rpc_port)${NC}"
            running=$((running + 1))
        else
            echo -e "${RED}Node $node_num: 停止${NC}"
            rm -f "$pid_file"
        fi
    done
    
    if [[ $total -eq 0 ]]; then
        echo "起動中のノードはありません"
        echo "起動: ./scripts/run-storage-nodes.sh start [ノード数]"
    else
        echo ""
        echo "稼働: $running / $total"
    fi
}

purge_data() {
    # 先に停止
    stop_nodes
    
    echo -e "${YELLOW}=== Storage Nodeデータ削除 ===${NC}"
    
    # 設定ファイル削除
    rm -f "$PROJECT_DIR"/node*.toml
    
    # データディレクトリ削除
    if [[ -d "$DATA_DIR" ]]; then
        rm -rf "$DATA_DIR"
        echo -e "${GREEN}データ削除: $DATA_DIR${NC}"
    fi
    
    # ログ削除
    if [[ -d "$LOGS_DIR" ]]; then
        rm -rf "$LOGS_DIR"
        echo -e "${GREEN}ログ削除: $LOGS_DIR${NC}"
    fi
    
    echo -e "${GREEN}完了${NC}"
}

case "${1:-status}" in
    start)
        start_nodes "${2:-5}"
        ;;
    stop)
        stop_nodes
        ;;
    status)
        show_status
        ;;
    purge)
        purge_data
        ;;
    *)
        echo "Usage: $0 [start|stop|status|purge] [ノード数]"
        echo "  start [N]  - N個のStorage Nodeを起動（デフォルト: 5）"
        echo "  stop       - 全Storage Nodeを停止"
        echo "  status     - 稼働状況を表示"
        echo "  purge      - 全データを削除"
        exit 1
        ;;
esac

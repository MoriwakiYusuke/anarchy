#!/bin/bash
# 統合テスト用ユーティリティ関数

# エラー時に停止しない（テスト継続のため）
set +e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
NODE_BIN="$PROJECT_DIR/target/release/anarchy-node"
TEST_DATA_DIR="$SCRIPT_DIR/data"
TEST_LOG_DIR="$SCRIPT_DIR/logs"

# カラー出力
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# テスト結果
TESTS_PASSED=0
TESTS_FAILED=0

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((TESTS_PASSED++))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((TESTS_FAILED++))
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# テストデータディレクトリを初期化
init_test_env() {
    rm -rf "$TEST_DATA_DIR" "$TEST_LOG_DIR"
    mkdir -p "$TEST_DATA_DIR" "$TEST_LOG_DIR"
}

# ノードを起動（引数: name, p2p_port, rpc_port, validator, bootnode）
start_node() {
    local name=$1
    local p2p_port=$2
    local rpc_port=$3
    local validator=$4
    local bootnode=$5
    local node_key=$6
    
    local args=(
        "--chain" "local"
        "--base-path" "$TEST_DATA_DIR/$name"
        "--port" "$p2p_port"
        "--rpc-port" "$rpc_port"
        "--prometheus-port" "$((rpc_port + 1000))"
        "--rpc-cors" "all"
        "--unsafe-force-node-key-generation"
    )
    
    # 開発アカウント指定（Alice, Bob, Charlie, Dave, Eve, Ferdie）
    case $name in
        alice|bob|charlie|dave|eve|ferdie)
            args+=("--$name")
            ;;
        *)
            # カスタムノード名の場合
            ;;
    esac
    
    if [ "$validator" = "true" ]; then
        args+=("--validator")
    fi
    
    if [ -n "$bootnode" ]; then
        args+=("--bootnodes" "$bootnode")
    fi
    
    if [ -n "$node_key" ]; then
        args+=("--node-key" "$node_key")
    fi
    
    log_info "Starting node: $name (P2P: $p2p_port, RPC: $rpc_port, Validator: $validator)"
    
    mkdir -p "$TEST_DATA_DIR/$name"
    $NODE_BIN "${args[@]}" > "$TEST_LOG_DIR/$name.log" 2>&1 &
    echo $! > "$TEST_DATA_DIR/$name.pid"
}

# ノードを停止
stop_node() {
    local name=$1
    local pid_file="$TEST_DATA_DIR/$name.pid"
    
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
            sleep 1
            kill -9 "$pid" 2>/dev/null || true
        fi
        rm -f "$pid_file"
    fi
}

# 全ノード停止
stop_all_nodes() {
    log_info "Stopping all test nodes..."
    pkill -f "anarchy-node.*$TEST_DATA_DIR" 2>/dev/null || true
    sleep 2
}

# ノードが起動するまで待機
wait_for_node() {
    local rpc_port=$1
    local timeout=${2:-30}
    local count=0
    
    while [ $count -lt $timeout ]; do
        if curl -s "http://127.0.0.1:$rpc_port" > /dev/null 2>&1; then
            return 0
        fi
        sleep 1
        ((count++))
    done
    return 1
}

# RPCでブロック高を取得
get_block_number() {
    local rpc_port=$1
    local result=$(curl -s -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"chain_getHeader","params":[],"id":1}' \
        "http://127.0.0.1:$rpc_port" 2>/dev/null)
    
    local hex=$(echo "$result" | jq -r '.result.number // "0x0"')
    echo $((hex))
}

# RPCでファイナライズ済みブロック高を取得  
get_finalized_block() {
    local rpc_port=$1
    local result=$(curl -s -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[],"id":1}' \
        "http://127.0.0.1:$rpc_port" 2>/dev/null)
    
    local hash=$(echo "$result" | jq -r '.result // ""')
    if [ -z "$hash" ] || [ "$hash" = "null" ]; then
        echo "0"
        return
    fi
    
    local header=$(curl -s -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"chain_getHeader\",\"params\":[\"$hash\"],\"id\":1}" \
        "http://127.0.0.1:$rpc_port" 2>/dev/null)
    
    local hex=$(echo "$header" | jq -r '.result.number // "0x0"')
    echo $((hex))
}

# RPCでピア数を取得
get_peer_count() {
    local rpc_port=$1
    local result=$(curl -s -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"system_peers","params":[],"id":1}' \
        "http://127.0.0.1:$rpc_port" 2>/dev/null)
    
    echo "$result" | jq '.result | length'
}

# ノードのPeer IDを取得
get_peer_id() {
    local log_file=$1
    local max_attempts=${2:-30}
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if [ -f "$log_file" ]; then
            local peer_id=$(grep -oP 'Local node identity is: \K[a-zA-Z0-9]+' "$log_file" | head -1)
            if [ -n "$peer_id" ]; then
                echo "$peer_id"
                return 0
            fi
        fi
        sleep 1
        ((attempt++))
    done
    return 1
}

# テスト結果サマリーを出力
print_test_summary() {
    echo ""
    echo "=========================================="
    echo "  Test Summary"
    echo "=========================================="
    echo -e "  ${GREEN}Passed:${NC} $TESTS_PASSED"
    echo -e "  ${RED}Failed:${NC} $TESTS_FAILED"
    echo "=========================================="
    
    if [ $TESTS_FAILED -gt 0 ]; then
        return 1
    fi
    return 0
}

# クリーンアップ
cleanup() {
    stop_all_nodes
    rm -rf "$TEST_DATA_DIR" "$TEST_LOG_DIR"
}

# トラップ設定
trap cleanup EXIT

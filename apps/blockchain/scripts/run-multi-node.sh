#!/bin/bash
# 複数ノードでローカルテストネットを起動するスクリプト
# Usage: ./scripts/run-multi-node.sh [start [N]|stop|status|logs] [--tor-mode=off|outbound-only|forced]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
NODE_BIN="$PROJECT_DIR/target/release/anarchy-node"
DATA_DIR="$PROJECT_DIR/data"
LOG_DIR="$PROJECT_DIR/logs"

# Tor mode (default: off)
TOR_MODE="off"

# ノード名（最大10ノード）
NODE_NAMES=(alice bob charlie dave eve ferdie one two nine ten)

# ベースポート
BASE_P2P_PORT=30333
BASE_RPC_PORT=9944
BASE_PROM_PORT=9615

# デフォルトノード数
DEFAULT_NODE_COUNT=3
MAX_NODE_COUNT=10

# カラー出力
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_binary() {
    if [ ! -f "$NODE_BIN" ]; then
        log_error "Node binary not found at $NODE_BIN"
        log_info "Build with: cargo build --release"
        exit 1
    fi
}

create_dirs() {
    mkdir -p "$LOG_DIR"
}

get_alice_peer_id() {
    # Aliceノードが起動するまで待機してPeer IDを取得
    local max_attempts=30
    local attempt=0
    while [ $attempt -lt $max_attempts ]; do
        if [ -f "$LOG_DIR/alice.log" ]; then
            local peer_id=$(grep -oP 'Local node identity is: \K[a-zA-Z0-9]+' "$LOG_DIR/alice.log" | head -1)
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

# 汎用ノード起動関数
# $1: ノード名 (alice, bob, charlie, ...)
# $2: ノードインデックス (0, 1, 2, ...)
# $3: Validator かどうか (true/false)
# $4: bootnode (空文字ならbootnodeなし)
start_node() {
    local name="$1"
    local index="$2"
    local is_validator="$3"
    local bootnode="$4"
    
    local p2p_port=$((BASE_P2P_PORT + index))
    local ws_p2p_port=$((BASE_P2P_PORT + index + 500))  # WebSocket P2P for smoldot (30833, 30834, ...)
    local rpc_port=$((BASE_RPC_PORT + index))
    local prom_port=$((BASE_PROM_PORT + index))
    
    mkdir -p "$DATA_DIR/$name"
    
    local role="Full node"
    local validator_flag=""
    if [ "$is_validator" = "true" ]; then
        role="Validator"
        validator_flag="--validator"
    fi
    
    log_info "Starting $name node ($role)..."
    
    local bootnode_flag=""
    if [ -n "$bootnode" ]; then
        bootnode_flag="--bootnodes $bootnode"
        log_info "  Connecting to bootnode"
    fi
    
    # node-keyはaliceのみ固定（peer ID取得のため）
    local node_key_flag=""
    if [ "$name" = "alice" ]; then
        node_key_flag="--node-key 0000000000000000000000000000000000000000000000000000000000000001"
    fi
    
    # Tor mode support
    local cmd="$NODE_BIN"
    local tor_mode_flag=""
    if [ "$TOR_MODE" != "off" ]; then
        tor_mode_flag="--tor-mode $TOR_MODE"
        if [ "$TOR_MODE" = "forced" ] || [ "$TOR_MODE" = "outbound-only" ]; then
            # Use torsocks wrapper
            if [ -f "$SCRIPT_DIR/anarchy-tor.sh" ]; then
                cmd="$SCRIPT_DIR/anarchy-tor.sh $NODE_BIN"
            else
                log_warn "anarchy-tor.sh not found, using torsocks directly"
                cmd="torsocks $NODE_BIN"
            fi
        fi
    fi
    
    $cmd \
        --chain local \
        --"$name" \
        --base-path "$DATA_DIR/$name" \
        --listen-addr /ip4/0.0.0.0/tcp/$p2p_port \
        --listen-addr /ip4/0.0.0.0/tcp/$ws_p2p_port/ws \
        --rpc-port $rpc_port \
        --prometheus-port $prom_port \
        --rpc-cors all \
        $validator_flag \
        $bootnode_flag \
        $node_key_flag \
        $tor_mode_flag \
        --unsafe-force-node-key-generation \
        > "$LOG_DIR/$name.log" 2>&1 &
    
    echo $! > "$DATA_DIR/$name.pid"
    log_info "$name started (PID: $(cat $DATA_DIR/$name.pid))"
    log_info "  P2P: $p2p_port (TCP), $ws_p2p_port (WS), RPC: $rpc_port"
}

start_all() {
    local node_count="${1:-$DEFAULT_NODE_COUNT}"
    
    # 多重起動チェック
    for name in "${NODE_NAMES[@]}"; do
        local pid_file="$DATA_DIR/$name.pid"
        if [ -f "$pid_file" ]; then
            local pid=$(cat "$pid_file")
            if kill -0 "$pid" 2>/dev/null; then
                log_error "ノードが既に起動中です。先に停止してください: $0 stop"
                exit 1
            fi
            rm -f "$pid_file"
        fi
    done
    
    # バリデーション
    if [ "$node_count" -lt 1 ] || [ "$node_count" -gt $MAX_NODE_COUNT ]; then
        log_error "Node count must be between 1 and $MAX_NODE_COUNT"
        exit 1
    fi
    
    check_binary
    create_dirs
    
    # Validator数を決定（最初の2ノード、または全ノードが2未満なら全部）
    local validator_count=2
    if [ "$node_count" -lt 2 ]; then
        validator_count=$node_count
    fi
    
    log_info "Starting local testnet with $node_count node(s)..."
    for ((i=0; i<node_count; i++)); do
        local name="${NODE_NAMES[$i]}"
        if [ $i -lt $validator_count ]; then
            log_info "  - ${name^}: Validator"
        else
            log_info "  - ${name^}: Full node"
        fi
    done
    log_info ""
    
    # ノードを順に起動
    local bootnode=""
    for ((i=0; i<node_count; i++)); do
        local name="${NODE_NAMES[$i]}"
        local is_validator="false"
        if [ $i -lt $validator_count ]; then
            is_validator="true"
        fi
        
        start_node "$name" "$i" "$is_validator" "$bootnode"
        
        # 最初のノード（alice）起動後にbootnode取得
        if [ $i -eq 0 ]; then
            sleep 3
            local alice_peer_id=$(get_alice_peer_id)
            if [ -z "$alice_peer_id" ]; then
                log_error "Could not get Alice's peer ID"
                exit 1
            fi
            bootnode="/ip4/127.0.0.1/tcp/$BASE_P2P_PORT/p2p/$alice_peer_id"
        else
            sleep 2
        fi
    done
    
    log_info ""
    log_info "=== Testnet Started ($node_count nodes) ==="
    for ((i=0; i<node_count; i++)); do
        local name="${NODE_NAMES[$i]}"
        local rpc_port=$((BASE_RPC_PORT + i))
        local role="Full node"
        if [ $i -lt $validator_count ]; then
            role="Validator"
        fi
        log_info "${name^} ($role) RPC: ws://127.0.0.1:$rpc_port"
    done
    log_info ""
    log_info "View logs: $0 logs [node_name]"
    log_info "Stop with: $0 stop"
    
    # 起動したノード数を記録
    echo "$node_count" > "$DATA_DIR/.node_count"
}

stop_all() {
    log_info "Stopping all nodes..."
    
    for name in "${NODE_NAMES[@]}"; do
        pid_file="$DATA_DIR/$name.pid"
        if [ -f "$pid_file" ]; then
            pid=$(cat "$pid_file")
            if kill -0 "$pid" 2>/dev/null; then
                kill "$pid"
                log_info "Stopped $name (PID: $pid)"
            fi
            rm -f "$pid_file"
        fi
    done
    
    # 念のため残プロセスも終了
    pkill -f "anarchy-node" 2>/dev/null || true
    
    rm -f "$DATA_DIR/.node_count"
    log_info "All nodes stopped"
}

status() {
    log_info "Node status:"
    
    local found_any=false
    for name in "${NODE_NAMES[@]}"; do
        pid_file="$DATA_DIR/$name.pid"
        if [ -f "$pid_file" ]; then
            found_any=true
            pid=$(cat "$pid_file")
            if kill -0 "$pid" 2>/dev/null; then
                echo -e "  $name: ${GREEN}Running${NC} (PID: $pid)"
            else
                echo -e "  $name: ${RED}Stopped${NC} (stale PID file)"
            fi
        fi
    done
    
    if [ "$found_any" = false ]; then
        log_info "  No nodes running"
    fi
}

show_logs() {
    local node="${1:-alice}"
    if [ -f "$LOG_DIR/$node.log" ]; then
        tail -f "$LOG_DIR/$node.log"
    else
        log_error "Log file not found for $node"
    fi
}

purge_data() {
    log_warn "This will delete all chain data!"
    read -p "Are you sure? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        stop_all
        rm -rf "$DATA_DIR"
        rm -rf "$LOG_DIR"
        log_info "All data purged"
    fi
}

usage() {
    echo "Usage: $0 <command> [options] [--tor-mode=MODE]"
    echo ""
    echo "Commands:"
    echo "  start [N]   Start N nodes (default: $DEFAULT_NODE_COUNT, max: $MAX_NODE_COUNT)"
    echo "              First 2 nodes are validators, rest are full nodes"
    echo "  stop        Stop all nodes"
    echo "  status      Show node status"
    echo "  logs [node] View logs (default: alice)"
    echo "  purge       Delete all chain data"
    echo ""
    echo "Tor Modes:"
    echo "  --tor-mode=off           Normal TCP (default, development only)"
    echo "  --tor-mode=outbound-only Outbound via Tor, inbound exposed (WARNING)"
    echo "  --tor-mode=forced        Full anonymity via Tor (requires Onion Service)"
    echo ""
    echo "Examples:"
    echo "  $0 start        # Start 3 nodes (alice/bob validators + charlie full node)"
    echo "  $0 start 5      # Start 5 nodes"
    echo "  $0 start 10     # Start 10 nodes (maximum)"
    echo "  $0 start 3 --tor-mode=outbound-only  # Start 3 nodes with Tor outbound"
    echo ""
    echo "Available node names: ${NODE_NAMES[*]}"
}

# Parse --tor-mode argument
for arg in "$@"; do
    case $arg in
        --tor-mode=*)
            TOR_MODE="${arg#*=}"
            if [[ ! "$TOR_MODE" =~ ^(off|outbound-only|forced)$ ]]; then
                log_error "Invalid tor-mode: $TOR_MODE"
                log_error "Valid values: off, outbound-only, forced"
                exit 1
            fi
            ;;
    esac
done

case "${1:-}" in
    start)
        start_all "${2:-}"
        ;;
    stop)
        stop_all
        ;;
    status)
        status
        ;;
    logs)
        show_logs "${2:-alice}"
        ;;
    purge)
        purge_data
        ;;
    *)
        usage
        ;;
esac

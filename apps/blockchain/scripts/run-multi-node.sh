#!/bin/bash
# 複数ノードでローカルテストネットを起動するスクリプト
# Usage: ./scripts/run-multi-node.sh [start|stop|status|logs]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
NODE_BIN="$PROJECT_DIR/target/release/anarchy-node"
DATA_DIR="$PROJECT_DIR/data"
LOG_DIR="$PROJECT_DIR/logs"

# ノード設定
ALICE_P2P_PORT=30333
ALICE_RPC_PORT=9944
ALICE_PROM_PORT=9615

BOB_P2P_PORT=30334
BOB_RPC_PORT=9945
BOB_PROM_PORT=9616

CHARLIE_P2P_PORT=30335
CHARLIE_RPC_PORT=9946
CHARLIE_PROM_PORT=9617

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
    mkdir -p "$DATA_DIR/alice" "$DATA_DIR/bob" "$DATA_DIR/charlie" "$LOG_DIR"
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

start_alice() {
    log_info "Starting Alice node (Authority)..."
    
    $NODE_BIN \
        --chain local \
        --alice \
        --base-path "$DATA_DIR/alice" \
        --port $ALICE_P2P_PORT \
        --rpc-port $ALICE_RPC_PORT \
        --prometheus-port $ALICE_PROM_PORT \
        --rpc-cors all \
        --validator \
        --node-key 0000000000000000000000000000000000000000000000000000000000000001 \
        --unsafe-force-node-key-generation \
        > "$LOG_DIR/alice.log" 2>&1 &
    
    echo $! > "$DATA_DIR/alice.pid"
    log_info "Alice started (PID: $(cat $DATA_DIR/alice.pid))"
    log_info "  P2P: $ALICE_P2P_PORT, RPC: $ALICE_RPC_PORT"
}

start_bob() {
    log_info "Starting Bob node (Authority)..."
    
    # Aliceのbootnode情報を取得
    local alice_peer_id=$(get_alice_peer_id)
    if [ -z "$alice_peer_id" ]; then
        log_error "Could not get Alice's peer ID. Is Alice running?"
        exit 1
    fi
    
    local bootnode="/ip4/127.0.0.1/tcp/$ALICE_P2P_PORT/p2p/$alice_peer_id"
    log_info "  Connecting to bootnode: $bootnode"
    
    $NODE_BIN \
        --chain local \
        --bob \
        --base-path "$DATA_DIR/bob" \
        --port $BOB_P2P_PORT \
        --rpc-port $BOB_RPC_PORT \
        --prometheus-port $BOB_PROM_PORT \
        --rpc-cors all \
        --validator \
        --bootnodes "$bootnode" \
        --unsafe-force-node-key-generation \
        > "$LOG_DIR/bob.log" 2>&1 &
    
    echo $! > "$DATA_DIR/bob.pid"
    log_info "Bob started (PID: $(cat $DATA_DIR/bob.pid))"
    log_info "  P2P: $BOB_P2P_PORT, RPC: $BOB_RPC_PORT"
}

start_charlie() {
    log_info "Starting Charlie node (Full node)..."
    
    local alice_peer_id=$(get_alice_peer_id)
    if [ -z "$alice_peer_id" ]; then
        log_error "Could not get Alice's peer ID. Is Alice running?"
        exit 1
    fi
    
    local bootnode="/ip4/127.0.0.1/tcp/$ALICE_P2P_PORT/p2p/$alice_peer_id"
    log_info "  Connecting to bootnode: $bootnode"
    
    $NODE_BIN \
        --chain local \
        --charlie \
        --base-path "$DATA_DIR/charlie" \
        --port $CHARLIE_P2P_PORT \
        --rpc-port $CHARLIE_RPC_PORT \
        --prometheus-port $CHARLIE_PROM_PORT \
        --rpc-cors all \
        --bootnodes "$bootnode" \
        --unsafe-force-node-key-generation \
        > "$LOG_DIR/charlie.log" 2>&1 &
    
    echo $! > "$DATA_DIR/charlie.pid"
    log_info "Charlie started (PID: $(cat $DATA_DIR/charlie.pid))"
    log_info "  P2P: $CHARLIE_P2P_PORT, RPC: $CHARLIE_RPC_PORT"
}

start_all() {
    check_binary
    create_dirs
    
    log_info "Starting local testnet with 3 nodes..."
    log_info "  - Alice: Validator"
    log_info "  - Bob: Validator"
    log_info "  - Charlie: Full node"
    log_info ""
    
    start_alice
    sleep 3  # Aliceが起動するまで待機
    start_bob
    sleep 2
    start_charlie
    
    log_info ""
    log_info "=== Testnet Started (3 nodes) ==="
    log_info "Alice (Validator) RPC:   ws://127.0.0.1:$ALICE_RPC_PORT"
    log_info "Bob (Validator) RPC:     ws://127.0.0.1:$BOB_RPC_PORT"
    log_info "Charlie (Full node) RPC: ws://127.0.0.1:$CHARLIE_RPC_PORT"
    log_info ""
    log_info "View logs:"
    log_info "  Alice:   tail -f $LOG_DIR/alice.log"
    log_info "  Bob:     tail -f $LOG_DIR/bob.log"
    log_info "  Charlie: tail -f $LOG_DIR/charlie.log"
    log_info ""
    log_info "Stop with: $0 stop"
}

stop_all() {
    log_info "Stopping all nodes..."
    
    for node in alice bob charlie; do
        pid_file="$DATA_DIR/$node.pid"
        if [ -f "$pid_file" ]; then
            pid=$(cat "$pid_file")
            if kill -0 "$pid" 2>/dev/null; then
                kill "$pid"
                log_info "Stopped $node (PID: $pid)"
            fi
            rm -f "$pid_file"
        fi
    done
    
    # 念のため残プロセスも終了
    pkill -f "anarchy-node" 2>/dev/null || true
    
    log_info "All nodes stopped"
}

status() {
    log_info "Node status:"
    
    for node in alice bob charlie; do
        pid_file="$DATA_DIR/$node.pid"
        if [ -f "$pid_file" ]; then
            pid=$(cat "$pid_file")
            if kill -0 "$pid" 2>/dev/null; then
                echo -e "  $node: ${GREEN}Running${NC} (PID: $pid)"
            else
                echo -e "  $node: ${RED}Stopped${NC} (stale PID file)"
            fi
        else
            echo -e "  $node: ${YELLOW}Not started${NC}"
        fi
    done
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
    echo "Usage: $0 <command>"
    echo ""
    echo "Commands:"
    echo "  start       Start 3 nodes (Alice/Bob validators + Charlie full node)"
    echo "  stop        Stop all nodes"
    echo "  status      Show node status"
    echo "  logs [node] View logs (default: alice)"
    echo "  purge       Delete all chain data"
    echo ""
    echo "Node endpoints after start:"
    echo "  Alice:   ws://127.0.0.1:$ALICE_RPC_PORT"
    echo "  Bob:     ws://127.0.0.1:$BOB_RPC_PORT"
    echo "  Charlie: ws://127.0.0.1:$CHARLIE_RPC_PORT"
}

case "${1:-}" in
    start)
        start_all
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

#!/usr/bin/env bash
# dev-stack.sh — testnet + storage + frontend を一括管理。
#
# 使い方:
#   ./scripts/dev-stack.sh start [--single-node]   # 全部起動 (default 3-node testnet)
#   ./scripts/dev-stack.sh stop                    # 全部停止
#   ./scripts/dev-stack.sh status                  # 各層の稼働状況
#   ./scripts/dev-stack.sh purge                   # stop + 全データ消去
#   ./scripts/dev-stack.sh restart [--single-node] # stop → start
#
# レイヤ:
#   1. blockchain — `apps/blockchain/scripts/run-multi-node.sh` (3-node) または
#                   `cargo run -- --dev` (--single-node 指定時)
#   2. storage    — `apps/storage-node/scripts/run-storage-nodes.sh` (5-node)
#   3. frontend   — `pnpm dev:frontend` (Next.js dev server, port 3000)
#
# 依存関係順に start し、逆順で stop する。各レイヤは前段の readiness を確認してから
# 起動する (storage は chain RPC、frontend は何も待たない)。

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLOCKCHAIN_DIR="$ROOT_DIR/apps/blockchain"
STORAGE_DIR="$ROOT_DIR/apps/storage-node"
FRONTEND_DIR="$ROOT_DIR/apps/frontend"
STATE_DIR="$ROOT_DIR/.dev-stack"
FRONTEND_PID_FILE="$STATE_DIR/frontend.pid"
FRONTEND_LOG_FILE="$STATE_DIR/frontend.log"
SINGLE_NODE_PID_FILE="$STATE_DIR/single-node.pid"
SINGLE_NODE_LOG_FILE="$STATE_DIR/single-node.log"

# ---- カラー ----
if [[ -t 1 ]]; then
    GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'
else
    GREEN=''; YELLOW=''; RED=''; CYAN=''; NC=''
fi

log_info()  { echo -e "${CYAN}[dev-stack]${NC} $*"; }
log_ok()    { echo -e "${GREEN}[dev-stack]${NC} $*"; }
log_warn()  { echo -e "${YELLOW}[dev-stack]${NC} $*" >&2; }
log_error() { echo -e "${RED}[dev-stack]${NC} $*" >&2; }

ensure_state_dir() { mkdir -p "$STATE_DIR"; }

# pid file 経由のプロセス生死判定。生きていれば 0 を返す。
pid_alive() {
    local pid_file=$1
    [[ -f "$pid_file" ]] || return 1
    local pid
    pid="$(cat "$pid_file" 2>/dev/null || true)"
    [[ -n "$pid" ]] || return 1
    kill -0 "$pid" 2>/dev/null
}

# ---- HTTP readiness ----
wait_http_200() {
    local url=$1 label=$2 timeout=${3:-60}
    local deadline=$(( SECONDS + timeout ))
    while (( SECONDS < deadline )); do
        if curl -s -o /dev/null -w "%{http_code}" --max-time 2 "$url" 2>/dev/null | grep -q "200\|405"; then
            log_ok "$label is ready ($url)"
            return 0
        fi
        sleep 1
    done
    log_error "$label not ready within ${timeout}s ($url)"
    return 1
}

wait_chain_rpc() {
    # substrate node の system_health は POST が必要なので curl で空 GET → 405 が返れば OK
    wait_http_200 "http://localhost:9944" "chain RPC" 60
}

# ---- BLOCKCHAIN ----
start_blockchain() {
    if [[ "${SINGLE_NODE:-0}" == "1" ]]; then
        if pid_alive "$SINGLE_NODE_PID_FILE"; then
            log_warn "single-node already running (pid $(cat "$SINGLE_NODE_PID_FILE"))"
            return 0
        fi
        log_info "starting single dev node (cargo run -- --dev --mine --coinbase Alice --randomx-mode light)"
        ensure_state_dir
        # PoW 移行後 (#52, #53): single-node でも `--mine --coinbase` が無いと block が進まない。
        # `package.json` の `dev:node` と一致させる。Alice の AccountId (5Grw…) を coinbase に。
        ( cd "$BLOCKCHAIN_DIR" && nohup cargo run --release -- \
            --dev \
            --mine \
            --coinbase 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY \
            --randomx-mode light \
            > "$SINGLE_NODE_LOG_FILE" 2>&1 & echo $! > "$SINGLE_NODE_PID_FILE" )
    else
        log_info "starting 3-node testnet"
        ( cd "$BLOCKCHAIN_DIR" && ./scripts/run-multi-node.sh start )
    fi
    wait_chain_rpc
}

stop_blockchain() {
    if pid_alive "$SINGLE_NODE_PID_FILE"; then
        log_info "stopping single dev node"
        kill "$(cat "$SINGLE_NODE_PID_FILE")" 2>/dev/null || true
        rm -f "$SINGLE_NODE_PID_FILE"
    fi
    if [[ -f "$BLOCKCHAIN_DIR/scripts/run-multi-node.sh" ]]; then
        ( cd "$BLOCKCHAIN_DIR" && ./scripts/run-multi-node.sh stop ) || log_warn "testnet stop returned non-zero (likely no nodes running)"
    fi
}

status_blockchain() {
    log_info "=== blockchain ==="
    if pid_alive "$SINGLE_NODE_PID_FILE"; then
        echo "  single-node: pid $(cat "$SINGLE_NODE_PID_FILE") (log: $SINGLE_NODE_LOG_FILE)"
    fi
    ( cd "$BLOCKCHAIN_DIR" && ./scripts/run-multi-node.sh status ) || true
}

purge_blockchain() {
    stop_blockchain
    log_info "purging blockchain data"
    if [[ -f "$BLOCKCHAIN_DIR/scripts/run-multi-node.sh" ]]; then
        ( cd "$BLOCKCHAIN_DIR" && ./scripts/run-multi-node.sh purge ) || true
    fi
    # single-node の dev データは cargo の base-path 既定 = $TMPDIR 配下なので明示削除しない
    rm -f "$SINGLE_NODE_LOG_FILE" "$SINGLE_NODE_PID_FILE"
}

# ---- STORAGE ----
start_storage() {
    log_info "starting storage nodes"
    ( cd "$STORAGE_DIR" && ./scripts/run-storage-nodes.sh start )
    wait_http_200 "http://localhost:3030" "storage node 1 RPC" 30 || true
}

stop_storage() {
    log_info "stopping storage nodes"
    ( cd "$STORAGE_DIR" && ./scripts/run-storage-nodes.sh stop ) || log_warn "storage stop returned non-zero"
}

status_storage() {
    log_info "=== storage ==="
    ( cd "$STORAGE_DIR" && ./scripts/run-storage-nodes.sh status ) || true
}

purge_storage() {
    stop_storage
    log_info "purging storage data"
    ( cd "$STORAGE_DIR" && ./scripts/run-storage-nodes.sh purge ) || true
}

# ---- FRONTEND ----
start_frontend() {
    if pid_alive "$FRONTEND_PID_FILE"; then
        log_warn "frontend already running (pid $(cat "$FRONTEND_PID_FILE"))"
        return 0
    fi
    ensure_state_dir
    log_info "starting frontend (next dev) → log: $FRONTEND_LOG_FILE"
    # nohup + setsid で親シェルから切り離す。pnpm 配下の next-server の親プロセス
    # PID を保存しておき、stop 時は同じプロセスグループを kill する。
    ( cd "$ROOT_DIR" && nohup setsid pnpm dev:frontend > "$FRONTEND_LOG_FILE" 2>&1 & echo $! > "$FRONTEND_PID_FILE" )
    log_info "waiting for http://localhost:3000 ..."
    wait_http_200 "http://localhost:3000" "frontend" 120 || {
        log_error "frontend failed to come up; check $FRONTEND_LOG_FILE"
        return 1
    }
}

stop_frontend() {
    if ! pid_alive "$FRONTEND_PID_FILE"; then
        log_warn "frontend not running (no pid file or stale)"
        # 念のため :3000 を listen している next-server を掃除
        local strays
        strays=$(ss -tlnp 2>/dev/null | awk -F'pid=' '/:3000/ {print $2}' | awk -F',' '{print $1}' | sort -u)
        if [[ -n "$strays" ]]; then
            log_warn "killing stray next-server pids: $strays"
            echo "$strays" | xargs -r kill 2>/dev/null || true
        fi
        rm -f "$FRONTEND_PID_FILE"
        return 0
    fi
    local pid
    pid=$(cat "$FRONTEND_PID_FILE")
    log_info "stopping frontend (pgid $pid)"
    # setsid で起動した子は同じ pgid を持つので -- pgid に SIGTERM を送れば全員死ぬ
    kill -TERM -- -"$pid" 2>/dev/null || kill "$pid" 2>/dev/null || true
    sleep 2
    if pid_alive "$FRONTEND_PID_FILE"; then
        log_warn "frontend did not exit, forcing SIGKILL"
        kill -KILL -- -"$pid" 2>/dev/null || true
    fi
    rm -f "$FRONTEND_PID_FILE"
}

status_frontend() {
    log_info "=== frontend ==="
    if pid_alive "$FRONTEND_PID_FILE"; then
        echo "  next dev: pid $(cat "$FRONTEND_PID_FILE") (log: $FRONTEND_LOG_FILE)"
        if curl -s -o /dev/null -w "%{http_code}" --max-time 2 http://localhost:3000 | grep -q "200"; then
            echo "  http://localhost:3000 → 200 OK"
        else
            echo "  http://localhost:3000 → not responding"
        fi
    else
        echo "  not running"
    fi
}

purge_frontend() {
    stop_frontend
    log_info "purging frontend dev cache (.next/)"
    rm -rf "$FRONTEND_DIR/.next"
    rm -f "$FRONTEND_LOG_FILE" "$FRONTEND_PID_FILE"
}

# ---- ENTRYPOINTS ----
cmd_start()   { start_blockchain && start_storage && start_frontend; }
cmd_stop()    { stop_frontend; stop_storage; stop_blockchain; }
cmd_status()  { status_blockchain; status_storage; status_frontend; }
cmd_purge()   { purge_frontend; purge_storage; purge_blockchain; }
cmd_restart() { cmd_stop; sleep 1; cmd_start; }

usage() {
    sed -n '2,/^$/p' "$0" | sed 's/^# *//;s/^#$//'
    exit 1
}

# ---- ARG PARSE ----
SINGLE_NODE=0
ACTION=""
for arg in "$@"; do
    case "$arg" in
        --single-node) SINGLE_NODE=1 ;;
        start|stop|status|purge|restart) ACTION="$arg" ;;
        -h|--help) usage ;;
        *) log_error "unknown arg: $arg"; usage ;;
    esac
done
export SINGLE_NODE

case "${ACTION:-}" in
    start)   cmd_start ;;
    stop)    cmd_stop ;;
    status)  cmd_status ;;
    purge)   cmd_purge ;;
    restart) cmd_restart ;;
    *)       usage ;;
esac

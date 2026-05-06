#!/usr/bin/env bash
# Phase B Task 7: hashrate 急増シミュレーション → DAA (LWMA-3) が target に再収束する確認
#
# シナリオ: 1 ノード mining → 60 ブロック生成 → 追加 5 ノード起動 → 100 ブロック後の
# block time 平均が target 30s ± 50% 以内に収まることを検証

set +e
source "$(dirname "$0")/../utils.sh"
trap - EXIT

NODE_BIN="${NODE_BIN:-./target/release/anarchy-node}"
RANDOMX_MODE="${RANDOMX_MODE:-fast}"

WORKDIR=$(mktemp -d /tmp/anarchy-pow-hj-XXXXXX)
PIDS=()

# ノード設定 (最大 6 ノード)
RPCS=(9944 9955 9966 9977 9988 9999)
P2PS=(30333 30334 30335 30336 30337 30338)
COINBASES=(
    "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
    "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"
    "5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy"
    "5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw"
    "5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL"
    "5GNJqTPyNqANBkUVMN1LPPrxXnFouWXoe2wNSmmEoLctxiZY"
)
NODE_KEYS=(
    "0000000000000000000000000000000000000000000000000000000000000001"
    "0000000000000000000000000000000000000000000000000000000000000002"
    "0000000000000000000000000000000000000000000000000000000000000003"
    "0000000000000000000000000000000000000000000000000000000000000004"
    "0000000000000000000000000000000000000000000000000000000000000005"
    "0000000000000000000000000000000000000000000000000000000000000006"
)

cleanup() {
    log_info "Cleaning up..."
    for pid in "${PIDS[@]}"; do
        kill -INT "$pid" 2>/dev/null || true
    done
    sleep 3
    for pid in "${PIDS[@]}"; do
        kill -KILL "$pid" 2>/dev/null || true
    done
}
trap cleanup EXIT

start_node() {
    local i=$1
    local bootnode="${2:-}"
    local base="$WORKDIR/node$i"
    mkdir -p "$base"
    local args=(
        --base-path "$base"
        --chain dev
        --mine
        --coinbase "${COINBASES[$i]}"
        --randomx-mode "$RANDOMX_MODE"
        --node-key "${NODE_KEYS[$i]}"
        --port "${P2PS[$i]}"
        --rpc-port "${RPCS[$i]}"
        --validator
        --no-prometheus --no-telemetry
        --tmp
    )
    if [ -n "$bootnode" ]; then
        args+=(--bootnodes "$bootnode")
    fi
    "$NODE_BIN" "${args[@]}" > "$WORKDIR/node$i.log" 2>&1 &
    PIDS+=($!)
    log_info "Node $i started (PID ${PIDS[$i]})"
}

best_block() {
    local rpc=$1
    local hex=$(curl -s -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader","params":[]}' \
        "http://127.0.0.1:$rpc" | jq -r '.result.number // "0x0"')
    echo $((hex))
}

log_info "=== PoW hashrate jump test ==="

# ノード 0 (boot) を起動
start_node 0
sleep 5
BOOT_PEER_ID=$(grep -oP "Local node identity is: \K[A-Za-z0-9]+" "$WORKDIR/node0.log" | head -1)
BOOTNODE="/ip4/127.0.0.1/tcp/${P2PS[0]}/p2p/$BOOT_PEER_ID"

log_info "Single-node mining for 60 blocks (Phase 1 baseline)..."
while true; do
    BEST=$(best_block "${RPCS[0]}")
    if [ "$BEST" -ge 60 ]; then
        log_info "Phase 1 complete at block $BEST"
        BEFORE_TIME=$(date +%s)
        BEFORE_BLOCK=$BEST
        break
    fi
    sleep 10
done

log_info "Spawning 5 more miners (Phase 2: hashrate 6x)..."
for i in 1 2 3 4 5; do
    start_node "$i" "$BOOTNODE"
done

# 100 ブロック追加生成を待つ (DAA 再収束)
TARGET_BLOCK=$((BEFORE_BLOCK + 100))
log_info "Waiting until block $TARGET_BLOCK (Phase 2 + 100 blocks)..."
while true; do
    BEST=$(best_block "${RPCS[0]}")
    if [ "$BEST" -ge "$TARGET_BLOCK" ]; then
        AFTER_TIME=$(date +%s)
        AFTER_BLOCK=$BEST
        break
    fi
    sleep 30
done

ELAPSED=$((AFTER_TIME - BEFORE_TIME))
BLOCKS_PRODUCED=$((AFTER_BLOCK - BEFORE_BLOCK))
AVG_BLOCK_TIME=$((ELAPSED / BLOCKS_PRODUCED))
log_info "Produced $BLOCKS_PRODUCED blocks in ${ELAPSED}s (avg ${AVG_BLOCK_TIME}s/block, target 30s)"

# 検証: target 30s ± 50% 以内 (15s〜45s)
if [ "$AVG_BLOCK_TIME" -lt 15 ] || [ "$AVG_BLOCK_TIME" -gt 45 ]; then
    log_fail "DAA failed to converge: avg block time ${AVG_BLOCK_TIME}s outside [15, 45]s"
    exit 1
fi
log_success "DAA converged: avg block time ${AVG_BLOCK_TIME}s within target ± 50%"
log_success "=== hashrate_jump.sh PASSED ==="
exit 0

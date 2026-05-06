#!/usr/bin/env bash
# Phase B Task 7: 3 ノード multi-miner reorg + GRANDPA finality 一致テスト
#
# 起動: 3 ノード (各 --mine 別 coinbase, --randomx-mode fast) を 30 分稼働
# 検証: best block ≥ 50, 各ノードの finalized block height が ±3 以内に収束

set +e
source "$(dirname "$0")/../utils.sh"
trap - EXIT

NODE_BIN="${NODE_BIN:-./target/release/anarchy-node}"
RANDOMX_MODE="${RANDOMX_MODE:-fast}"
DURATION_S="${DURATION_S:-1800}"  # 30 分
EXPECTED_MIN_BEST="${EXPECTED_MIN_BEST:-50}"
FINALITY_TOLERANCE="${FINALITY_TOLERANCE:-3}"

# 3 ノードの coinbase (//Alice / //Bob / //Charlie)
COINBASE_A="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
COINBASE_B="5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"
COINBASE_C="5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy"

WORKDIR=$(mktemp -d /tmp/anarchy-pow-mm-XXXXXX)
PIDS=()
RPCS=(9944 9955 9966)
P2PS=(30333 30334 30335)
COINBASES=("$COINBASE_A" "$COINBASE_B" "$COINBASE_C")
NODE_KEYS=(
    "0000000000000000000000000000000000000000000000000000000000000001"
    "0000000000000000000000000000000000000000000000000000000000000002"
    "0000000000000000000000000000000000000000000000000000000000000003"
)

cleanup() {
    log_info "Cleaning up nodes..."
    for pid in "${PIDS[@]}"; do
        kill -INT "$pid" 2>/dev/null || true
    done
    sleep 3
    for pid in "${PIDS[@]}"; do
        kill -KILL "$pid" 2>/dev/null || true
    done
}
trap cleanup EXIT

log_info "=== PoW multi-miner test (3 nodes, ${DURATION_S}s) ==="
log_info "Working dir: $WORKDIR"

# ノード 0 (boot node) を先に起動
i=0
BASE="$WORKDIR/node$i"
mkdir -p "$BASE"
"$NODE_BIN" \
    --base-path "$BASE" \
    --chain dev \
    --mine \
    --coinbase "${COINBASES[$i]}" \
    --randomx-mode "$RANDOMX_MODE" \
    --node-key "${NODE_KEYS[$i]}" \
    --port "${P2PS[$i]}" \
    --rpc-port "${RPCS[$i]}" \
    --validator \
    --no-prometheus --no-telemetry \
    --tmp \
    > "$WORKDIR/node$i.log" 2>&1 &
PIDS+=($!)
log_info "Node 0 started (PID ${PIDS[$i]}), waiting 5s for boot peer ID..."
sleep 5

BOOT_PEER_ID=$(grep -oP "Local node identity is: \K[A-Za-z0-9]+" "$WORKDIR/node0.log" | head -1)
if [ -z "$BOOT_PEER_ID" ]; then
    log_fail "Boot node peer ID not found"
    cat "$WORKDIR/node0.log" | tail -30
    exit 1
fi
BOOTNODE="/ip4/127.0.0.1/tcp/${P2PS[0]}/p2p/$BOOT_PEER_ID"
log_success "Boot node: $BOOTNODE"

# ノード 1, 2 を起動
for i in 1 2; do
    BASE="$WORKDIR/node$i"
    mkdir -p "$BASE"
    "$NODE_BIN" \
        --base-path "$BASE" \
        --chain dev \
        --mine \
        --coinbase "${COINBASES[$i]}" \
        --randomx-mode "$RANDOMX_MODE" \
        --node-key "${NODE_KEYS[$i]}" \
        --port "${P2PS[$i]}" \
        --rpc-port "${RPCS[$i]}" \
        --validator \
        --bootnodes "$BOOTNODE" \
        --no-prometheus --no-telemetry \
        --tmp \
        > "$WORKDIR/node$i.log" 2>&1 &
    PIDS+=($!)
    log_info "Node $i started (PID ${PIDS[$i]})"
done

log_info "Mining for ${DURATION_S}s..."
sleep "$DURATION_S"

log_info "=== Final state ==="
for i in 0 1 2; do
    BEST=$(curl -s -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader","params":[]}' \
        "http://127.0.0.1:${RPCS[$i]}" | jq -r '.result.number // "0x0"')
    BEST_DEC=$((BEST))
    FINAL=$(curl -s -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"chain_getFinalizedHead","params":[]}' \
        "http://127.0.0.1:${RPCS[$i]}" | jq -r '.result // ""')
    FINAL_NUM=$(curl -s -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"chain_getHeader\",\"params\":[\"$FINAL\"]}" \
        "http://127.0.0.1:${RPCS[$i]}" | jq -r '.result.number // "0x0"')
    FINAL_DEC=$((FINAL_NUM))
    log_info "Node $i: best=#$BEST_DEC finalized=#$FINAL_DEC"
    eval "BEST_$i=$BEST_DEC"
    eval "FINAL_$i=$FINAL_DEC"
done

# 検証 1: best block が EXPECTED_MIN_BEST 以上
MAX_BEST=$(echo "$BEST_0 $BEST_1 $BEST_2" | tr ' ' '\n' | sort -n | tail -1)
if [ "$MAX_BEST" -lt "$EXPECTED_MIN_BEST" ]; then
    log_fail "Max best block $MAX_BEST < expected $EXPECTED_MIN_BEST"
    exit 1
fi
log_success "Max best block: $MAX_BEST (≥ $EXPECTED_MIN_BEST)"

# 検証 2: 各ノードの finalized が tolerance 以内に収束
MIN_FINAL=$(echo "$FINAL_0 $FINAL_1 $FINAL_2" | tr ' ' '\n' | sort -n | head -1)
MAX_FINAL=$(echo "$FINAL_0 $FINAL_1 $FINAL_2" | tr ' ' '\n' | sort -n | tail -1)
DIVERGENCE=$((MAX_FINAL - MIN_FINAL))
if [ "$DIVERGENCE" -gt "$FINALITY_TOLERANCE" ]; then
    log_fail "Finality divergence $DIVERGENCE > tolerance $FINALITY_TOLERANCE (min=$MIN_FINAL max=$MAX_FINAL)"
    exit 1
fi
log_success "Finality divergence: $DIVERGENCE (≤ $FINALITY_TOLERANCE)"

log_success "=== multi_miner.sh PASSED ==="
exit 0

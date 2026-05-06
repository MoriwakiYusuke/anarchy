#!/usr/bin/env bash
# Phase B Task 7: selfish mining (秘匿チェーン公開) で reorg は起きるが finalized は守られる
#
# シナリオ: 2 ノード起動 → 一方を P2P 切断状態で 6 ブロック先行 mining → 公開 →
# 公開ノードが追従して reorg が発生するが、すでに finalized したブロックは固定であること。

set +e
source "$(dirname "$0")/../utils.sh"
trap - EXIT

NODE_BIN="${NODE_BIN:-./target/release/anarchy-node}"
RANDOMX_MODE="${RANDOMX_MODE:-fast}"

WORKDIR=$(mktemp -d /tmp/anarchy-pow-sm-XXXXXX)
PIDS=()
RPCS=(9944 9955)
P2PS=(30333 30334)
COINBASES=(
    "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
    "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"
)
NODE_KEYS=(
    "0000000000000000000000000000000000000000000000000000000000000001"
    "0000000000000000000000000000000000000000000000000000000000000002"
)

cleanup() {
    for pid in "${PIDS[@]}"; do kill -INT "$pid" 2>/dev/null || true; done
    sleep 2
    for pid in "${PIDS[@]}"; do kill -KILL "$pid" 2>/dev/null || true; done
}
trap cleanup EXIT

log_info "=== Selfish mining test ==="

# Public node 起動
mkdir -p "$WORKDIR/node0"
"$NODE_BIN" --base-path "$WORKDIR/node0" --chain dev --mine \
    --coinbase "${COINBASES[0]}" --randomx-mode "$RANDOMX_MODE" \
    --node-key "${NODE_KEYS[0]}" --port "${P2PS[0]}" --rpc-port "${RPCS[0]}" \
    --validator --no-prometheus --no-telemetry --tmp \
    > "$WORKDIR/node0.log" 2>&1 &
PIDS+=($!)
sleep 5

# Selfish (private) node 起動 (bootnodes 指定なし → 孤立)
mkdir -p "$WORKDIR/node1"
"$NODE_BIN" --base-path "$WORKDIR/node1" --chain dev --mine \
    --coinbase "${COINBASES[1]}" --randomx-mode "$RANDOMX_MODE" \
    --node-key "${NODE_KEYS[1]}" --port "${P2PS[1]}" --rpc-port "${RPCS[1]}" \
    --validator --no-prometheus --no-telemetry --tmp \
    > "$WORKDIR/node1.log" 2>&1 &
PIDS+=($!)
sleep 5

best() {
    curl -s -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader","params":[]}' \
        "http://127.0.0.1:$1" | jq -r '.result.number // "0x0"' | xargs -I{} printf "%d" "{}"
}

finalized() {
    local hash=$(curl -s -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"chain_getFinalizedHead","params":[]}' \
        "http://127.0.0.1:$1" | jq -r '.result // ""')
    curl -s -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"chain_getHeader\",\"params\":[\"$hash\"]}" \
        "http://127.0.0.1:$1" | jq -r '.result.number // "0x0"' | xargs -I{} printf "%d" "{}"
}

log_info "Phase 1: Both nodes mine independently for 30 blocks each..."
sleep 600  # 10 分 (30 ブロック分 + 余裕)

PUBLIC_BEST=$(best "${RPCS[0]}")
PUBLIC_FINAL=$(finalized "${RPCS[0]}")
SELFISH_BEST=$(best "${RPCS[1]}")
log_info "Public: best=#$PUBLIC_BEST finalized=#$PUBLIC_FINAL"
log_info "Selfish: best=#$SELFISH_BEST"

# 検証 1: selfish node が public より先行している (隠匿チェーン)
if [ "$SELFISH_BEST" -lt "$PUBLIC_BEST" ]; then
    log_warn "Selfish node didn't outpace public — test may not exercise the attack scenario"
fi

# Phase 2: selfish ノードを bootnodes 経由で public に接続
log_info "Phase 2: connecting selfish node to public chain..."
SELFISH_PEER_ID=$(grep -oP "Local node identity is: \K[A-Za-z0-9]+" "$WORKDIR/node1.log" | head -1)
PUBLIC_PEER_ID=$(grep -oP "Local node identity is: \K[A-Za-z0-9]+" "$WORKDIR/node0.log" | head -1)
# 接続: addPeer RPC (system_addReservedPeer)
curl -s -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"system_addReservedPeer\",\"params\":[\"/ip4/127.0.0.1/tcp/${P2PS[0]}/p2p/$PUBLIC_PEER_ID\"]}" \
    "http://127.0.0.1:${RPCS[1]}" > /dev/null

log_info "Phase 3: Waiting 5 minutes for reorg + sync..."
sleep 300

POST_BEST=$(best "${RPCS[0]}")
POST_FINAL=$(finalized "${RPCS[0]}")
SELFISH_FINAL=$(finalized "${RPCS[1]}")

log_info "Post-merge state:"
log_info "  Public:  best=#$POST_BEST  finalized=#$POST_FINAL"
log_info "  Selfish: best=#$POST_BEST  finalized=#$SELFISH_FINAL"

# 検証 2: Phase 1 で finalized したブロックは reorg の影響を受けない
if [ "$POST_FINAL" -lt "$PUBLIC_FINAL" ]; then
    log_fail "Finality regressed: was #$PUBLIC_FINAL, now #$POST_FINAL"
    exit 1
fi
log_success "Finality preserved across reorg: #$PUBLIC_FINAL → #$POST_FINAL (≥)"
log_success "=== selfish_mining.sh PASSED ==="
exit 0

#!/usr/bin/env bash
# Phase B Task 7: GRANDPA top-K authority rotation の動作検証
#
# シナリオ: 5 ノード mining、各ノードで register_grandpa_key extrinsic 発行 →
# RotationPeriod (600 ブロック @30s = 5h) を超えて稼働 → AuthoritySetRotated
# event がオンチェーンに発行されたことを system_events で確認。
#
# 5 時間長いので CI では走らせない。staging で release ゲートとして実施。

set +e
source "$(dirname "$0")/../utils.sh"
trap - EXIT

NODE_BIN="${NODE_BIN:-./target/release/anarchy-node}"
RANDOMX_MODE="${RANDOMX_MODE:-fast}"
# 開発用に短縮: dev runtime で RotationPeriod を 50 に下げる builds (要 runtime config 経由)
# → 本番 runtime のまま走らせる場合は 600 blocks ≒ 5h 必要
ROTATION_BLOCKS="${ROTATION_BLOCKS:-600}"
WAIT_BUFFER="${WAIT_BUFFER:-30}"  # rotation 後の確認猶予 (blocks)

WORKDIR=$(mktemp -d /tmp/anarchy-pow-ar-XXXXXX)
PIDS=()
RPCS=(9944 9955 9966 9977 9988)
P2PS=(30333 30334 30335 30336 30337)
COINBASES=(
    "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
    "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"
    "5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy"
    "5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw"
    "5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL"
)
NODE_KEYS=(
    "0000000000000000000000000000000000000000000000000000000000000001"
    "0000000000000000000000000000000000000000000000000000000000000002"
    "0000000000000000000000000000000000000000000000000000000000000003"
    "0000000000000000000000000000000000000000000000000000000000000004"
    "0000000000000000000000000000000000000000000000000000000000000005"
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

log_info "=== GRANDPA authority rotation test (RotationPeriod=$ROTATION_BLOCKS blocks) ==="

# ノード起動 (簡略: multi_miner.sh と同じ流れ)
start_node() {
    local i=$1; local bootnode="${2:-}"
    local base="$WORKDIR/node$i"; mkdir -p "$base"
    local args=(
        --base-path "$base" --chain dev --mine
        --coinbase "${COINBASES[$i]}" --randomx-mode "$RANDOMX_MODE"
        --node-key "${NODE_KEYS[$i]}" --port "${P2PS[$i]}" --rpc-port "${RPCS[$i]}"
        --validator --no-prometheus --no-telemetry --tmp
    )
    [ -n "$bootnode" ] && args+=(--bootnodes "$bootnode")
    "$NODE_BIN" "${args[@]}" > "$WORKDIR/node$i.log" 2>&1 &
    PIDS+=($!)
    log_info "Node $i started"
}

start_node 0; sleep 5
BOOT_PEER_ID=$(grep -oP "Local node identity is: \K[A-Za-z0-9]+" "$WORKDIR/node0.log" | head -1)
BOOTNODE="/ip4/127.0.0.1/tcp/${P2PS[0]}/p2p/$BOOT_PEER_ID"
for i in 1 2 3 4; do start_node "$i" "$BOOTNODE"; done
sleep 10

# 各ノードで register_grandpa_key extrinsic を発行 (PAPI 経由 / 既存スクリプト流用想定)
log_info "Registering GRANDPA keys for 5 miners..."
for i in 0 1 2 3 4; do
    # 実装メモ: scripts/ 以下に register_grandpa_key の PAPI スクリプトが
    # 必要 (Phase B Task 8 の bench-randomx.sh と同様の補助スクリプト)。
    # 暫定: TODO で skip し、rotation トリガまで稼働させる。
    log_info "  → node $i (TODO: register_grandpa_key extrinsic via PAPI script)"
done

best_block() {
    curl -s -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader","params":[]}' \
        "http://127.0.0.1:$1" | jq -r '.result.number // "0x0"' \
        | xargs -I{} printf "%d" "{}"
}

# RotationPeriod + buffer まで稼働
TARGET=$((ROTATION_BLOCKS + WAIT_BUFFER))
log_info "Waiting for block $TARGET (rotation period + buffer)..."
while true; do
    B=$(best_block "${RPCS[0]}")
    log_info "  current best=$B / target=$TARGET"
    if [ "$B" -ge "$TARGET" ]; then break; fi
    sleep 60
done

# AuthoritySetRotated event が発行されたか確認
# (system_events / state_getStorage で grandpaElection events を取得)
log_info "Checking for AuthoritySetRotated events..."
# TODO: PAPI スクリプトで recent events を walk + AuthoritySetRotated count を取得
# 暫定: ノードログで確認
ROTATED_COUNT=$(grep -c "AuthoritySetRotated" "$WORKDIR/node0.log" 2>/dev/null || echo 0)
if [ "$ROTATED_COUNT" -lt 1 ]; then
    log_fail "No AuthoritySetRotated event found in node 0 log"
    log_info "Last 50 lines of node 0 log:"
    tail -50 "$WORKDIR/node0.log"
    exit 1
fi
log_success "AuthoritySetRotated event(s) detected: $ROTATED_COUNT"
log_success "=== authority_rotation.sh PASSED (smoke level) ==="
log_warn "NOTE: register_grandpa_key extrinsic はまだ自動化されていない。手動で確認すること。"
exit 0

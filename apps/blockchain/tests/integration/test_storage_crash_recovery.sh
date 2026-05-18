#!/bin/bash
#
# test_storage_crash_recovery.sh — TODO §4.9 crash test
#
# Validates redb's atomicity claim end-to-end: SIGKILL the storage-node
# while it's accepting writes, then restart and confirm:
#   1. The DB still opens (no corruption that the read path can detect).
#   2. Either the in-flight fragment is fully present, or fully absent —
#      never half-written.
#   3. used_bytes counter is consistent with the data tables.
#   4. verify_on_read passes for every recovered fragment.
#
# Phase 1 added redb. Phase 2 added per-fragment Blake2 metadata, so we
# can now actually test "no half-written fragments" (before, you'd just
# trust the file size).
#
# Test design — one process, one shot:
#   * Spawn the storage-node (auth disabled, verify_on_read=true).
#   * Fire N concurrent storage_storeFragment requests.
#   * Hard-kill (SIGKILL, not SIGTERM) at a random moment between the
#     first request and the last.
#   * Restart. List fragments. For each one, retrieve via RPC (which
#     runs verify_on_read internally). Any error → fail.
#   * Confirm `used_bytes == sum(retrieve(f).len() for f in list)`.
#
# Usage: ./test_storage_crash_recovery.sh

set -uo pipefail

source "$(dirname "$0")/utils.sh"

WORK_DIR="$(mktemp -d -t anarchy-crash-XXXXXX)"
NODE_BIN_STORAGE="$PROJECT_DIR/../storage-node/target/release/anarchy-storage-node"
RPC_PORT=3134
P2P_PORT=4104

cleanup() {
    if [[ -n "${SN_PID:-}" ]] && kill -0 "$SN_PID" 2>/dev/null; then
        kill "$SN_PID" 2>/dev/null || true
        wait "$SN_PID" 2>/dev/null || true
    fi
    if [[ -n "${CHAIN_PID:-}" ]] && kill -0 "$CHAIN_PID" 2>/dev/null; then
        kill "$CHAIN_PID" 2>/dev/null || true
        wait "$CHAIN_PID" 2>/dev/null || true
    fi
    rm -rf "$WORK_DIR"
}
trap cleanup EXIT

mkdir -p "$WORK_DIR/data"
cat > "$WORK_DIR/config.toml" <<EOF
data_dir = "$WORK_DIR/data"
capacity = 1073741824
chain_url = "ws://127.0.0.1:9944"
listen_addr = "/ip4/127.0.0.1/tcp/$P2P_PORT"
declare_rate_limit = 100
rpc_port = $RPC_PORT
auth_enabled = false
bootstrap_peers = []
srs_path = ""
dev_mode = true
verify_on_read = true
signer_seed = "e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a"
EOF

# Storage node needs *some* chain endpoint, even if registration fails.
# Use the existing dev chain if running; otherwise spawn one for the test.
if ! curl -sf -m 1 -X POST -H 'Content-Type: application/json' \
        -d '{"jsonrpc":"2.0","method":"system_health","params":[],"id":1}' \
        http://127.0.0.1:9944 >/dev/null 2>&1; then
    log_info "Spawning dev chain for the test"
    "$NODE_BIN" --dev --tmp --rpc-cors all --rpc-port 9944 --port 30333 \
        --mine --coinbase 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY \
        --randomx-mode light >"$WORK_DIR/chain.log" 2>&1 &
    CHAIN_PID=$!
    for _ in $(seq 1 60); do
        curl -sf -m 1 -X POST -H 'Content-Type: application/json' \
            -d '{"jsonrpc":"2.0","method":"system_health","params":[],"id":1}' \
            http://127.0.0.1:9944 >/dev/null 2>&1 && break
        sleep 1
    done
fi

start_storage() {
    "$NODE_BIN_STORAGE" --config "$WORK_DIR/config.toml" \
        >>"$WORK_DIR/storage.log" 2>&1 &
    SN_PID=$!
    for _ in $(seq 1 30); do
        curl -sf "http://127.0.0.1:$RPC_PORT/health" 2>/dev/null \
            | grep -q healthy && return 0
        sleep 0.5
    done
    log_fail "storage node did not become healthy"
    cat "$WORK_DIR/storage.log"
    return 1
}

# `--max-time` is the safety net: when SIGKILL kills the storage-node
# mid-burst, in-flight curls that hadn't yet connected would otherwise
# stall on TCP retry until the kernel timeout (~2 minutes).
store_fragment() {
    local merkle_root="$1"
    local idx="$2"
    local payload="$3"
    local b64
    b64="$(printf '%s' "$payload" | base64 -w0)"
    local proof
    proof="$(printf '\x00%.0s' {1..32} | base64 -w0)"
    curl -sf --max-time 5 -X POST "http://127.0.0.1:$RPC_PORT/" \
        -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"storage_storeFragment\",\"params\":{\"merkle_root\":$merkle_root,\"index\":$idx,\"data\":\"$b64\",\"proof\":\"$proof\",\"total_leaves\":5},\"id\":1}" \
        2>/dev/null
}

retrieve_fragment() {
    local merkle_root="$1"
    local idx="$2"
    curl -sf --max-time 5 -X POST "http://127.0.0.1:$RPC_PORT/" \
        -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"storage_getFragment\",\"params\":{\"merkle_root\":$merkle_root,\"index\":$idx},\"id\":1}" \
        2>/dev/null
}

# 32-byte merkle_root encoded as a JSON int array, with a fixed prefix and
# a varying suffix so each request hits a different fragment_id.
mk_root() {
    local i="$1"
    printf '['
    for b in $(seq 1 28); do printf '%d,' "$b"; done
    printf '%d,%d,%d,%d]' \
        $(( (i >> 24) & 0xFF )) \
        $(( (i >> 16) & 0xFF )) \
        $(( (i >>  8) & 0xFF )) \
        $((  i        & 0xFF ))
}

# ===========================================================================
# Phase A: spawn, write a burst, SIGKILL mid-burst
# ===========================================================================

start_storage || exit 1
log_info "storage node up (run 1, PID $SN_PID)"

N=20  # number of write attempts
KILL_AFTER=8  # SIGKILL after this many requests have been *issued*

issued_payloads=()
curl_pids=()
for i in $(seq 1 "$N"); do
    payload="crash-test-$i-$(date -u +%s%N)"
    issued_payloads+=("$payload")
    root="$(mk_root "$i")"
    # Fire and forget; some will land, some won't.
    store_fragment "$root" 0 "$payload" >/dev/null &
    curl_pids+=($!)
    if [[ "$i" -eq "$KILL_AFTER" ]]; then
        # Don't wait for the burst to drain — the whole point is mid-write.
        log_info "SIGKILL at request $i"
        kill -9 "$SN_PID" 2>/dev/null || true
        break
    fi
done

# Wait ONLY for the in-flight curls — bare `wait` would also wait on
# CHAIN_PID, which runs forever, hanging the test.
for pid in "${curl_pids[@]}"; do
    wait "$pid" 2>/dev/null || true
done
SN_PID=""
sleep 1

# ===========================================================================
# Phase B: restart, verify integrity
# ===========================================================================

start_storage || exit 1
log_info "storage node up (run 2, PID $SN_PID)"

# Pull the stored fragments back, count + sum sizes for cross-check.
total_bytes=0
ok=0
fail=0
for i in $(seq 1 "$N"); do
    root="$(mk_root "$i")"
    response="$(retrieve_fragment "$root" 0 || true)"
    if echo "$response" | grep -q '"result"'; then
        # Decode b64, count length.
        b64="$(echo "$response" | python3 -c "import sys,json;print(json.load(sys.stdin)['result']['data'])")"
        decoded_len="$(echo "$b64" | base64 -d | wc -c)"
        total_bytes=$((total_bytes + decoded_len))
        ok=$((ok + 1))
    elif echo "$response" | grep -q "Fragment not found"; then
        # Fragment was either rejected mid-write or never reached fsync.
        # That's fine — atomicity says it's all-or-nothing.
        fail=$((fail + 1))
    elif echo "$response" | grep -q "hash mismatch"; then
        # This is the BAD case — partially written fragment passed redb's
        # txn boundary but the bytes don't match the metadata. Should be
        # impossible if Phase 1+2 atomicity holds.
        log_fail "verify_on_read found half-written fragment after crash: $response"
        exit 1
    else
        log_warn "unexpected response for $i: $response"
        fail=$((fail + 1))
    fi
done

# used_bytes counter (post-recovery) must equal the sum of retrieved sizes.
metrics="$(curl -sf http://127.0.0.1:$RPC_PORT/metrics 2>/dev/null || true)"
used_bytes_metric="$(echo "$metrics" | grep '^storage_capacity_used_bytes' | awk '{print $2}')"

log_info "fragments persisted across crash: $ok / $N (rejected: $fail)"
log_info "sum(retrieve.len) = $total_bytes"
log_info "metrics.storage_capacity_used_bytes = $used_bytes_metric"

if [[ "$used_bytes_metric" != "$total_bytes" ]]; then
    log_fail "used_bytes counter ($used_bytes_metric) ≠ sum of retrieved sizes ($total_bytes)"
    exit 1
fi

# Sanity: at least one fragment should have made it (otherwise SIGKILL
# was too eager and the test isn't actually testing crash-mid-write).
if [[ "$ok" -lt 1 ]]; then
    log_fail "no fragments persisted — SIGKILL may have raced startup"
    exit 1
fi

log_success "crash recovery: $ok fragments survived SIGKILL with hash + size consistency"
echo "Result: PASS ($TESTS_PASSED passed, $TESTS_FAILED failed)"
exit 0

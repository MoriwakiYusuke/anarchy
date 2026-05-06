#!/usr/bin/env bash
# Phase B Task 8: RandomX hashrate を実測して initial_difficulty を算出
#
# Usage:
#   ./scripts/bench-randomx.sh [DURATION_S] [TARGET_BLOCK_SECS]
#
# Defaults: DURATION_S=60, TARGET_BLOCK_SECS=30
#
# Output: stdout に推奨 initial_difficulty を出力 (chain_spec.rs の "difficulty" 欄に焼く想定)

set -euo pipefail

DURATION_S="${1:-60}"
TARGET_BLOCK_SECS="${2:-30}"

NODE_BIN="${NODE_BIN:-./apps/blockchain/target/release/anarchy-node}"
COINBASE="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"

if [ ! -x "$NODE_BIN" ]; then
    echo "ERROR: $NODE_BIN not found. Run 'cd apps/blockchain && cargo build --release -p anarchy-node' first." >&2
    exit 1
fi

WORKDIR=$(mktemp -d /tmp/bench-randomx-XXXXXX)
LOGFILE="$WORKDIR/bench.log"

cleanup() {
    [ -n "${NODE_PID:-}" ] && kill -INT "$NODE_PID" 2>/dev/null || true
    sleep 2
    [ -n "${NODE_PID:-}" ] && kill -KILL "$NODE_PID" 2>/dev/null || true
}
trap cleanup EXIT

echo "=== RandomX hashrate benchmark ===" >&2
echo "Duration: ${DURATION_S}s, target block time: ${TARGET_BLOCK_SECS}s" >&2

RUST_LOG="info,pow-miner=debug" "$NODE_BIN" --dev --mine \
    --coinbase "$COINBASE" --randomx-mode fast \
    --base-path "$WORKDIR" --no-prometheus --no-telemetry \
    > "$LOGFILE" 2>&1 &
NODE_PID=$!
sleep "$DURATION_S"

# 最後の hashrate update ログから hps を取得
# 形式: "⛏  hashrate update: total=N hps=X.X"
HPS=$(grep -oP "hashrate update: total=\d+ hps=\K[0-9.]+" "$LOGFILE" | tail -1 || echo "0")
if [ "$HPS" = "0" ]; then
    # debug ログが出てない場合は info で 1 度出る "new build" が無いか確認、
    # でなければ手動で総ハッシュ数を生成ブロックから推定
    echo "WARN: hashrate update log not found. Falling back to block-count estimate." >&2
    BLOCKS=$(grep -c "Imported #" "$LOGFILE" || echo 0)
    if [ "$BLOCKS" -lt 1 ]; then
        echo "ERROR: No blocks imported in ${DURATION_S}s. Bench failed." >&2
        echo "Log tail:" >&2
        tail -30 "$LOGFILE" >&2
        exit 1
    fi
    # 概算: block ごと平均 nonce/2 とすると total_hashes = blocks * MIN_DIFF
    # MIN_DIFF=100 (現行 dev runtime) で上限見積もり
    # → 単純に blocks/sec を report
    BPS=$(echo "scale=3; $BLOCKS / $DURATION_S" | bc)
    echo "Blocks/sec: $BPS" >&2
    HPS="N/A (use blocks_per_sec=$BPS)"
fi

echo "Measured hashrate: ${HPS} H/s" >&2
echo "" >&2

# 推奨 initial_difficulty: hashrate × target_block_secs
# 例: 500 H/s × 30s = 15_000 → 30 秒に 1 ブロック平均
if [[ "$HPS" =~ ^[0-9.]+$ ]]; then
    DIFF=$(echo "scale=0; $HPS * $TARGET_BLOCK_SECS / 1" | bc)
    echo "Recommended initial_difficulty: $DIFF" >&2
    echo "" >&2
    echo "$DIFF"
else
    echo "$HPS"
fi

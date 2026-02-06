#!/bin/bash
# ノード復旧テスト
# テスト内容:
#   1. ノードがデータを永続化している
#   2. クラッシュ後に再起動してチェーンを復元できる
#   3. 復旧後に他ノードと同期できる
#   4. 状態（ストレージ）が保持されている

set +e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

echo "=========================================="
echo "  Test: Node Recovery"
echo "=========================================="

init_test_env

# ノード設定
ALICE_P2P=43333
ALICE_RPC=43944
BOB_P2P=43334
BOB_RPC=43945
CHARLIE_P2P=43335
CHARLIE_RPC=43946

# Step 1: ノード起動
log_info "Step 1: Starting all nodes..."
start_node "alice" $ALICE_P2P $ALICE_RPC "true" "" "0000000000000000000000000000000000000000000000000000000000000004"

if ! wait_for_node $ALICE_RPC 30; then
    log_fail "Alice failed to start"
    exit 1
fi

ALICE_PEER_ID=$(get_peer_id "$TEST_LOG_DIR/alice.log" 30)
BOOTNODE="/ip4/127.0.0.1/tcp/$ALICE_P2P/p2p/$ALICE_PEER_ID"

start_node "bob" $BOB_P2P $BOB_RPC "true" "$BOOTNODE"
start_node "charlie" $CHARLIE_P2P $CHARLIE_RPC "false" "$BOOTNODE"

if ! wait_for_node $BOB_RPC 30 || ! wait_for_node $CHARLIE_RPC 30; then
    log_fail "Nodes failed to start"
    exit 1
fi

# Step 2: ブロック生成を待機
log_info "Step 2: Waiting for block production (20 seconds)..."
sleep 20

ALICE_HEIGHT_1=$(get_block_number $ALICE_RPC)
CHARLIE_HEIGHT_1=$(get_block_number $CHARLIE_RPC)

log_info "Before crash - Alice: $ALICE_HEIGHT_1, Charlie: $CHARLIE_HEIGHT_1"

if [ "$CHARLIE_HEIGHT_1" -lt 3 ]; then
    log_fail "Not enough blocks produced"
    exit 1
fi

# Step 3: Charlieの状態を確認（Genesis hash）
GENESIS_BEFORE=$(curl -s -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"chain_getBlockHash","params":[0],"id":1}' \
    "http://127.0.0.1:$CHARLIE_RPC" | jq -r '.result')
    
log_info "Genesis hash: $GENESIS_BEFORE"

# Step 4: Charlieをクラッシュさせる（SIGKILL）
log_info "Step 4: Simulating node crash (SIGKILL)..."
CHARLIE_PID=$(cat "$TEST_DATA_DIR/charlie.pid")
kill -9 "$CHARLIE_PID" 2>/dev/null || true
sleep 2

# Charlieのデータディレクトリが存在することを確認
if [ -d "$TEST_DATA_DIR/charlie" ]; then
    log_success "Charlie data directory preserved"
else
    log_fail "Charlie data directory lost"
fi

# Step 5: さらにブロックを生成
log_info "Step 5: Continuing block production (15 seconds)..."
sleep 15

ALICE_HEIGHT_2=$(get_block_number $ALICE_RPC)
log_info "During recovery - Alice: $ALICE_HEIGHT_2"

# Step 6: Charlieを再起動（同じデータディレクトリを使用）
log_info "Step 6: Restarting Charlie with existing data..."
start_node "charlie" $CHARLIE_P2P $CHARLIE_RPC "false" "$BOOTNODE"

if ! wait_for_node $CHARLIE_RPC 30; then
    log_fail "Charlie failed to restart"
    exit 1
fi

# Step 7: 復旧を確認
log_info "Step 7: Verifying recovery..."
sleep 10

# テスト1: Genesis hashが一致
GENESIS_AFTER=$(curl -s -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"chain_getBlockHash","params":[0],"id":1}' \
    "http://127.0.0.1:$CHARLIE_RPC" | jq -r '.result')

if [ "$GENESIS_BEFORE" = "$GENESIS_AFTER" ]; then
    log_success "Genesis hash preserved after recovery"
else
    log_fail "Genesis hash mismatch (before: $GENESIS_BEFORE, after: $GENESIS_AFTER)"
fi

# テスト2: Charlieがチェーンに追いついている
CHARLIE_HEIGHT_2=$(get_block_number $CHARLIE_RPC)
ALICE_HEIGHT_3=$(get_block_number $ALICE_RPC)

log_info "After recovery - Alice: $ALICE_HEIGHT_3, Charlie: $CHARLIE_HEIGHT_2"

DIFF=$((ALICE_HEIGHT_3 - CHARLIE_HEIGHT_2))
DIFF=${DIFF#-}

if [ "$DIFF" -le 3 ]; then
    log_success "Charlie synchronized after recovery (diff: $DIFF blocks)"
else
    log_warn "Charlie still catching up (diff: $DIFF blocks)"
    # 追加待機
    sleep 20
    CHARLIE_HEIGHT_3=$(get_block_number $CHARLIE_RPC)
    ALICE_HEIGHT_4=$(get_block_number $ALICE_RPC)
    DIFF2=$((ALICE_HEIGHT_4 - CHARLIE_HEIGHT_3))
    DIFF2=${DIFF2#-}
    
    if [ "$DIFF2" -le 3 ]; then
        log_success "Charlie finally synchronized (diff: $DIFF2 blocks)"
    else
        log_fail "Charlie failed to synchronize (diff: $DIFF2 blocks)"
    fi
fi

# テスト3: 復旧前に存在したブロックを取得できる
log_info "Verifying historical block access..."
BLOCK_5_HASH=$(curl -s -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"chain_getBlockHash","params":[5],"id":1}' \
    "http://127.0.0.1:$CHARLIE_RPC" | jq -r '.result')

if [ -n "$BLOCK_5_HASH" ] && [ "$BLOCK_5_HASH" != "null" ]; then
    log_success "Historical block accessible (block 5: ${BLOCK_5_HASH:0:20}...)"
else
    log_fail "Could not access historical block"
fi

# テスト4: ピア接続が復旧
CHARLIE_PEERS=$(get_peer_count $CHARLIE_RPC)
if [ "$CHARLIE_PEERS" -ge 1 ]; then
    log_success "Charlie reconnected to $CHARLIE_PEERS peer(s)"
else
    log_fail "Charlie has no peers after recovery"
fi

# Step 8: 追加のデータ整合性確認
log_info "Step 8: Additional integrity checks..."
sleep 10

# 全ノードのファイナライズブロックを比較
FINALIZED_ALICE=$(get_finalized_block $ALICE_RPC)
FINALIZED_BOB=$(get_finalized_block $BOB_RPC)
FINALIZED_CHARLIE=$(get_finalized_block $CHARLIE_RPC)

log_info "Finalized blocks - Alice: $FINALIZED_ALICE, Bob: $FINALIZED_BOB, Charlie: $FINALIZED_CHARLIE"

# テスト5: ファイナリティの一貫性
if [ "$FINALIZED_CHARLIE" -ge "$((FINALIZED_ALICE - 3))" ]; then
    log_success "Finality state consistent across nodes"
else
    log_fail "Finality inconsistent (Charlie: $FINALIZED_CHARLIE, Alice: $FINALIZED_ALICE)"
fi

print_test_summary

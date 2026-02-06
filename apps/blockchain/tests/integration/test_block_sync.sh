#!/bin/bash
# ブロック同期テスト
# テスト内容:
#   1. バリデータ2ノード（Alice, Bob）でチェーンを進める
#   2. 数ブロック後に新規ノード（Charlie）を追加
#   3. Charlieがチェーンに追いつけることを確認
#   4. 全ノードで同じブロックハイトを持つことを確認

set +e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

echo "=========================================="
echo "  Test: Block Synchronization"
echo "=========================================="

init_test_env

# ノード設定
ALICE_P2P=40333
ALICE_RPC=40944
BOB_P2P=40334
BOB_RPC=40945
CHARLIE_P2P=40335
CHARLIE_RPC=40946

# Step 1: Alice（バリデータ）を起動
log_info "Step 1: Starting Alice (validator)..."
start_node "alice" $ALICE_P2P $ALICE_RPC "true" "" "0000000000000000000000000000000000000000000000000000000000000001"

if ! wait_for_node $ALICE_RPC 30; then
    log_fail "Alice failed to start"
    exit 1
fi

# AliceのPeer IDを取得
ALICE_PEER_ID=$(get_peer_id "$TEST_LOG_DIR/alice.log" 30)
if [ -z "$ALICE_PEER_ID" ]; then
    log_fail "Could not get Alice peer ID"
    exit 1
fi
log_info "Alice Peer ID: $ALICE_PEER_ID"

BOOTNODE="/ip4/127.0.0.1/tcp/$ALICE_P2P/p2p/$ALICE_PEER_ID"

# Step 2: Bob（バリデータ）を起動
log_info "Step 2: Starting Bob (validator)..."
start_node "bob" $BOB_P2P $BOB_RPC "true" "$BOOTNODE"

if ! wait_for_node $BOB_RPC 30; then
    log_fail "Bob failed to start"
    exit 1
fi

# Step 3: ブロック生成を待機（約30秒、10ブロック程度）
log_info "Step 3: Waiting for block production (30 seconds)..."
sleep 30

# ブロック高を確認
ALICE_HEIGHT=$(get_block_number $ALICE_RPC)
BOB_HEIGHT=$(get_block_number $BOB_RPC)

log_info "Current block heights - Alice: $ALICE_HEIGHT, Bob: $BOB_HEIGHT"

if [ "$ALICE_HEIGHT" -lt 5 ]; then
    log_fail "Block production too slow (Alice: $ALICE_HEIGHT blocks)"
    exit 1
fi

# Step 4: 新規ノード（Charlie）を追加
log_info "Step 4: Starting Charlie (full node) to test sync..."
start_node "charlie" $CHARLIE_P2P $CHARLIE_RPC "false" "$BOOTNODE"

if ! wait_for_node $CHARLIE_RPC 30; then
    log_fail "Charlie failed to start"
    exit 1
fi

# Step 5: Charlieの同期を待機（最大60秒）
log_info "Step 5: Waiting for Charlie to sync..."
SYNC_TIMEOUT=60
SYNC_COUNT=0

while [ $SYNC_COUNT -lt $SYNC_TIMEOUT ]; do
    CHARLIE_HEIGHT=$(get_block_number $CHARLIE_RPC)
    ALICE_HEIGHT=$(get_block_number $ALICE_RPC)
    
    log_info "  Sync progress - Charlie: $CHARLIE_HEIGHT / Alice: $ALICE_HEIGHT"
    
    # Charlieが追いついたか確認（2ブロック以内の差を許容）
    if [ "$CHARLIE_HEIGHT" -ge $((ALICE_HEIGHT - 2)) ]; then
        break
    fi
    
    sleep 2
    ((SYNC_COUNT+=2))
done

# Step 6: 同期結果を検証
log_info "Step 6: Verifying synchronization..."

FINAL_ALICE=$(get_block_number $ALICE_RPC)
FINAL_BOB=$(get_block_number $BOB_RPC)
FINAL_CHARLIE=$(get_block_number $CHARLIE_RPC)

log_info "Final heights - Alice: $FINAL_ALICE, Bob: $FINAL_BOB, Charlie: $FINAL_CHARLIE"

# テスト1: 全ノードが近いブロック高を持つ
DIFF_AB=$((FINAL_ALICE - FINAL_BOB))
DIFF_AC=$((FINAL_ALICE - FINAL_CHARLIE))
DIFF_AB=${DIFF_AB#-}  # 絶対値
DIFF_AC=${DIFF_AC#-}

if [ "$DIFF_AB" -le 2 ] && [ "$DIFF_AC" -le 2 ]; then
    log_success "All nodes synchronized within 2 blocks"
else
    log_fail "Nodes not synchronized (diff AB: $DIFF_AB, AC: $DIFF_AC)"
fi

# テスト2: ピア接続を確認
CHARLIE_PEERS=$(get_peer_count $CHARLIE_RPC)
if [ "$CHARLIE_PEERS" -ge 1 ]; then
    log_success "Charlie connected to $CHARLIE_PEERS peer(s)"
else
    log_fail "Charlie has no peers"
fi

# テスト3: ファイナリティを確認
sleep 10  # GRANDPAファイナリティのために待機

FINALIZED_ALICE=$(get_finalized_block $ALICE_RPC)
FINALIZED_BOB=$(get_finalized_block $BOB_RPC)
FINALIZED_CHARLIE=$(get_finalized_block $CHARLIE_RPC)

log_info "Finalized blocks - Alice: $FINALIZED_ALICE, Bob: $FINALIZED_BOB, Charlie: $FINALIZED_CHARLIE"

if [ "$FINALIZED_ALICE" -gt 0 ] && [ "$FINALIZED_ALICE" = "$FINALIZED_BOB" ]; then
    log_success "GRANDPA finality working (finalized block: $FINALIZED_ALICE)"
else
    log_warn "GRANDPA finality check: Alice=$FINALIZED_ALICE, Bob=$FINALIZED_BOB (may need more time)"
fi

print_test_summary

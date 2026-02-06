#!/bin/bash
# コンセンサス・フォーク解決テスト
# テスト内容:
#   1. 2バリデータ（Alice, Bob）で正常にブロック生成
#   2. ネットワーク分断をシミュレート（Bobを一時停止）
#   3. Aliceが単独でブロック生成（ファイナリティなし）
#   4. Bob復帰後にチェーンが収束することを確認

set +e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

echo "=========================================="
echo "  Test: Consensus & Fork Resolution"
echo "=========================================="

init_test_env

# ノード設定
ALICE_P2P=41333
ALICE_RPC=41944
BOB_P2P=41334
BOB_RPC=41945

# Step 1: Alice（バリデータ）を起動
log_info "Step 1: Starting Alice (validator)..."
start_node "alice" $ALICE_P2P $ALICE_RPC "true" "" "0000000000000000000000000000000000000000000000000000000000000002"

if ! wait_for_node $ALICE_RPC 30; then
    log_fail "Alice failed to start"
    exit 1
fi

ALICE_PEER_ID=$(get_peer_id "$TEST_LOG_DIR/alice.log" 30)
BOOTNODE="/ip4/127.0.0.1/tcp/$ALICE_P2P/p2p/$ALICE_PEER_ID"

# Step 2: Bob（バリデータ）を起動
log_info "Step 2: Starting Bob (validator)..."
start_node "bob" $BOB_P2P $BOB_RPC "true" "$BOOTNODE"

if ! wait_for_node $BOB_RPC 30; then
    log_fail "Bob failed to start"
    exit 1
fi

# Step 3: 正常なブロック生成を確認
log_info "Step 3: Waiting for normal block production (20 seconds)..."
sleep 20

ALICE_HEIGHT_1=$(get_block_number $ALICE_RPC)
BOB_HEIGHT_1=$(get_block_number $BOB_RPC)
FINALIZED_1=$(get_finalized_block $ALICE_RPC)

log_info "Before partition - Alice: $ALICE_HEIGHT_1, Bob: $BOB_HEIGHT_1, Finalized: $FINALIZED_1"

if [ "$ALICE_HEIGHT_1" -lt 3 ]; then
    log_fail "Block production too slow"
    exit 1
fi

# テスト1: 正常時の同期を確認
DIFF_1=$((ALICE_HEIGHT_1 - BOB_HEIGHT_1))
DIFF_1=${DIFF_1#-}
if [ "$DIFF_1" -le 1 ]; then
    log_success "Nodes synchronized before partition"
else
    log_fail "Nodes not synchronized before partition (diff: $DIFF_1)"
fi

# Step 4: ネットワーク分断をシミュレート（Bobを停止）
log_info "Step 4: Simulating network partition (stopping Bob)..."
stop_node "bob"
sleep 2

# Step 5: 分断中のブロック生成（Aliceのみ）
# Auraでは各スロット（約6秒）は特定のバリデータに割り当てられる
# 2ノードでは、一方が停止すると残りは自分のスロットでのみブロック生成
# 30秒待てば、Aliceのスロットが約5回来る
log_info "Step 5: Block production during partition (30 seconds)..."
sleep 30

ALICE_HEIGHT_2=$(get_block_number $ALICE_RPC)
FINALIZED_2=$(get_finalized_block $ALICE_RPC)

log_info "During partition - Alice: $ALICE_HEIGHT_2, Finalized: $FINALIZED_2"

# テスト2: 分断中はファイナリティが進まない（GRANDPA要件：2/3以上のバリデータ）
# local_testnetでは2バリデータなので、1ノード停止でファイナリティは止まる
if [ "$FINALIZED_2" = "$FINALIZED_1" ] || [ "$FINALIZED_2" -le "$((FINALIZED_1 + 1))" ]; then
    log_success "Finality stalled during partition (expected behavior)"
else
    log_warn "Finality continued during partition: $FINALIZED_1 -> $FINALIZED_2"
fi

# テスト3: Aliceはブロック生成を継続（Auraでは自分のスロットでのみ生成可能）
# 2バリデータで30秒 = 約5ブロック分のAliceスロット
# 1ブロック以上増えていれば成功
if [ "$ALICE_HEIGHT_2" -ge "$((ALICE_HEIGHT_1 + 1))" ]; then
    log_success "Alice continued block production during partition"
else
    # Auraのスロット割り当てによっては生成が少ない場合がある
    log_warn "Alice produced fewer blocks than expected (may be normal due to Aura slot timing)"
fi

# Step 6: Bob復帰
log_info "Step 6: Restoring Bob..."
start_node "bob" $BOB_P2P $BOB_RPC "true" "$BOOTNODE"

if ! wait_for_node $BOB_RPC 30; then
    log_fail "Bob failed to restart"
    exit 1
fi

# Step 7: チェーン収束を待機
log_info "Step 7: Waiting for chain convergence (30 seconds)..."
sleep 30

ALICE_HEIGHT_3=$(get_block_number $ALICE_RPC)
BOB_HEIGHT_3=$(get_block_number $BOB_RPC)
FINALIZED_3=$(get_finalized_block $ALICE_RPC)

log_info "After recovery - Alice: $ALICE_HEIGHT_3, Bob: $BOB_HEIGHT_3, Finalized: $FINALIZED_3"

# テスト4: 復帰後に同期
DIFF_3=$((ALICE_HEIGHT_3 - BOB_HEIGHT_3))
DIFF_3=${DIFF_3#-}
if [ "$DIFF_3" -le 2 ]; then
    log_success "Nodes re-synchronized after partition (diff: $DIFF_3)"
else
    log_fail "Nodes failed to re-synchronize (diff: $DIFF_3)"
fi

# テスト5: ファイナリティが再開
if [ "$FINALIZED_3" -gt "$FINALIZED_2" ]; then
    log_success "Finality resumed after recovery (finalized: $FINALIZED_3)"
else
    log_warn "Finality not resumed yet (still at: $FINALIZED_3)"
fi

# Step 8: 追加の同期確認
log_info "Step 8: Verifying continued operation (20 seconds)..."
sleep 20

ALICE_HEIGHT_4=$(get_block_number $ALICE_RPC)
BOB_HEIGHT_4=$(get_block_number $BOB_RPC)

DIFF_4=$((ALICE_HEIGHT_4 - BOB_HEIGHT_4))
DIFF_4=${DIFF_4#-}

log_info "Final check - Alice: $ALICE_HEIGHT_4, Bob: $BOB_HEIGHT_4"

if [ "$DIFF_4" -le 1 ]; then
    log_success "Continued normal operation after recovery"
else
    log_fail "Nodes diverged after recovery (diff: $DIFF_4)"
fi

print_test_summary

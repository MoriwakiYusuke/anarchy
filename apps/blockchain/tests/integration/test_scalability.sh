#!/bin/bash
# スケーラビリティテスト（多ノード協調）
# テスト内容:
#   1. N個のノードを起動
#   2. 全ノードが同期できることを確認
#   3. ブロック伝播の一貫性を検証  
#   4. 大規模ネットワークでの安定性確認
#
# 使用方法: ./test_scalability.sh [node_count]
#   デフォルト: 10ノード
#   最大: 20ノード（リソース制約）

set +e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

# ノード数（引数またはデフォルト）
NODE_COUNT=${1:-10}
MAX_NODES=20

if [ "$NODE_COUNT" -gt "$MAX_NODES" ]; then
    log_warn "Node count limited to $MAX_NODES (requested: $NODE_COUNT)"
    NODE_COUNT=$MAX_NODES
fi

echo "=========================================="
echo "  Test: Scalability ($NODE_COUNT nodes)"
echo "=========================================="

init_test_env

# ベースポート
BASE_P2P_PORT=44333
BASE_RPC_PORT=44944

# バリデータ数（2つ固定 - Alice, Bob）
VALIDATOR_COUNT=2

declare -a NODE_NAMES
declare -a NODE_RPC_PORTS

# Step 1: ノード名とポートを設定
log_info "Step 1: Configuring $NODE_COUNT nodes..."

for i in $(seq 0 $((NODE_COUNT - 1))); do
    case $i in
        0) NODE_NAMES[$i]="alice" ;;
        1) NODE_NAMES[$i]="bob" ;;
        2) NODE_NAMES[$i]="charlie" ;;
        3) NODE_NAMES[$i]="dave" ;;
        4) NODE_NAMES[$i]="eve" ;;
        5) NODE_NAMES[$i]="ferdie" ;;
        *) NODE_NAMES[$i]="node_$i" ;;
    esac
    NODE_RPC_PORTS[$i]=$((BASE_RPC_PORT + i))
done

# Step 2: Alice（バリデータ1）を起動
log_info "Step 2: Starting validators..."
start_node "alice" $BASE_P2P_PORT $BASE_RPC_PORT "true" "" "0000000000000000000000000000000000000000000000000000000000000005"

if ! wait_for_node $BASE_RPC_PORT 30; then
    log_fail "Alice failed to start"
    exit 1
fi

ALICE_PEER_ID=$(get_peer_id "$TEST_LOG_DIR/alice.log" 30)
BOOTNODE="/ip4/127.0.0.1/tcp/$BASE_P2P_PORT/p2p/$ALICE_PEER_ID"


# Step 3: Bob（バリデータ2）を起動
start_node "bob" $((BASE_P2P_PORT + 1)) $((BASE_RPC_PORT + 1)) "true" "$BOOTNODE"

if ! wait_for_node $((BASE_RPC_PORT + 1)) 30; then
    log_fail "Bob failed to start"
    exit 1
fi

log_success "2 validators started"

# Step 4: 残りのフルノードを起動
log_info "Step 4: Starting $((NODE_COUNT - 2)) full nodes..."

START_TIME=$(date +%s)

for i in $(seq 2 $((NODE_COUNT - 1))); do
    name="${NODE_NAMES[$i]}"
    p2p_port=$((BASE_P2P_PORT + i))
    rpc_port=$((BASE_RPC_PORT + i))
    
    start_node "$name" $p2p_port $rpc_port "false" "$BOOTNODE"
    
    # 過負荷を防ぐため少し間隔を空ける
    sleep 1
done

# 全ノードの起動を待機
log_info "Waiting for all nodes to start..."
FAILED_NODES=0

for i in $(seq 2 $((NODE_COUNT - 1))); do
    rpc_port=$((BASE_RPC_PORT + i))
    if ! wait_for_node $rpc_port 60; then
        log_warn "Node ${NODE_NAMES[$i]} failed to start"
        ((FAILED_NODES++))
    fi
done

END_TIME=$(date +%s)
STARTUP_TIME=$((END_TIME - START_TIME))

if [ $FAILED_NODES -eq 0 ]; then
    log_success "All $NODE_COUNT nodes started in ${STARTUP_TIME}s"
else
    log_warn "$FAILED_NODES nodes failed to start"
fi

# Step 5: ネットワーク形成を待機
log_info "Step 5: Waiting for network formation (30 seconds)..."
sleep 30

# Step 6: ピア接続を確認
log_info "Step 6: Verifying peer connections..."

TOTAL_PEERS=0
MIN_PEERS=999
MAX_PEERS=0

for i in $(seq 0 $((NODE_COUNT - 1))); do
    rpc_port=$((BASE_RPC_PORT + i))
    peers=$(get_peer_count $rpc_port 2>/dev/null || echo "0")
    
    if [ "$peers" -lt "$MIN_PEERS" ]; then MIN_PEERS=$peers; fi
    if [ "$peers" -gt "$MAX_PEERS" ]; then MAX_PEERS=$peers; fi
    TOTAL_PEERS=$((TOTAL_PEERS + peers))
done

AVG_PEERS=$((TOTAL_PEERS / NODE_COUNT))

log_info "Peer counts - Min: $MIN_PEERS, Max: $MAX_PEERS, Avg: $AVG_PEERS"

if [ "$MIN_PEERS" -ge 1 ]; then
    log_success "All nodes have at least 1 peer"
else
    log_fail "Some nodes have no peers"
fi

# Step 7: ブロック同期を確認
log_info "Step 7: Checking block synchronization..."
sleep 20

declare -a HEIGHTS
MIN_HEIGHT=999999
MAX_HEIGHT=0

for i in $(seq 0 $((NODE_COUNT - 1))); do
    rpc_port=$((BASE_RPC_PORT + i))
    height=$(get_block_number $rpc_port 2>/dev/null || echo "0")
    HEIGHTS[$i]=$height
    
    if [ "$height" -lt "$MIN_HEIGHT" ]; then MIN_HEIGHT=$height; fi
    if [ "$height" -gt "$MAX_HEIGHT" ]; then MAX_HEIGHT=$height; fi
done

HEIGHT_DIFF=$((MAX_HEIGHT - MIN_HEIGHT))

log_info "Block heights - Min: $MIN_HEIGHT, Max: $MAX_HEIGHT, Diff: $HEIGHT_DIFF"

if [ "$HEIGHT_DIFF" -le 5 ]; then
    log_success "All nodes synchronized within 5 blocks"
else
    log_warn "Block height variance is high: $HEIGHT_DIFF blocks"
fi

# Step 8: ブロック伝播速度をテスト
log_info "Step 8: Testing block propagation..."

# 現在の最高ブロック
START_HEIGHT=$MAX_HEIGHT

# 数ブロック待機
sleep 18  # 約6ブロック（3秒/ブロック）

# 再度確認
declare -a NEW_HEIGHTS
NEW_MIN=999999
NEW_MAX=0

for i in $(seq 0 $((NODE_COUNT - 1))); do
    rpc_port=$((BASE_RPC_PORT + i))
    height=$(get_block_number $rpc_port 2>/dev/null || echo "0")
    NEW_HEIGHTS[$i]=$height
    
    if [ "$height" -lt "$NEW_MIN" ]; then NEW_MIN=$height; fi
    if [ "$height" -gt "$NEW_MAX" ]; then NEW_MAX=$height; fi
done

PROPAGATION_DIFF=$((NEW_MAX - NEW_MIN))
BLOCKS_PRODUCED=$((NEW_MAX - START_HEIGHT))

log_info "Block propagation - Produced: $BLOCKS_PRODUCED, Height diff: $PROPAGATION_DIFF"

if [ "$PROPAGATION_DIFF" -le 3 ]; then
    log_success "Block propagation working correctly"
else
    log_fail "Block propagation delay detected"
fi

# Step 9: ファイナリティ確認
log_info "Step 9: Checking finality across nodes..."
sleep 10

declare -a FINALIZED
FIN_MIN=999999
FIN_MAX=0

for i in $(seq 0 $((NODE_COUNT - 1))); do
    rpc_port=$((BASE_RPC_PORT + i))
    fin=$(get_finalized_block $rpc_port 2>/dev/null || echo "0")
    FINALIZED[$i]=$fin
    
    if [ "$fin" -lt "$FIN_MIN" ]; then FIN_MIN=$fin; fi
    if [ "$fin" -gt "$FIN_MAX" ]; then FIN_MAX=$fin; fi
done

FIN_DIFF=$((FIN_MAX - FIN_MIN))

log_info "Finalized blocks - Min: $FIN_MIN, Max: $FIN_MAX, Diff: $FIN_DIFF"

if [ "$FIN_MIN" -gt 0 ] && [ "$FIN_DIFF" -le 3 ]; then
    log_success "Finality consistent across $NODE_COUNT nodes"
else
    log_warn "Finality variance detected"
fi

# Step 10: 一部ノード停止後の耐障害性
log_info "Step 10: Testing fault tolerance (stopping 2 full nodes)..."

# 2つのフルノードを停止
if [ "$NODE_COUNT" -ge 5 ]; then
    stop_node "${NODE_NAMES[3]}"
    stop_node "${NODE_NAMES[4]}"
    
    sleep 15
    
    # Aliceが動作継続していることを確認
    ALICE_HEIGHT_AFTER=$(get_block_number $BASE_RPC_PORT)
    ALICE_HEIGHT_BEFORE=$NEW_MAX
    
    if [ "$ALICE_HEIGHT_AFTER" -gt "$ALICE_HEIGHT_BEFORE" ]; then
        log_success "Network continues operating after node failures"
    else
        log_fail "Network stalled after node failures"
    fi
else
    log_info "Skipping fault tolerance test (need at least 5 nodes)"
fi

# サマリー
echo ""
echo "=========================================="
echo "  Scalability Test Results"
echo "=========================================="
echo "  Nodes tested:     $NODE_COUNT"
echo "  Startup time:     ${STARTUP_TIME}s"
echo "  Avg peer count:   $AVG_PEERS"
echo "  Block sync diff:  $HEIGHT_DIFF"
echo "  Final height:     $NEW_MAX"
echo "  Finalized block:  $FIN_MIN"
echo "=========================================="

print_test_summary

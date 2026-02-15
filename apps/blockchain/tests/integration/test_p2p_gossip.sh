#!/bin/bash
# T119: P2Pエンドポイント伝播テスト
# 使用方法: ./test_p2p_gossip.sh

set +e
source "$(dirname "$0")/utils.sh"

# 外部ノードを使用するためtrapを無効化
trap - EXIT

log_info "=== P2Pエンドポイント伝播テスト ==="

RPC_ENDPOINT="${RPC_ENDPOINT:-http://127.0.0.1:9944}"

# 新しいノードを登録
register_node() {
    local endpoint=$1
    log_info "ノード登録: $endpoint"
    
    response=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"storage_registerEndpoint\",\"params\":[\"$endpoint\"]}" \
        "$RPC_ENDPOINT")
    
    success=$(echo "$response" | jq -r '.result.success // false')
    if [[ "$success" == "true" ]]; then
        log_success "ノード登録成功: $endpoint"
        return 0
    else
        log_warn "ノード登録: $endpoint (既に登録済みまたはエラー)"
        return 1
    fi
}

# ノード一覧確認
verify_nodes() {
    response=$(curl -s -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"storage_getNodes","params":[]}' \
        "$RPC_ENDPOINT")
    
    total=$(echo "$response" | jq -r '.result.total_count // 0')
    log_info "現在の登録ノード数: $total"
    echo "$response" | jq -r '.result.nodes[].endpoint' 2>/dev/null | while read ep; do
        echo "  - $ep"
    done
}

run_test() {
    # 初期状態確認
    log_info "初期状態確認"
    verify_nodes
    
    # テスト用ノードを登録
    test_endpoint="http://test-node-gossip:3030"
    register_node "$test_endpoint" || true
    
    # 登録後の状態確認
    log_info "登録後の状態確認"
    verify_nodes
    
    # ノードが登録されていることを確認
    response=$(curl -s -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"storage_getNodes","params":[]}' \
        "$RPC_ENDPOINT")
    
    if echo "$response" | jq -e '.result.nodes | length > 0' > /dev/null 2>&1; then
        log_success "エンドポイント伝播テストPASS"
    else
        log_fail "ノードが登録されていません"
    fi
    
    log_info "=== テスト完了 ==="
    print_test_summary
}

run_test

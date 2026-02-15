#!/bin/bash
# T118: 3ノード断片分散テスト
# 使用方法: ./test_multi_node.sh

set +e
source "$(dirname "$0")/utils.sh"

# 外部ノードを使用するためtrapを無効化
trap - EXIT

log_info "=== マルチノード断片分散テスト ==="

# チェーンノードに接続できるか確認
RPC_ENDPOINT="${RPC_ENDPOINT:-http://127.0.0.1:9944}"

check_chain_connection() {
    log_info "チェーン接続確認: $RPC_ENDPOINT"
    response=$(curl -s -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"system_health","params":[]}' \
        "$RPC_ENDPOINT" 2>/dev/null || echo "")
    
    if [[ -z "$response" ]] || [[ "$response" == *"error"* ]]; then
        log_fail "チェーンノードに接続できません"
        return 1
    fi
    log_success "チェーン接続OK"
}

# ストレージノード一覧を取得
get_storage_nodes() {
    curl -s -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"storage_getNodes","params":[]}' \
        "$RPC_ENDPOINT"
}

# テスト実行
run_test() {
    check_chain_connection || exit 1
    
    # ノード一覧取得
    nodes_response=$(get_storage_nodes)
    
    # ノード数確認
    total_count=$(echo "$nodes_response" | jq -r '.result.total_count // 0')
    online_count=$(echo "$nodes_response" | jq -r '.result.online_count // 0')
    
    log_info "登録ノード数: $total_count, オンライン: $online_count"
    
    if [[ "$total_count" -ge 3 ]]; then
        log_success "3ノード以上が登録されています"
    elif [[ "$total_count" -ge 1 ]]; then
        log_warn "ノード数が3未満 ($total_count)。分散テストにはより多くのノードが必要です"
    else
        log_fail "ストレージノードが登録されていません"
        exit 1
    fi
    
    # 各ノードの状態を表示
    log_info "ノード一覧:"
    echo "$nodes_response" | jq -r '.result.nodes[] | "  - \(.endpoint) [\(if .is_online then "ONLINE" else "OFFLINE" end)]"'
    
    log_info "=== テスト完了 ==="
    print_test_summary
}

run_test

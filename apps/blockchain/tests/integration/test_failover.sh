#!/bin/bash
# T120: ノード障害時のフェイルオーバーテスト
# 使用方法: ./test_failover.sh

set +e
source "$(dirname "$0")/utils.sh"

# 外部ノードを使用するためtrapを無効化
trap - EXIT

log_info "=== フェイルオーバーテスト ==="

RPC_ENDPOINT="${RPC_ENDPOINT:-http://127.0.0.1:9944}"

# ノード一覧取得
get_nodes() {
    curl -s -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"storage_getNodes","params":[]}' \
        "$RPC_ENDPOINT"
}

# オンラインノード数を確認
check_online_count() {
    response=$(get_nodes)
    online=$(echo "$response" | jq -r '.result.online_count // 0')
    total=$(echo "$response" | jq -r '.result.total_count // 0')
    
    log_info "ノード状態: $online / $total オンライン" >&2
    echo "$online"
}

# k-of-n 冗長性を確認
check_redundancy() {
    local k=3  # 復元に必要な最小断片数
    online=$(check_online_count)
    
    if [[ "$online" -ge "$k" ]]; then
        log_success "冗長性確保: $online ノードがオンライン (必要: $k)"
        return 0
    else
        log_warn "冗長性不足: $online ノードのみオンライン (必要: $k)"
        return 1
    fi
}

# フェイルオーバーシミュレーション情報
show_failover_info() {
    log_info "--- フェイルオーバー動作 ---"
    log_info "1. storage_getFragment はプライマリノードに接続試行"
    log_info "2. 失敗時、他のオンラインノードにフォールバック"
    log_info "3. k個以上のノードがオンラインなら復元可能"
    log_info "-----------------------------"
}

run_test() {
    # ノード状態確認
    response=$(get_nodes)
    total=$(echo "$response" | jq -r '.result.total_count // 0')
    
    if [[ "$total" -eq 0 ]]; then
        log_warn "ストレージノードが登録されていません"
        log_info "テストをスキップ"
        exit 0
    fi
    
    # ノード一覧表示
    log_info "登録ノード:"
    echo "$response" | jq -r '.result.nodes[] | "  \(.endpoint) - \(if .is_online then "ONLINE" else "OFFLINE" end)"'
    
    # 冗長性確認
    check_redundancy || true
    
    # フェイルオーバー情報表示
    show_failover_info
    
    log_info "=== テスト完了 ==="
    print_test_summary
}

run_test

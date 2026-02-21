#!/bin/bash
# 全統合テスト実行スクリプト
# 使用方法: ./run_all_tests.sh [--quick] [--scalability N]
#   --quick: スケーラビリティテストをスキップ
#   --scalability N: スケーラビリティテストのノード数（デフォルト: 10）

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"

# カラー出力
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# オプション解析
QUICK_MODE=false
SCALABILITY_NODES=10

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)
            QUICK_MODE=true
            shift
            ;;
        --scalability)
            SCALABILITY_NODES="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo ""
echo -e "${BLUE}╔═══════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   Anarchy Blockchain Integration Tests            ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════╝${NC}"
echo ""

# バイナリ確認
if [ ! -f "$PROJECT_DIR/target/release/anarchy-node" ]; then
    echo -e "${RED}Error: Node binary not found${NC}"
    echo "Build with: cd $PROJECT_DIR && cargo build --release"
    exit 1
fi

# jq確認
if ! command -v jq &> /dev/null; then
    echo -e "${RED}Error: jq is required but not installed${NC}"
    echo "Install with: sudo apt install jq"
    exit 1
fi

# 既存ノードを停止
echo -e "${YELLOW}Stopping any existing nodes...${NC}"
pkill -f "anarchy-node" 2>/dev/null || true
sleep 2

# テスト結果
declare -a TEST_RESULTS
declare -a TEST_NAMES
TOTAL_PASSED=0
TOTAL_FAILED=0

run_test() {
    local name=$1
    local script_with_args=$2
    
    # スクリプト名と引数を分離
    local script_name="${script_with_args%% *}"
    local script_args="${script_with_args#* }"
    if [ "$script_args" = "$script_with_args" ]; then
        script_args=""
    fi
    
    TEST_NAMES+=("$name")
    
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}  Running: $name${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    
    # テスト実行
    chmod +x "$SCRIPT_DIR/$script_name"
    
    if "$SCRIPT_DIR/$script_name" $script_args; then
        TEST_RESULTS+=("PASS")
        ((++TOTAL_PASSED)) || true
    else
        TEST_RESULTS+=("FAIL")
        ((++TOTAL_FAILED)) || true
    fi
    
    # クリーンアップ
    pkill -f "anarchy-node.*integration" 2>/dev/null || true
    sleep 3
}

# テスト実行
START_TIME=$(date +%s)

run_test "Block Synchronization" "test_block_sync.sh"
run_test "Consensus & Fork Resolution" "test_consensus.sh"
run_test "Invalid Data Rejection" "test_invalid_data.sh"
run_test "Node Recovery" "test_node_recovery.sh"

if [ "$QUICK_MODE" = false ]; then
    run_test "Scalability ($SCALABILITY_NODES nodes)" "test_scalability.sh $SCALABILITY_NODES"
else
    echo ""
    echo -e "${YELLOW}Skipping scalability test (--quick mode)${NC}"
fi

END_TIME=$(date +%s)
TOTAL_TIME=$((END_TIME - START_TIME))

# 最終結果
echo ""
echo -e "${BLUE}╔═══════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   Final Results                                   ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════╝${NC}"
echo ""

for i in "${!TEST_NAMES[@]}"; do
    if [ "${TEST_RESULTS[$i]}" = "PASS" ]; then
        echo -e "  ${GREEN}✓${NC} ${TEST_NAMES[$i]}"
    else
        echo -e "  ${RED}✗${NC} ${TEST_NAMES[$i]}"
    fi
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "  Total: $((TOTAL_PASSED + TOTAL_FAILED)) tests"
echo -e "  ${GREEN}Passed:${NC} $TOTAL_PASSED"
echo -e "  ${RED}Failed:${NC} $TOTAL_FAILED"
echo "  Time: ${TOTAL_TIME}s"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# 終了コード
if [ $TOTAL_FAILED -gt 0 ]; then
    echo ""
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
else
    echo ""
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi

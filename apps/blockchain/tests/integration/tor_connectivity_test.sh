#!/usr/bin/env bash
#
# Tor Connectivity Integration Test
#
# Tests the Tor integration functionality of Anarchy nodes.
#
# Prerequisites:
#   - Node binary built (cargo build --release or cargo build)
#   - Optional: Tor and torsocks installed for full tests
#
# Usage:
#   ./tests/integration/tor_connectivity_test.sh
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BLOCKCHAIN_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"

# Try release first, then debug
if [[ -x "${BLOCKCHAIN_DIR}/target/release/anarchy-node" ]]; then
    NODE_BINARY="${BLOCKCHAIN_DIR}/target/release/anarchy-node"
elif [[ -x "${BLOCKCHAIN_DIR}/target/debug/anarchy-node" ]]; then
    NODE_BINARY="${BLOCKCHAIN_DIR}/target/debug/anarchy-node"
else
    NODE_BINARY=""
fi

# Source common utilities if available
if [[ -f "${SCRIPT_DIR}/utils.sh" ]]; then
    source "${SCRIPT_DIR}/utils.sh"
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_test() {
    echo -e "${BLUE}[TEST]${NC} $1"
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1" >&2
}

log_skip() {
    echo -e "${YELLOW}[SKIP]${NC} $1"
}

# ============================================================
# Prerequisites Check
# ============================================================

check_prerequisites() {
    log_test "Checking prerequisites..."
    
    # Check node binary
    if [[ -z "$NODE_BINARY" ]]; then
        log_fail "Node binary not found"
        log_fail "Run: cargo build --release (or cargo build)"
        return 1
    fi
    
    log_pass "Node binary found: $NODE_BINARY"
    return 0
}

check_tor_available() {
    if command -v torsocks &> /dev/null; then
        return 0
    fi
    return 1
}

check_tor_running() {
    if pgrep -x tor > /dev/null 2>&1; then
        return 0
    fi
    return 1
}

# ============================================================
# Script Tests (no binary needed)
# ============================================================

test_wrapper_script_exists() {
    log_test "Testing anarchy-tor.sh wrapper script exists"
    
    local wrapper="${BLOCKCHAIN_DIR}/scripts/anarchy-tor.sh"
    
    if [[ ! -x "$wrapper" ]]; then
        log_fail "Wrapper script not found or not executable: $wrapper"
        return 1
    fi
    
    log_pass "anarchy-tor.sh exists and is executable"
    return 0
}

test_wrapper_sets_env_var() {
    log_test "Testing anarchy-tor.sh sets environment variable"
    
    local wrapper="${BLOCKCHAIN_DIR}/scripts/anarchy-tor.sh"
    
    if grep -q "ANARCHY_RUNNING_UNDER_TORSOCKS=1" "$wrapper"; then
        log_pass "Wrapper sets ANARCHY_RUNNING_UNDER_TORSOCKS=1"
        return 0
    else
        log_fail "Wrapper missing environment variable export"
        return 1
    fi
}

test_setup_script() {
    log_test "Testing tor-setup.sh script"
    
    local setup="${BLOCKCHAIN_DIR}/scripts/tor-setup.sh"
    
    if [[ ! -x "$setup" ]]; then
        log_fail "Setup script not found or not executable: $setup"
        return 1
    fi
    
    if "$setup" help > /dev/null 2>&1; then
        log_pass "tor-setup.sh help works"
        return 0
    else
        log_fail "tor-setup.sh help failed"
        return 1
    fi
}

test_onion_script() {
    log_test "Testing onion-service.sh script"
    
    local onion="${BLOCKCHAIN_DIR}/scripts/onion-service.sh"
    
    if [[ ! -x "$onion" ]]; then
        log_fail "Onion script not found or not executable: $onion"
        return 1
    fi
    
    if "$onion" help > /dev/null 2>&1; then
        log_pass "onion-service.sh help works"
        return 0
    else
        log_fail "onion-service.sh help failed"
        return 1
    fi
}

# ============================================================
# Source Code Tests
# ============================================================

test_tor_mode_enum_in_cli() {
    log_test "Testing TorMode enum exists in cli.rs"
    
    if grep -q "pub enum TorMode" "${BLOCKCHAIN_DIR}/node/src/cli.rs"; then
        log_pass "TorMode enum found in cli.rs"
        return 0
    else
        log_fail "TorMode enum not found in cli.rs"
        return 1
    fi
}

test_tor_mode_values() {
    log_test "Testing TorMode has correct values (Off, OutboundOnly, Forced)"
    
    local cli_file="${BLOCKCHAIN_DIR}/node/src/cli.rs"
    local found_all=true
    
    for mode in "Off" "OutboundOnly" "Forced"; do
        if ! grep -q "$mode" "$cli_file"; then
            log_fail "TorMode::$mode not found"
            found_all=false
        fi
    done
    
    if $found_all; then
        log_pass "All TorMode values found"
        return 0
    fi
    return 1
}

test_onion_validation_function() {
    log_test "Testing validate_onion_address function exists"
    
    if grep -q "fn validate_onion_address" "${BLOCKCHAIN_DIR}/node/src/command.rs"; then
        log_pass "validate_onion_address function exists"
        return 0
    else
        log_fail "validate_onion_address function not found"
        return 1
    fi
}

test_mainnet_enforcement() {
    log_test "Testing mainnet Tor enforcement code exists"
    
    if grep -q 'contains("mainnet")' "${BLOCKCHAIN_DIR}/node/src/command.rs"; then
        log_pass "Mainnet enforcement logic found"
        return 0
    else
        log_fail "Mainnet enforcement logic not found"
        return 1
    fi
}

test_unit_tests_exist() {
    log_test "Testing unit tests exist in command.rs"
    
    if grep -q "#\[cfg(test)\]" "${BLOCKCHAIN_DIR}/node/src/command.rs"; then
        log_pass "Unit tests module found"
        return 0
    else
        log_fail "Unit tests module not found"
        return 1
    fi
}

# ============================================================
# Binary Tests (requires built node)
# ============================================================

test_tor_mode_cli_help() {
    if [[ -z "$NODE_BINARY" ]]; then
        log_skip "No binary, skipping CLI help test"
        return 0
    fi
    
    log_test "Testing --tor-mode appears in --help output"
    
    if timeout 5 "$NODE_BINARY" --help 2>&1 | grep -q "tor-mode"; then
        log_pass "--tor-mode found in help output"
        return 0
    else
        log_fail "--tor-mode not found in help output"
        return 1
    fi
}

test_tor_mode_off_help() {
    if [[ -z "$NODE_BINARY" ]]; then
        log_skip "No binary, skipping tor-mode=off test"
        return 0
    fi
    
    log_test "Testing --tor-mode=off with --help (should work)"
    
    if timeout 5 "$NODE_BINARY" --tor-mode=off --help > /dev/null 2>&1; then
        log_pass "tor-mode=off accepted"
        return 0
    else
        log_fail "tor-mode=off rejected unexpectedly"
        return 1
    fi
}

test_forced_mode_without_torsocks() {
    if [[ -z "$NODE_BINARY" ]]; then
        log_skip "No binary, skipping forced mode test"
        return 0
    fi
    
    log_test "Testing --tor-mode=forced without torsocks (should fail)"
    
    # Ensure env var is not set
    unset ANARCHY_RUNNING_UNDER_TORSOCKS 2>/dev/null || true
    
    local output
    output=$(timeout 10 "$NODE_BINARY" --tor-mode=forced --chain=dev 2>&1 || true)
    
    if echo "$output" | grep -qi "torsocks"; then
        log_pass "forced mode correctly requires torsocks"
        return 0
    else
        log_fail "forced mode should require torsocks"
        echo "Output: $output"
        return 1
    fi
}

test_forced_mode_with_env_var() {
    if [[ -z "$NODE_BINARY" ]]; then
        log_skip "No binary, skipping env var test"
        return 0
    fi
    
    log_test "Testing --tor-mode=forced with ANARCHY_RUNNING_UNDER_TORSOCKS=1"
    
    # Set env var and run with --help (should not fail)
    export ANARCHY_RUNNING_UNDER_TORSOCKS=1
    
    if timeout 5 "$NODE_BINARY" --tor-mode=forced --help > /dev/null 2>&1; then
        log_pass "forced mode accepts torsocks env var"
        unset ANARCHY_RUNNING_UNDER_TORSOCKS
        return 0
    else
        log_fail "forced mode should work with env var set"
        unset ANARCHY_RUNNING_UNDER_TORSOCKS
        return 1
    fi
}

# ============================================================
# Tor Connectivity Tests (requires Tor installed and running)
# ============================================================

test_torsocks_connectivity() {
    if ! check_tor_available; then
        log_skip "torsocks not installed"
        return 0
    fi
    
    if ! check_tor_running; then
        log_skip "Tor daemon not running"
        return 0
    fi
    
    log_test "Testing torsocks connectivity via Tor Project API"
    
    if command -v curl &> /dev/null; then
        if timeout 30 torsocks curl -s https://check.torproject.org/api/ip 2>/dev/null | grep -q "IsTor"; then
            log_pass "torsocks working (verified via Tor Project)"
            return 0
        else
            log_fail "torsocks connectivity failed"
            return 1
        fi
    else
        log_skip "curl not available"
        return 0
    fi
}

# Get configured Onion address from Tor hidden service
get_onion_address() {
    local hostname_file="/var/lib/tor/anarchy-node/hostname"
    if [[ -r "$hostname_file" ]]; then
        cat "$hostname_file"
    elif sudo cat "$hostname_file" 2>/dev/null; then
        :
    else
        echo ""
    fi
}

test_onion_rpc_connectivity() {
    if ! check_tor_running; then
        log_skip "Tor daemon not running"
        return 0
    fi
    
    local onion_addr
    onion_addr=$(get_onion_address)
    
    if [[ -z "$onion_addr" ]]; then
        log_skip "No Onion address configured (run scripts/onion-service.sh setup)"
        return 0
    fi
    
    # Check if local node RPC is running
    if ! curl -s http://127.0.0.1:9944 -X POST -H "Content-Type: application/json" \
         -d '{"id":1,"jsonrpc":"2.0","method":"system_health"}' 2>/dev/null | grep -q "peers"; then
        log_skip "Local node RPC not available on port 9944"
        return 0
    fi
    
    log_test "Testing Onion RPC connectivity (HTTP via Tor SOCKS5)"
    
    local result
    result=$(timeout 60 curl -s --socks5-hostname 127.0.0.1:9050 \
        "http://${onion_addr}:9944" \
        -X POST -H "Content-Type: application/json" \
        -d '{"id":1,"jsonrpc":"2.0","method":"system_health"}' 2>/dev/null)
    
    if echo "$result" | grep -q '"peers"'; then
        log_pass "Onion RPC connectivity works: ${onion_addr}:9944"
        return 0
    else
        log_fail "Onion RPC connectivity failed"
        return 1
    fi
}

test_onion_rpc_chain_info() {
    if ! check_tor_running; then
        log_skip "Tor daemon not running"
        return 0
    fi
    
    local onion_addr
    onion_addr=$(get_onion_address)
    
    if [[ -z "$onion_addr" ]]; then
        log_skip "No Onion address configured"
        return 0
    fi
    
    # Check if local node RPC is running
    if ! curl -s http://127.0.0.1:9944 -X POST -H "Content-Type: application/json" \
         -d '{"id":1,"jsonrpc":"2.0","method":"system_health"}' 2>/dev/null | grep -q "peers"; then
        log_skip "Local node RPC not available"
        return 0
    fi
    
    log_test "Testing Onion RPC chain info retrieval"
    
    local result
    result=$(timeout 60 curl -s --socks5-hostname 127.0.0.1:9050 \
        "http://${onion_addr}:9944" \
        -X POST -H "Content-Type: application/json" \
        -d '{"id":1,"jsonrpc":"2.0","method":"system_chain"}' 2>/dev/null)
    
    if echo "$result" | grep -q '"result"'; then
        local chain_name
        chain_name=$(echo "$result" | grep -o '"result":"[^"]*"' | cut -d'"' -f4)
        log_pass "Chain info retrieved via Onion: $chain_name"
        return 0
    else
        log_fail "Failed to retrieve chain info via Onion"
        return 1
    fi
}

test_onion_rpc_transaction() {
    if ! check_tor_running; then
        log_skip "Tor daemon not running"
        return 0
    fi
    
    if ! check_tor_available; then
        log_skip "torsocks not available"
        return 0
    fi
    
    local onion_addr
    onion_addr=$(get_onion_address)
    
    if [[ -z "$onion_addr" ]]; then
        log_skip "No Onion address configured"
        return 0
    fi
    
    # Check if local node RPC is running
    if ! curl -s http://127.0.0.1:9944 -X POST -H "Content-Type: application/json" \
         -d '{"id":1,"jsonrpc":"2.0","method":"system_health"}' 2>/dev/null | grep -q "peers"; then
        log_skip "Local node RPC not available"
        return 0
    fi
    
    # Check if transfer script exists
    local transfer_script="${BLOCKCHAIN_DIR}/../../scripts/transfer-native.mjs"
    if [[ ! -f "$transfer_script" ]]; then
        log_skip "transfer-native.mjs script not found"
        return 0
    fi
    
    # Check if node is available
    if ! command -v node &> /dev/null; then
        log_skip "Node.js not available"
        return 0
    fi
    
    log_test "Testing transaction submission via Onion RPC"
    
    # Use Ferdie as test recipient (won't affect other tests)
    local recipient="Ferdie"
    local amount="1"  # 1 Unit
    
    # Run transfer via torsocks
    local root_dir="${BLOCKCHAIN_DIR}/../.."
    local result
    result=$(cd "$root_dir" && \
        WS_ENDPOINT="ws://${onion_addr}:9944" \
        timeout 120 torsocks node scripts/transfer-native.mjs "$recipient" "$amount" 2>&1)
    
    if echo "$result" | grep -q "送金成功"; then
        local block
        block=$(echo "$result" | grep -o "ブロック: #[0-9]*" | head -1)
        log_pass "Transaction via Onion RPC successful (${block})"
        return 0
    else
        log_fail "Transaction via Onion RPC failed"
        echo "$result" >&2
        return 1
    fi
}

# ============================================================
# Main Test Runner
# ============================================================

main() {
    echo "============================================="
    echo "  Anarchy Tor Integration Tests"
    echo "============================================="
    echo ""
    
    local passed=0
    local failed=0
    local skipped=0
    
    run_test() {
        if "$1"; then
            ((passed++))
        else
            local result=$?
            if [[ $result -eq 0 ]]; then
                ((skipped++))
            else
                ((failed++))
            fi
        fi
    }
    
    # Prerequisites
    echo -e "\n${BLUE}--- Prerequisites ---${NC}"
    run_test check_prerequisites
    
    # Script tests (always run)
    echo -e "\n${BLUE}--- Script Tests ---${NC}"
    run_test test_wrapper_script_exists
    run_test test_wrapper_sets_env_var
    run_test test_setup_script
    run_test test_onion_script
    
    # Source code tests
    echo -e "\n${BLUE}--- Source Code Tests ---${NC}"
    run_test test_tor_mode_enum_in_cli
    run_test test_tor_mode_values
    run_test test_onion_validation_function
    run_test test_mainnet_enforcement
    run_test test_unit_tests_exist
    
    # Binary tests
    echo -e "\n${BLUE}--- Binary Tests ---${NC}"
    run_test test_tor_mode_cli_help
    run_test test_tor_mode_off_help
    run_test test_forced_mode_without_torsocks
    run_test test_forced_mode_with_env_var
    
    # Connectivity tests
    echo -e "\n${BLUE}--- Connectivity Tests ---${NC}"
    run_test test_torsocks_connectivity
    
    # Onion RPC tests
    echo -e "\n${BLUE}--- Onion RPC Tests ---${NC}"
    run_test test_onion_rpc_connectivity
    run_test test_onion_rpc_chain_info
    run_test test_onion_rpc_transaction
    
    # Summary
    echo ""
    echo "============================================="
    echo -e "  Results: ${GREEN}${passed} passed${NC}, ${RED}${failed} failed${NC}, ${YELLOW}${skipped} skipped${NC}"
    echo "============================================="
    
    if [[ $failed -gt 0 ]]; then
        exit 1
    fi
    exit 0
}

main "$@"
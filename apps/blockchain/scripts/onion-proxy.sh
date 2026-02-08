#!/bin/bash
# =============================================================================
# onion-proxy.sh - SOCKS5 proxy for Onion Service connections
# =============================================================================
# 
# Creates a local TCP listener that forwards connections to an Onion address
# via Tor's SOCKS5 proxy. This allows libp2p (which doesn't natively support
# .onion addresses) to connect to Onion Services.
#
# Usage:
#   ./onion-proxy.sh <onion-address> [local-port] [remote-port]
#
# Example:
#   ./onion-proxy.sh zjnzfe3rv3yhwrxt6vwu6yeq3xi3kqxvepjfysaj2j7plysduuucvcqd.onion
#   # Creates proxy at 127.0.0.2:30333 -> onion:30333 via Tor
#
#   # Then connect with:
#   ./target/release/anarchy-node --bootnodes /ip4/127.0.0.2/tcp/30333/p2p/<peer-id>
#
# =============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }

# Configuration
ONION_ADDR="${1:-}"
LOCAL_PORT="${2:-30333}"
REMOTE_PORT="${3:-30333}"
LOCAL_IP="127.0.0.2"  # Use .2 to avoid conflict with nodes on 127.0.0.1
TOR_SOCKS_HOST="${TOR_SOCKS_HOST:-127.0.0.1}"
TOR_SOCKS_PORT="${TOR_SOCKS_PORT:-9050}"

show_help() {
    echo "Onion Proxy - Forward local connections to Onion Service via Tor"
    echo ""
    echo "Usage: $0 <onion-address> [local-port] [remote-port]"
    echo ""
    echo "Arguments:"
    echo "  onion-address   The .onion address to connect to (required)"
    echo "  local-port      Local port to listen on (default: 30333)"
    echo "  remote-port     Remote port on the Onion Service (default: 30333)"
    echo ""
    echo "Environment Variables:"
    echo "  TOR_SOCKS_HOST  Tor SOCKS5 proxy host (default: 127.0.0.1)"
    echo "  TOR_SOCKS_PORT  Tor SOCKS5 proxy port (default: 9050)"
    echo ""
    echo "Examples:"
    echo "  # Start proxy for an Onion bootnode"
    echo "  $0 zjnzfe3...vcqd.onion"
    echo ""
    echo "  # Then start your node with the proxy address"
    echo "  ./anarchy-node --bootnodes /ip4/${LOCAL_IP}/tcp/30333/p2p/<peer-id>"
    echo ""
    echo "Note: Requires socat to be installed (apt install socat)"
}

# Validate Onion address format
validate_onion() {
    local addr="$1"
    if [[ ! "$addr" =~ ^[a-z2-7]{56}\.onion$ ]]; then
        log_error "Invalid Onion address format: $addr"
        log_info "Expected format: <56 lowercase letters/digits>.onion"
        return 1
    fi
    return 0
}

# Check dependencies
check_deps() {
    if ! command -v socat &> /dev/null; then
        log_error "socat not found. Install with: sudo apt install socat"
        exit 1
    fi
    
    # Check if Tor SOCKS is available
    if ! nc -z "$TOR_SOCKS_HOST" "$TOR_SOCKS_PORT" 2>/dev/null; then
        log_error "Tor SOCKS proxy not available at ${TOR_SOCKS_HOST}:${TOR_SOCKS_PORT}"
        log_info "Make sure Tor is running: sudo systemctl status tor"
        exit 1
    fi
}

# Start the proxy
start_proxy() {
    local onion="$1"
    local local_port="$2"
    local remote_port="$3"
    
    # Remove .onion suffix for socat
    local onion_host="${onion%.onion}"
    
    log_info "Starting Onion proxy..."
    log_info "  Local:  ${LOCAL_IP}:${local_port}"
    log_info "  Remote: ${onion}:${remote_port}"
    log_info "  Via:    Tor SOCKS at ${TOR_SOCKS_HOST}:${TOR_SOCKS_PORT}"
    echo ""
    log_info "Use this bootnode address:"
    echo "  /ip4/${LOCAL_IP}/tcp/${local_port}/p2p/<peer-id>"
    echo ""
    log_info "Press Ctrl+C to stop the proxy"
    echo ""
    
    # socat command:
    # - TCP-LISTEN: Listen on local IP:port, fork for multiple connections
    # - SOCKS4A: Connect via SOCKS proxy (SOCKS4A supports .onion resolution)
    exec socat \
        TCP-LISTEN:${local_port},bind=${LOCAL_IP},reuseaddr,fork \
        SOCKS4A:${TOR_SOCKS_HOST}:${onion}:${remote_port},socksport=${TOR_SOCKS_PORT}
}

# Main
main() {
    # Handle help
    if [[ "$1" == "-h" || "$1" == "--help" || "$1" == "help" ]]; then
        show_help
        exit 0
    fi
    
    # Validate arguments
    if [[ -z "$ONION_ADDR" ]]; then
        log_error "Onion address required"
        echo ""
        show_help
        exit 1
    fi
    
    # Add .onion suffix if missing
    if [[ ! "$ONION_ADDR" =~ \.onion$ ]]; then
        ONION_ADDR="${ONION_ADDR}.onion"
    fi
    
    validate_onion "$ONION_ADDR" || exit 1
    check_deps
    start_proxy "$ONION_ADDR" "$LOCAL_PORT" "$REMOTE_PORT"
}

main "$@"

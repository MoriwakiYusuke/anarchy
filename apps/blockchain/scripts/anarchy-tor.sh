#!/bin/bash
# anarchy-tor.sh - Wrapper script to run Anarchy node under torsocks
# 
# This script sets the ANARCHY_RUNNING_UNDER_TORSOCKS environment variable
# which is required for --tor-mode=forced and --tor-mode=outbound-only modes.
#
# Usage:
#   ./scripts/anarchy-tor.sh ./target/release/anarchy-node --tor-mode=forced
#   ./scripts/anarchy-tor.sh ./target/release/anarchy-node --tor-mode=outbound-only

set -e

# Check if torsocks is installed
if ! command -v torsocks &> /dev/null; then
    echo "ERROR: torsocks is not installed"
    echo "Install with: ./scripts/tor-setup.sh install"
    exit 1
fi

# Check if Tor daemon is running
if ! pgrep -x "tor" > /dev/null; then
    echo "WARNING: Tor daemon does not appear to be running"
    echo "Start with: sudo systemctl start tor (Linux) or brew services start tor (macOS)"
fi

# Set the environment variable that indicates we're running under torsocks
export ANARCHY_RUNNING_UNDER_TORSOCKS=1

# Run the command under torsocks
exec torsocks "$@"

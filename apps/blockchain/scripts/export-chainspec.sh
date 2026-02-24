#!/bin/bash
# export-chainspec.sh - Export Anarchy chain spec for smoldot light client
#
# Usage:
#   ./export-chainspec.sh [--chain <chain>] [--output <path>]
#
# Options:
#   --chain   Chain type: dev (default), local, or custom file path
#   --output  Output path (default: ../frontend/src/lib/chainspec.json)
#
# Examples:
#   ./export-chainspec.sh                          # Export dev chain spec
#   ./export-chainspec.sh --chain local            # Export local chain spec
#   ./export-chainspec.sh --output /tmp/chain.json # Custom output path

set -e

SCRIPT_DIR=$(dirname "$0")
BLOCKCHAIN_DIR="$SCRIPT_DIR/.."
DEFAULT_OUTPUT="$BLOCKCHAIN_DIR/../frontend/src/lib/chainspec.json"

# Parse arguments
CHAIN="dev"
OUTPUT="$DEFAULT_OUTPUT"

while [[ $# -gt 0 ]]; do
  case $1 in
    --chain)
      CHAIN="$2"
      shift 2
      ;;
    --output)
      OUTPUT="$2"
      shift 2
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

# Resolve paths
BLOCKCHAIN_DIR=$(cd "$BLOCKCHAIN_DIR" && pwd)
NODE_BINARY="$BLOCKCHAIN_DIR/target/release/anarchy-node"

# Check if node binary exists
if [ ! -f "$NODE_BINARY" ]; then
  echo "Error: anarchy-node binary not found at $NODE_BINARY"
  echo "Please build the node first: cd apps/blockchain && cargo build --release"
  exit 1
fi

# Create output directory if needed
OUTPUT_DIR=$(dirname "$OUTPUT")
mkdir -p "$OUTPUT_DIR"

# Export chain spec with raw genesis
echo "Exporting chain spec for chain: $CHAIN"
echo "Output: $OUTPUT"

# Get local node peer ID for bootnode configuration
echo "Getting local node info..."

# Export the chain spec
"$NODE_BINARY" build-spec \
  --chain="$CHAIN" \
  --raw \
  --disable-default-bootnode > "$OUTPUT"

echo ""
echo "Chain spec exported successfully!"
echo ""
echo "IMPORTANT: The exported chain spec has no bootnodes configured."
echo "For smoldot to connect, you need to add bootnode addresses."
echo ""
echo "To get your local node's bootnode address:"
echo "  1. Start the node: ./target/release/anarchy-node --dev"
echo "  2. Note the 'Local node identity' line in the output"
echo "  3. Add it to the chainspec.json bootNodes array:"
echo "     \"bootNodes\": [\"/ip4/127.0.0.1/tcp/30333/p2p/<PEER_ID>\"]"
echo ""
echo "Or run a testnet and use those bootnode addresses:"
echo "  pnpm testnet:start"

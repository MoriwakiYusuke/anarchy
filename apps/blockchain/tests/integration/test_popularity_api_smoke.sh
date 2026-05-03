#!/bin/bash
#
# E2E smoke test: PopularityApi runtime API is wired correctly on a --dev node.
#
# This test verifies that the runtime exposes the Popularity_get_post_popularity
# RPC and returns the expected None result for an empty chain. It does NOT
# exercise the full create-post → react → decay → delete flow because the
# necessary helper scripts (create-post.mjs / react.mjs) don't exist yet —
# wiring those would require SSS+Merkle generation in JS (currently only the
# wasm-engine handles it from the frontend) and PoW computation. Behavior of
# the underlying logic is comprehensively covered by 22 unit tests in
# pallet-popularity.
#
# What this test confirms:
#   1. The dev node boots cleanly with the new pallet-popularity in
#      construct_runtime!
#   2. spec_version is >= 106 (new runtime API added)
#   3. PopularityApi::get_post_popularity is callable via state_call RPC and
#      returns the expected None for a non-existent post id
#
# Future work (for a follow-up task):
#   To extend this into a true lifecycle E2E, add the following helper scripts
#   under scripts/ (PAPI-based, using getWsProvider):
#     - create-post.mjs : SSS + Merkle fragment generation, post::create_post
#     - react.mjs       : PoW computation for reaction::react
#     - query-popularity.mjs : state_call wrapper for PopularityApi
#   Then this script can be expanded to: create post → react N times →
#   query effective score → wait blocks → query again (decay) → delete →
#   query None.
#
# Prerequisites: a running --dev node on http://127.0.0.1:9944 (or pass URL as $1).
#
# Usage: ./test_popularity_lifecycle.sh [http://127.0.0.1:9944]

set -euo pipefail
source "$(dirname "$0")/utils.sh"

# Default to HTTP-RPC port (substrate dev nodes serve both HTTP and WS on 9944).
RPC_URL="${1:-http://127.0.0.1:9944}"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

log_info "=== Popularity API smoke test ==="
log_info "RPC: $RPC_URL"

# --- Check prerequisites ---
if ! command -v curl >/dev/null 2>&1; then
  log_error "curl is required"
  exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
  log_error "jq is required"
  exit 1
fi

# Verify node reachability up front so we surface a clear error rather than
# letting subsequent jq parses fail on empty bodies.
if ! curl -sS -m 5 -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":0,"method":"system_health","params":[]}' \
    "$RPC_URL" >/dev/null 2>&1; then
  log_error "Blockchain node not reachable at $RPC_URL"
  exit 1
fi

# --- Step 1: Verify spec_version via state_getRuntimeVersion ---

log_info "Querying runtime version..."
VERSION_JSON=$(curl -sS -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"state_getRuntimeVersion","params":[]}' \
  "$RPC_URL")

SPEC_VERSION=$(echo "$VERSION_JSON" | jq -r '.result.specVersion')
SPEC_NAME=$(echo "$VERSION_JSON" | jq -r '.result.specName')

if [ "$SPEC_NAME" != "anarchy" ]; then
  log_error "Unexpected spec_name: $SPEC_NAME (expected 'anarchy')"
  exit 1
fi
if [ "$SPEC_VERSION" -lt 106 ]; then
  log_error "spec_version is $SPEC_VERSION but expected >= 106 (popularity pallet should bump it)"
  log_error "Hint: rebuild and restart the node (cargo build --release && ./target/release/anarchy-node --dev)"
  exit 1
fi
log_info "spec_version=$SPEC_VERSION OK"

# --- Step 2: Sanity check the runtime apis list ---
# Substrate exposes (api_hash, version) pairs via runtimeVersion.apis. We don't
# compute the blake2_64 hash of "PopularityApi" here (no portable hashing in
# pure shell); instead we sanity-check that the runtime exposes a reasonable
# number of APIs. The actual presence check is performed by the state_call
# below — if PopularityApi weren't registered, state_call would error out.
APIS_COUNT=$(echo "$VERSION_JSON" | jq -r '.result.apis | length')
log_info "Runtime exposes $APIS_COUNT runtime APIs"
if [ "$APIS_COUNT" -lt 8 ]; then
  log_warn "Runtime exposes fewer APIs than expected ($APIS_COUNT < 8)"
fi

# --- Step 3: Call Popularity_get_post_popularity(0) via state_call ---
# state_call expects: method name "Trait_method_name", and the SCALE-encoded
# parameter bytes as a hex string. For PopularityApi::get_post_popularity(post_id: u64),
# we encode `0u64` as little-endian bytes: 0x0000000000000000.

ENCODED_POST_ID="0x0000000000000000"
CALL_JSON=$(curl -sS -H "Content-Type: application/json" \
  -d "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"state_call\",\"params\":[\"PopularityApi_get_post_popularity\",\"$ENCODED_POST_ID\"]}" \
  "$RPC_URL")

CALL_ERROR=$(echo "$CALL_JSON" | jq -r '.error.message // empty')
if [ -n "$CALL_ERROR" ]; then
  log_error "state_call returned an error: $CALL_ERROR"
  log_error "Full response: $CALL_JSON"
  exit 1
fi

CALL_RESULT=$(echo "$CALL_JSON" | jq -r '.result')
log_info "PopularityApi_get_post_popularity(0) = $CALL_RESULT"

# An Option<PostPopularityRpc> with None is encoded as a single zero byte: 0x00
# (SCALE: Option None = 0x00, Option Some(x) = 0x01 || encoded x).
# An empty/None response should be exactly "0x00".

if [ "$CALL_RESULT" != "0x00" ]; then
  log_error "Expected None (0x00) for non-existent post, got: $CALL_RESULT"
  exit 1
fi
log_info "PopularityApi returns None for non-existent post id, as expected"

log_info "=== Popularity API smoke test PASSED ==="

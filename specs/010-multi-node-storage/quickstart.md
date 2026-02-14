# Quickstart: マルチノード対応とストレージセキュリティ

**Feature**: 010-multi-node-storage  
**Date**: 2026-02-14

## Prerequisites

- Rust 1.75+ with `wasm32v1-none` target
- Node.js 18+ with pnpm
- Docker (optional, for multi-node testing)

## Development Environment Setup

### 1. Build Blockchain Node

```bash
cd apps/blockchain
cargo build --release
```

### 2. Build Storage Node

```bash
cd apps/storage-node
cargo build --release
```

### 3. Build Frontend

```bash
pnpm install
pnpm build:wasm-engine  # Build WASM crypto engine first
pnpm build:frontend
```

## Running Tests

### Pallet Unit Tests

```bash
# Storage pallet tests (including new security tests)
cargo test -p pallet-storage -- --nocapture

# Post pallet tests (including tight coupling tests)
cargo test -p pallet-post -- --nocapture
```

### Storage Node Tests

```bash
cd apps/storage-node
cargo test -- --nocapture

# Specific test modules
cargo test auth::tests -- --nocapture
cargo test gossip::tests -- --nocapture
cargo test failover::tests -- --nocapture
```

### Integration Tests

```bash
# Start testnet (3 nodes)
pnpm testnet:start

# Run integration tests
pnpm test:integration

# Stop testnet
pnpm testnet:stop
```

### Frontend Tests

```bash
cd apps/frontend
pnpm test
```

## Local Development

### Start Dev Node

```bash
# Terminal 1: Blockchain node
pnpm dev:node

# Terminal 2: Storage node 1
cd apps/storage-node
./target/release/anarchy-storage-node --config config.example.toml

# Terminal 3: Storage node 2 (optional)
./target/release/anarchy-storage-node --config config-node2.toml

# Terminal 4: Frontend
pnpm dev:frontend
```

### Configuration Files

**Storage Node Config** (`config.example.toml`):
```toml
[node]
data_dir = "./data"
http_port = 3030
p2p_port = 4001

[chain]
ws_url = "ws://127.0.0.1:9944"
# Failover endpoints (learned via Gossipsub or manual)
backup_urls = ["ws://127.0.0.1:9945", "ws://127.0.0.1:9946"]

[auth]
enabled = true
signature_validity_secs = 300

[network]
bootstrap_peers = []
gossipsub_enabled = true

[metrics]
enabled = true
endpoint = "/metrics"
```

## Key Implementation Files

### Storage Pallet

| File | Purpose |
|------|---------|
| `pallets/storage/src/lib.rs` | Main pallet (extrinsics, storage) |
| `pallets/storage/src/pow.rs` | PoW verification module [NEW] |
| `pallets/storage/src/rate_limit.rs` | Rate limiting [NEW] |
| `pallets/storage/src/tests.rs` | Unit tests |

### Storage Node

| File | Purpose |
|------|---------|
| `storage-node/src/rpc/auth.rs` | Signature authentication [NEW] |
| `storage-node/src/network/gossip.rs` | Gossipsub protocol [NEW] |
| `storage-node/src/network/endpoint_cache.rs` | Endpoint cache [NEW] |
| `storage-node/src/network/reputation.rs` | Peer reputation [NEW] |
| `storage-node/src/chain/failover.rs` | Active-Standby failover [NEW] |
| `storage-node/src/metrics.rs` | Prometheus metrics [MODIFY] |

### Frontend

| File | Purpose |
|------|---------|
| `frontend/src/hooks/useStorage.ts` | Storage hook [MODIFY] |
| `frontend/src/components/FragmentStatus.tsx` | Visualization [NEW] |
| `frontend/src/stores/storageSettings.ts` | Settings [MODIFY] |

## Testing Scenarios

### Scenario 1: Multi-Node Fragment Distribution

```bash
# 1. Start 5 storage nodes
for i in 1 2 3 4 5; do
  ./target/release/anarchy-storage-node --config config-node${i}.toml &
done

# 2. Create a post via frontend
# 3. Verify fragments distributed across nodes
curl http://localhost:3030/fragments  # Node 1
curl http://localhost:3031/fragments  # Node 2
# ... etc

# 4. Stop 2 nodes and verify recovery
kill %2 %3

# 5. Access post - should recover from 3 remaining nodes
```

### Scenario 2: PoW Difficulty Adjustment

```bash
# 1. Register multiple nodes rapidly
for i in 1..10; do
  # Submit register_node extrinsic
  # Observe increasing PoW difficulty in logs
done

# 2. Wait 10 blocks
# 3. Register another node
# 4. Observe difficulty has decreased
```

### Scenario 3: Signature Authentication

```bash
# Without auth header - should fail
curl -X PUT http://localhost:3030/fragment/abc123 \
  -d "fragment data" \
  -H "Content-Type: application/octet-stream"
# Expected: 401 Unauthorized

# With valid auth header
curl -X PUT http://localhost:3030/fragment/abc123 \
  -d "fragment data" \
  -H "Content-Type: application/octet-stream" \
  -H "X-Anarchy-Auth: {...}"
# Expected: 201 Created
```

### Scenario 4: Blockchain Node Failover

```bash
# 1. Start storage node connected to blockchain node A
# 2. Kill blockchain node A
# 3. Observe logs: "Primary node failed, switching to standby"
# 4. Verify storage node continues operating
```

## API Examples

### Upload Fragment (with auth)

```typescript
import { blake2b } from '@noble/hashes/blake2b';
import { sr25519Sign } from '@polkadot/util-crypto';

async function uploadFragment(
  fragmentId: string,
  data: Uint8Array,
  keypair: KeyPair
): Promise<void> {
  const timestamp = Math.floor(Date.now() / 1000);
  const nonce = crypto.getRandomValues(new Uint8Array(16));
  const payloadHash = blake2b(data, { dkLen: 32 });
  
  // Message to sign: account_id(32) || timestamp(8) || nonce(16) || payloadHash(32) = 88 bytes
  // Uses schnorrkel signing_context(b"substrate") to match @polkadot/keyring
  const message = new Uint8Array(88);
  message.set(keypair.publicKey, 0);        // account_id (32 bytes)
  new DataView(message.buffer).setBigUint64(32, BigInt(timestamp), true);
  message.set(nonce, 40);                    // nonce (16 bytes)
  message.set(payloadHash, 56);              // payload_hash (32 bytes)
  
  const signature = sr25519Sign(message, keypair);
  
  const authHeader = JSON.stringify({
    account_id: `0x${Buffer.from(keypair.publicKey).toString('hex')}`,
    timestamp,
    nonce: `0x${Buffer.from(nonce).toString('hex')}`,
    payload_hash: `0x${Buffer.from(payloadHash).toString('hex')}`,
    signature: `0x${Buffer.from(signature).toString('hex')}`,
  });

  await fetch(`http://localhost:3030/fragment/${fragmentId}`, {
    method: 'PUT',
    body: data,
    headers: {
      'Content-Type': 'application/octet-stream',
      'X-Anarchy-Auth': authHeader,
    },
  });
}
```

### Query Fragment Placement

```typescript
import { createClient } from 'polkadot-api';
import { getWsProvider } from 'polkadot-api/ws-provider/node';

async function getFragmentHolders(fragmentId: string): Promise<string[]> {
  const client = createClient(getWsProvider('ws://127.0.0.1:9944'));
  const api = client.getUnsafeApi();
  
  const holders = await api.query.storage.fragmentHolders(
    hexToU8a(fragmentId)
  );
  
  return holders.map(h => u8aToHex(h));
}
```

## Debugging

### Enable Debug Logs

```bash
# Storage node
RUST_LOG=debug ./target/release/anarchy-storage-node --config config.toml

# Specific modules
RUST_LOG=anarchy_storage_node::network=trace,anarchy_storage_node::auth=debug
```

### Prometheus Metrics

```bash
# View metrics
curl http://localhost:3030/metrics

# Key metrics to monitor:
# - fragment_upload_total
# - blockchain_node_failover_total
# - peer_reputation_score{peer_id="..."}
# - storage_node_peers
```

### Common Issues

| Issue | Solution |
|-------|----------|
| PoW validation failing | Increase nonce calculation iterations |
| Signature expired | Check system clock sync |
| Gossipsub not receiving | Verify P2P port open, check bootstrap peers |
| Failover not working | Check backup_urls configured correctly |

## Next Steps

After completing implementation:

1. Run full test suite: `cargo test --all`
2. Run integration tests: `pnpm test:integration`
3. Update documentation in `docs/`
4. Create PR for review

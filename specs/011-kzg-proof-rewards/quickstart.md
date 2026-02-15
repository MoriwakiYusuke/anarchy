# Quickstart: KZG-VSS 保持証明・報酬システム

**Feature**: 011-kzg-proof-rewards  
**Date**: 2026-02-16

## Prerequisites

```bash
# Rust toolchain (stable2503)
rustup update stable
rustup target add wasm32-unknown-unknown

# wasm-pack
cargo install wasm-pack

# pnpm
npm install -g pnpm

# Node.js 18+
node --version  # v18.x or v20.x
```

---

## 1. Build Wasm Engine (KZG-VSS)

```bash
cd packages/wasm-engine

# Install dependencies (arkworks)
cargo fetch

# Build for browser
wasm-pack build --target web --out-dir pkg

# Run tests
cargo test
```

**Expected Output**:
```
   Compiling anarchy-wasm-engine v2.0.0
    Finished release [optimized]
[INFO]: Optimizing wasm binaries...
[INFO]: Writing package to pkg/
```

---

## 2. Build Blockchain (Storage Pallet)

```bash
cd apps/blockchain

# Build all pallets
cargo build --release

# Run pallet tests
cargo test -p pallet-storage

# Specific KZG tests
cargo test -p pallet-storage kzg
```

**Expected Test Output**:
```
running 8 tests
test tests::prove_holding_kzg_success ... ok
test tests::prove_holding_kzg_invalid_proof ... ok
test tests::claim_reward_success ... ok
test tests::claim_reward_no_pending ... ok
test tests::issue_challenge_success ... ok
test tests::register_fragment_success ... ok
test tests::reward_calculation_score_above_threshold ... ok
test tests::reward_calculation_score_below_threshold ... ok
```

---

## 3. Run Dev Node

```bash
# Terminal 1: Start blockchain node
cd apps/blockchain
./target/release/anarchy-node --dev

# Expected: Block production starts
# 🏷  Local node identity: 12D3KooW...
# 💻 Operating system: linux
# 📦 Highest known block at #0
```

---

## 4. Run Storage Node

```bash
# Terminal 2: Start storage node
cd apps/storage-node
cargo build --release
./target/release/anarchy-storage-node --config config.example.toml

# Expected: Connects to blockchain
# INFO Starting storage node...
# INFO Connected to chain at ws://127.0.0.1:9944
# INFO Registered as storage provider
```

---

## 5. Build Frontend

```bash
cd apps/frontend
pnpm install
pnpm dev

# Expected: Dev server starts
# ▲ Next.js 14.x
# - Local: http://localhost:3000
```

---

## 6. Test KZG-VSS in Browser

```javascript
// Open browser console at http://localhost:3000

import { vss_split, vss_recover, verify_kzg_proof } from 'anarchy-wasm-engine';

// Test data
const data = new TextEncoder().encode('Hello, KZG-VSS!');

// Split into 3-of-5 shares
const result = vss_split(data, 3, 5);
console.log('Commitment:', result.commitment);
console.log('Shares:', result.shares.length);  // 5

// Verify first share's proof
const valid = verify_kzg_proof(
  result.commitment,
  result.shares[0].index,
  result.shares[0].value,
  result.proofs[0]
);
console.log('Proof valid:', valid);  // true

// Recover from 3 shares
const recovered = vss_recover(result.shares.slice(0, 3), 3, result.compressed);
console.log('Recovered:', new TextDecoder().decode(recovered));
// "Hello, KZG-VSS!"
```

---

## 7. Test Proof Submission (PAPI)

```typescript
// scripts/test-proof.mjs
import { createClient } from 'polkadot-api';
import { getWsProvider } from 'polkadot-api/ws-provider/node';

const client = createClient(getWsProvider('ws://127.0.0.1:9944'));
const api = client.getUnsafeApi();

// Submit proof
const tx = api.tx.Storage.prove_holding_kzg({
  content_hash: '0x1234...', 
  share_index: 1,
  share_value: '0x...',
  proof: '0x...'
});

const result = await tx.signAndSend(alice);
console.log('Proof submitted:', result.hash);
```

---

## Directory Structure After Build

```
anarchy/
├── packages/wasm-engine/
│   ├── pkg/                    # Built Wasm package
│   │   ├── anarchy_wasm_engine.js
│   │   ├── anarchy_wasm_engine_bg.wasm
│   │   └── package.json
│   └── src/
│       └── kzg/                # NEW: KZG-VSS implementation
├── apps/blockchain/
│   ├── target/release/
│   │   └── anarchy-node        # Built node binary
│   └── pallets/storage/
│       └── src/
│           ├── kzg.rs          # NEW: KZG verification
│           └── rewards.rs      # NEW: Reward distribution
└── apps/storage-node/
    └── target/release/
        └── anarchy-storage-node
```

---

## Common Issues

### Wasm Build Fails with "unresolved import"

arkworks依存関係でstd featureが有効になっている可能性。

```toml
# packages/wasm-engine/Cargo.toml
[dependencies]
ark-bls12-381 = { version = "0.4", default-features = false }
ark-poly = { version = "0.4", default-features = false }
```

### KZG Verification Fails

SRSが正しくロードされていない可能性。

```typescript
import { init_srs } from 'anarchy-wasm-engine';

// Explicitly load SRS
const srsBytes = await fetch('/srs/mainnet.bin').then(r => r.arrayBuffer());
init_srs(new Uint8Array(srsBytes));
```

### Proof Submission "InvalidKzgProof"

コミットメントとProofの不一致。同じ多項式から生成されていることを確認。

```bash
# Debug: Check commitment matches fragment
cd apps/blockchain
cargo test -p pallet-storage test_kzg_commitment_match -- --nocapture
```

---

## Next Steps

1. `tasks.md` を生成: `/speckit.tasks`
2. 実装開始（TDD）: Wasm Engine → Storage Pallet → Storage Node → Frontend

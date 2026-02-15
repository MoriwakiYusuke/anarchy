# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Anarchy is an L1 blockchain-based decentralized SNS protocol built on Polkadot SDK (stable2503). It uses Substrate for the blockchain layer, Next.js for the frontend, and a separate Rust storage node daemon. The project is a pnpm monorepo.

**Language**: Documentation and comments are primarily in Japanese. Code is in Rust and TypeScript.

## Build & Development Commands

### Blockchain (Rust/Substrate)

```bash
# Build (from apps/blockchain/)
cargo build --release

# Run all pallet unit tests
cargo test --all

# Run a single pallet's tests
cargo test -p pallet-post
cargo test -p pallet-faucet
cargo test -p pallet-storage

# Lint
cargo clippy

# Start dev node (single, ephemeral)
./target/release/anarchy-node --dev

# From repo root via pnpm:
pnpm build:blockchain
pnpm dev:node
```

### Storage Node (Rust, separate workspace)

```bash
cd apps/storage-node
cargo build --release
cargo test

# Run with config
./target/release/anarchy-storage-node --config config.toml
```

### Wasm Crypto Engine

```bash
cd packages/wasm-engine
cargo install wasm-pack          # First time only
wasm-pack build --target web --out-dir pkg
```

The frontend depends on the Wasm engine (`"anarchy-wasm-engine": "file:../../packages/wasm-engine/pkg"`), so this must be built before `pnpm install`.

### Frontend (Next.js)

```bash
pnpm install                  # Install all workspace deps
pnpm dev:frontend             # Dev server at http://localhost:3000
pnpm build:frontend           # Production build
cd apps/frontend && pnpm test # Jest unit tests
cd apps/frontend && pnpm lint # ESLint
```

### Integration Tests (shell-based, requires running nodes)

```bash
pnpm test:integration         # All tests
pnpm test:sync                # Block sync
pnpm test:consensus           # Consensus/fork resolution
pnpm test:invalid             # Invalid data rejection
pnpm test:recovery            # Node crash recovery
pnpm test:scalability         # 10-node scalability
```

### Multi-Node Testnet

```bash
pnpm testnet:start            # Start 3-node testnet
pnpm testnet:stop             # Stop all nodes
pnpm testnet:status           # Check status
pnpm testnet:purge            # Purge chain data
```

## Architecture

### Monorepo Structure

- **apps/blockchain/** — Substrate L1 chain (Cargo workspace)
  - `node/` — Node binary (networking, RPC, consensus orchestration)
  - `runtime/` — FRAME runtime (pallet composition, genesis config)
  - `pallets/post/` — Post pallet (`create_post_v2`: records MerkleRoot on-chain, content stored off-chain)
  - `pallets/faucet/` — PoW faucet pallet (token claiming with client-side proof-of-work)
  - `pallets/storage/` — Distributed storage pallet (on-chain storage commitments)
  - `tests/integration/` — Shell-based integration tests
- **apps/storage-node/** — Off-chain distributed storage daemon (libp2p P2P + axum HTTP JSON-RPC on port 3030, separate Cargo project). Auto-registers with blockchain node on startup.
- **apps/frontend/** — Next.js 14 (App Router) + React 18 + TypeScript
- **packages/wasm-engine/** — Wasm crypto engine (SSS via `sharks`, MerkleTree via `rs_merkle`, Blake2b hashing). Built with `wasm-pack`, consumed by frontend as file dependency.
- **scripts/** — Token minting utilities (sudo-mint, transfer scripts using PAPI)
- **specs/** — Feature specifications (numbered: 001-identity, 002-webauthn, ..., 009-post-storage-migration)
- **docs/** — Architecture docs, Tor deployment guides

### Key Technical Constraints

**PAPI required, not @polkadot/api**: Polkadot SDK stable2503 uses metadata v16. The legacy `@polkadot/api` does NOT work (produces signature errors). Always use `polkadot-api` (PAPI) with `getUnsafeApi()` for chain interaction.

```typescript
import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'
const client = createClient(getWsProvider('ws://127.0.0.1:9944'))
const api = client.getUnsafeApi()
```

**Moral token precision**: 12 decimals (1 MORAL = 1_000_000_000_000 units). Post costs: base 10 MORAL + 0.1 MORAL/byte.

**Rust toolchain**: Stable channel with `wasm32v1-none` target and `rust-src` component (configured in `apps/blockchain/rust-toolchain.toml`).

### Security Principles (non-negotiable)

1. **Network anonymity**: Tor/I2P enforced at libp2p transport layer — no IP metadata leakage
2. **No raw private keys for users**: WebAuthn + Account Abstraction with Secure Enclave signing
3. **Client-side only crypto**: Encryption, SSS fragmentation, metadata stripping must happen client-side before transmission
4. **Foreground PoW only**: Reaction mining controlled via Page Visibility API

### Pallet Inter-dependencies

The Post pallet depends on `pallet_balances` (via tight coupling with `Config: pallet_balances::Config`) for burning MORAL tokens on post creation. Cost formula: `PostBaseCost + (content_bytes × PostByteCost)`.

### Spec-Driven Development

Feature specifications live in `specs/NNN-feature-name/` with a standard structure: `spec.md`, `plan.md`, `tasks.md`, `research.md`, `quickstart.md`, `contracts/`, `checklists/`. The `.github/agents/` and `.github/prompts/` directories contain SpecKit agents for automated spec workflows.

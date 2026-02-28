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
cargo test -p pallet-reaction

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
  - `pallets/storage/` — Distributed storage pallet (on-chain storage commitments, KZG proof verification, reward distribution)
  - `pallets/reaction/` — Reaction mining pallet (Like/Boost/Bad with PoW, dynamic difficulty, author rewards)
  - `tests/integration/` — Shell-based integration tests
- **apps/storage-node/** — Off-chain distributed storage daemon (libp2p P2P + axum HTTP JSON-RPC on port 3030, separate Cargo project). Auto-registers with blockchain node on startup.
- **apps/frontend/** — Next.js 14 (App Router) + React 18 + TypeScript
- **packages/wasm-engine/** — Wasm crypto engine (KZG-VSS hybrid via `ark-bls12-381`, Merkle tree via `rs_merkle`, Blake2b hashing). Built with `wasm-pack`, consumed by frontend as file dependency.
  - **KZG-VSS hybrid scheme**: Combines verifiable secret sharing with KZG polynomial commitments for efficient storage proofs
  - Key functions: `hybrid_split()`, `hybrid_reconstruct()`, `generate_kzg_proof()`, `verify_kzg_proof()`
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

### AI Agent Rules (non-negotiable)

The following rules must NEVER be violated. Violations completely destroy trustworthiness.

1. **No false task completion**: Before marking a task complete, you MUST actually write code, run tests, and verify functionality. Marking complete while saying "will do later" or "in next step" is strictly forbidden.
2. **No referencing non-existent files**: Before reporting file creation/editing, you MUST actually use tools to create/edit the file.
3. **No false test success reports**: When running tests, you MUST check actual output before reporting results.
4. **No claiming unimplemented features are done**: Before saying "implemented", you MUST verify code exists and compiles/builds successfully.
5. **No unchecked checklist updates**: Before marking tasks.md checkboxes as `[X]`, you MUST verify the task is 100% complete.
6. **No mock-only tests without real implementation**: Writing tests that pass using mocks/stubs while the actual implementation doesn't exist or doesn't work is strictly forbidden. Tests must validate real, working code.

**Violation examples (NEVER do these)**:
- Reporting "file created" without calling file creation tool
- Reporting "tests passed" without calling test execution tool
- Reporting "implementation complete" without writing code
- Reporting "success" when errors occurred
- Writing mock tests that pass without implementing the actual feature

### Security Principles (non-negotiable)

1. **Network anonymity**: Tor/I2P enforced at libp2p transport layer — no IP metadata leakage
2. **No raw private keys for users**: WebAuthn + Account Abstraction with Secure Enclave signing
3. **Client-side only crypto**: Encryption, SSS fragmentation, metadata stripping must happen client-side before transmission
4. **Foreground PoW only**: Reaction mining controlled via Page Visibility API

### Pallet Inter-dependencies

The Post pallet depends on `pallet_balances` (via tight coupling with `Config: pallet_balances::Config`) for burning MORAL tokens on post creation. Cost formula: `PostBaseCost + (content_bytes × PostByteCost)`.

### KZG Reward System (pallet-storage)

Storage nodes receive MORAL rewards for provable fragment holding. Key concepts:

- **KZG Commitment**: Polynomial commitment generated from post content shards
- **Proof Verification**: Storage nodes submit `prove_holding_kzg(fragment_id, kzg_proof)` to claim rewards
- **Reward Pool**: Post fees flow 90% to reward pool, 10% burned
- **Score System**: `ScoreProvider` trait for node reputation (default: score=1000, threshold=100)
- **GC Lifecycle**: Fragment lifecycle StateProposed → Active → ForgettingCandidate → deleted

### Reaction Mining (pallet-reaction)

Users react to posts (Like/Boost/Bad) with PoW proof, authors receive MORAL rewards:

- **PoW Mining**: Client-side Blake2b mining in Web Worker (`apps/frontend/src/workers/crypto.ts`)
- **Difficulty Adjustment**: Dynamic based on network reaction rate (adjusted every `AdjustmentWindow` blocks)
- **Reward Formula**: `reward = weight × cpu_power / 1_000_000` (capped by pool balance)
- **Foreground Enforcement**: Page Visibility API pauses mining when tab loses focus
- **Challenge Expiry**: PoW challenge valid for `ChallengeValidity` blocks (default: 100)
- **Stealth Recipients**: Optional stealth address for reward destination

### Spec-Driven Development

Feature specifications live in `specs/NNN-feature-name/` with a standard structure: `spec.md`, `plan.md`, `tasks.md`, `research.md`, `quickstart.md`, `contracts/`, `checklists/`. The `.github/agents/` and `.github/prompts/` directories contain SpecKit agents for automated spec workflows.

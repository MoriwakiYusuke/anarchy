# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Anarchy is an L1 blockchain-based decentralized SNS protocol built on Polkadot SDK (stable2503). It uses Substrate for the blockchain layer, Next.js for the frontend, and a separate Rust storage node daemon. The project is a pnpm monorepo.

**Language**: Documentation and comments are primarily in Japanese. Code is in Rust and TypeScript.

## Build & Development Commands

See [`.claude/skills/dev-command/SKILL.md`](.claude/skills/dev-command/SKILL.md) for all build, test, and dev commands.

## Architecture

### Monorepo Structure

- **apps/blockchain/** — Substrate L1 chain (Cargo workspace): `node/`, `runtime/`, `pallets/` (post / faucet / storage / reaction / stealth / nickname / messaging), `tests/integration/` (shell E2E)
- **apps/storage-node/** — Off-chain distributed storage daemon (libp2p P2P + axum HTTP JSON-RPC on `:3030`). Auto-registers with blockchain node on startup
- **apps/frontend/** — Next.js 14 App Router + React 18 + TypeScript. Uses PAPI via WebSocket (`getWsProvider`) to chain-node `:9944`
- **packages/wasm-engine/** — Wasm crypto engine (KZG-VSS hybrid via `ark-bls12-381`, Merkle via `rs_merkle`, Blake2b). Built with `wasm-pack`, consumed by frontend as file dependency
- **scripts/** — PAPI CLI scripts (sudo-mint, transfer, seed mint)
- **docs/** — Vision / architecture / economic / operations / security / development docs. Index: [docs/README.md](docs/README.md)
- **docs/superpowers/** — Superpowers plugin's fixed location for in-flight plans / specs. Do NOT move.
- **docs/archive/specs/** — Numbered feature specifications (001-identity … 019-direct-messages, 過去の設計資料として保持)

詳細な pallet 実装パターンは [`.claude/skills/backend-patterns/SKILL.md`](.claude/skills/backend-patterns/SKILL.md)、Wasm エンジン内部は [`.claude/skills/wasm-engine/SKILL.md`](.claude/skills/wasm-engine/SKILL.md)、フロント側は [`.claude/skills/frontend-patterns/SKILL.md`](.claude/skills/frontend-patterns/SKILL.md)、セキュリティチェックは [`.claude/skills/security-review/SKILL.md`](.claude/skills/security-review/SKILL.md) を参照。

### Key Technical Constraints

**PAPI required, not @polkadot/api**: Polkadot SDK stable2503 uses metadata v16. The legacy `@polkadot/api` does NOT work (produces signature errors). Always use `polkadot-api` (PAPI) with `getUnsafeApi()` for chain interaction.

- **Frontend**: WebSocket via `getWsProvider` ([apps/frontend/src/lib/chain-client.ts](apps/frontend/src/lib/chain-client.ts)) — `NEXT_PUBLIC_CHAIN_RPC_URL` で onion address を含む override 可。Phase B (PoW migration) で smoldot から切替: smoldot は consensus enum に PoW を持たず、また Anarchy の post / DM / storage は元々 chain-node RPC 拡張に依存しているため smoldot 経由できなかった
- **Node CLI scripts**: WebSocket via `getWsProvider` ([scripts/](scripts/))

**MORAL token precision**: 12 decimals (1 MORAL = 1_000_000_000_000 units). Post costs: `PostBaseCost + content_bytes × PostByteCost` (defaults: 10 MORAL + 0.1 MORAL/byte).

**Rust toolchain**: Stable channel with `wasm32v1-none` target and `rust-src` component (configured in [apps/blockchain/rust-toolchain.toml](apps/blockchain/rust-toolchain.toml)).

### Compatibility Policy

**Forward and backward compatibility are NOT a concern.** This project is in early development. When making breaking changes to storage formats, extrinsic signatures, RPCs, chain state, or DB schemas, **discard existing data and rebuild from scratch** — do not write migration code or compatibility shims.

- No Substrate runtime `StorageVersion` migrations, legacy field retention, or deprecated API preservation
- When chain state changes, regenerate the chainspec / dev chain rather than migrating
- Frontend IndexedDB / localStorage schemas may be wiped and recreated the same way
- Prefer code simplicity over "let old data still load" considerations

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
2. **Client-side key management**: Private keys are held in session memory only. Users authenticate via seed-phrase-derived AccountId (sr25519). Keys are never persisted to browser storage; cross-device access requires a user-exported, password-encrypted backup file.
3. **Client-side only crypto**: Encryption, SSS fragmentation, metadata stripping must happen client-side before transmission
4. **Foreground PoW only**: Reaction mining controlled via Page Visibility API
5. **No direct storage-node access from frontend**: The frontend MUST NOT connect to storage-node HTTP/JSON-RPC (`:3030`) or libp2p endpoints directly. All storage reads and writes go through chain-node RPC extensions (`storage_*` methods on the blockchain node WS endpoint), which fan out to storage-nodes internally. This keeps frontend → storage-node IP correlation out of the attack surface, centralizes session-auth on the chain node, and keeps storage-node addressing/selection a server-side concern. See [docs/architecture/storage.md §1](docs/architecture/storage.md) for the authoritative diagram.

詳細チェックリストは [`.claude/skills/security-review/SKILL.md`](.claude/skills/security-review/SKILL.md)。

### Feature Development Workflow

新規機能の開発は **Superpowers** スキル群を使う:

- `superpowers:brainstorming` — 要件・設計の探索（実装前に必ず）
- `superpowers:writing-plans` — multi-step タスクの実装計画作成
- `superpowers:executing-plans` / `superpowers:subagent-driven-development` — 計画の実行
- `superpowers:test-driven-development` — 実装前にテストを書く
- `superpowers:systematic-debugging` — バグ・テスト失敗時
- `superpowers:verification-before-completion` — "完了" 宣言前の検証
- `superpowers:requesting-code-review` / `superpowers:receiving-code-review` — レビュー
- `superpowers:finishing-a-development-branch` — マージ/PR 判断

`docs/archive/specs/NNN-feature-name/` 配下の既存仕様 (001..019) は過去の設計資料として参照可能だが、今後の新規機能で同様のディレクトリは作成しない。

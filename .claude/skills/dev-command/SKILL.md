---
name: dev-command
description: Anarchyモノレポのビルド・テスト・開発サーバ起動など、ブロックチェーン/ストレージノード/Wasmエンジン/フロントエンド/統合テスト/マルチノードテストネットの各種コマンドを参照するためのスキル。ユーザーが「ビルドコマンド」「テストコマンド」「dev server」「testnet起動」「DMテスト」などを尋ねたときに使用する。
---

# Dev Commands — Anarchy

Anarchy (Substrate L1 + Next.js + Rust storage node の pnpm monorepo) の開発コマンドリファレンス。すべてのコマンドはリポジトリ root から実行する想定。

## Blockchain (Rust / Substrate)

```bash
# Root から (推奨)
pnpm build:blockchain        # cargo build --release
pnpm dev:node                # cargo run -- --dev  (単一dev node)

# apps/blockchain/ から直接
cargo build --release
cargo test --all             # 全 pallet のユニットテスト
cargo test -p pallet-post    # 単一 pallet
cargo test -p pallet-stealth
cargo test -p pallet-messaging
cargo test -p pallet-storage
cargo test -p pallet-reaction
cargo test -p pallet-faucet
cargo test -p pallet-nickname
cargo clippy                 # lint
./target/release/anarchy-node --dev
```

Rust toolchain は `apps/blockchain/rust-toolchain.toml` で `stable + wasm32v1-none + rust-src` に固定。

## Storage Node (Rust, 独立 Cargo workspace)

```bash
# Root から (多ノード起動)
pnpm storage:start           # scripts/run-storage-nodes.sh start
pnpm storage:stop
pnpm storage:status
pnpm storage:purge

# apps/storage-node/ から直接
cargo build --release
cargo test
./target/release/anarchy-storage-node --config config.toml
```

Storage node は HTTP JSON-RPC を `:3030` で公開し、libp2p で P2P接続する。起動時にチェーンへ自動登録。

## Wasm Crypto Engine

```bash
cd packages/wasm-engine
cargo install wasm-pack                          # 初回のみ
wasm-pack build --target web --out-dir pkg
```

**重要**: フロントエンドは `"anarchy-wasm-engine": "file:../../packages/wasm-engine/pkg"` としてローカル依存を持つため、**`pnpm install` の前に `wasm-pack build` が完了している必要がある**。`pkg/` が無いと install が失敗する。

## Frontend (Next.js 14 App Router)

```bash
pnpm install                 # 全 workspace dep (事前に wasm-pack build 必須)
pnpm dev:frontend            # http://localhost:3000
pnpm build:frontend          # production build

# apps/frontend/ から
cd apps/frontend && pnpm test     # Jest ユニット/統合
cd apps/frontend && pnpm lint     # ESLint

# Playwright E2E (実ブラウザ。dev:node 起動必須。詳細は playwright-e2e skill)
pnpm test:e2e                # root から (= --filter @anarchy/frontend test:e2e)
cd apps/frontend && pnpm test:e2e:ui    # UI モード
```

## Integration Tests (shell, ノード起動必須)

```bash
pnpm test:integration                 # 全 shell 統合テスト
pnpm test:integration:quick           # 迅速版 (主要シナリオのみ)
pnpm test:sync                        # ブロック同期
pnpm test:consensus                   # コンセンサス / fork resolution
pnpm test:invalid                     # 不正データ拒否
pnpm test:recovery                    # ノードクラッシュリカバリ
pnpm test:scalability                 # 10-node スケーラビリティ
pnpm test:dm                          # DM (019): send-receive / stealth-linkage / multi-device
```

統合テストは `apps/blockchain/tests/integration/` 配下の shell スクリプト群。多くは dev node + storage node の同時起動を前提とする。

## Multi-Node Testnet

```bash
pnpm testnet:start           # 3-node testnet 起動
pnpm testnet:stop
pnpm testnet:status
pnpm testnet:logs
pnpm testnet:purge           # chain data 削除
```

## よくある落とし穴

- `@polkadot/api` は Polkadot SDK stable2503 (metadata v16) で動かない。**PAPI (`polkadot-api`) + `getUnsafeApi()` を使用**。
- `pnpm install` 失敗時はまず `packages/wasm-engine/pkg/` が存在するか確認。
- Rust ビルドが `wasm32v1-none` target 不在で失敗する場合は `rustup target add wasm32v1-none` ではなく `rust-toolchain.toml` を参照 (toolchain 自動セット)。
- Dev node 再起動時に古い chain data が原因で sync しない場合は `--tmp` フラグまたは `pnpm testnet:purge`。

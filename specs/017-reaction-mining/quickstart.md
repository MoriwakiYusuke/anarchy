# Quickstart: Reaction Mining

**Feature**: 017-reaction-mining  
**Date**: 2026-02-28

## 概要

このガイドでは、Reaction Mining機能の実装手順を説明する。

## 前提条件

- Rust toolchain (stable via `apps/blockchain/rust-toolchain.toml`)
- pnpm (workspace管理)
- 既存の開発環境が動作すること (`pnpm dev:node`, `pnpm dev:frontend`)

## Phase 1: Pallet Scaffolding

### 1.1 ディレクトリ作成

```bash
mkdir -p apps/blockchain/pallets/reaction/src
```

### 1.2 Cargo.toml作成

```bash
cat > apps/blockchain/pallets/reaction/Cargo.toml << 'EOF'
[package]
name = "pallet-reaction"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Reaction mining pallet with PoW-based rewards"

[dependencies]
parity-scale-codec = { workspace = true }
scale-info = { workspace = true }
frame-support = { workspace = true }
frame-system = { workspace = true }
sp-io = { workspace = true }
sp-runtime = { workspace = true }
sp-std = { workspace = true }
sp-api = { workspace = true }

# Pallet dependencies
pallet-post = { path = "../post" }

[dev-dependencies]
sp-core = { workspace = true }

[features]
default = ["std"]
std = [
    "parity-scale-codec/std",
    "scale-info/std",
    "frame-support/std",
    "frame-system/std",
    "sp-io/std",
    "sp-runtime/std",
    "sp-std/std",
    "sp-api/std",
    "pallet-post/std",
]
runtime-benchmarks = []
try-runtime = []
EOF
```

### 1.3 Workspaceに追加

```bash
# apps/blockchain/Cargo.toml の [workspace] members に追加
# "pallets/reaction",
```

## Phase 2: Core Implementation

### 2.1 基本構造 (`src/lib.rs`)

```bash
touch apps/blockchain/pallets/reaction/src/lib.rs
```

最小限の実装:
1. ReactionType enum
2. Reaction struct
3. Storage定義
4. react extrinsic（PoWなし）
5. 基本テスト

### 2.2 PoW検証追加

pallet-faucetから以下を流用:
- `compute_challenge()`
- `verify_proof()`
- `count_leading_zero_bits()`

### 2.3 Runtime統合

`apps/blockchain/runtime/src/lib.rs`:
```rust
// Add to construct_runtime!
Reaction: pallet_reaction,

// Configure pallet
impl pallet_reaction::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type NativeToken = Balances;
    type BaseDifficulty = ConstU8<16>;
    // ...
}
```

## Phase 3: Frontend Integration

### 3.1 WebWorkerマイニング追加

`apps/frontend/src/workers/crypto.ts`:
```typescript
case 'mine_reaction':
  // PoWマイニングロジック追加
```

### 3.2 フック作成

```bash
touch apps/frontend/src/hooks/useReactionMining.ts
```

主要機能:
- Page Visibility API監視
- マイニング進捗状態
- 一時停止/再開

### 3.3 UIコンポーネント

```bash
touch apps/frontend/src/components/ReactionButton.tsx
```

## ビルド & テスト

### パレットテスト

```bash
cd apps/blockchain
cargo test -p pallet-reaction
```

### フロントエンドテスト

```bash
cd apps/frontend
pnpm test
```

### 統合テスト

```bash
# ノード起動
pnpm dev:node

# フロントエンド起動
pnpm dev:frontend

# 手動確認:
# 1. 投稿を作成
# 2. いいねボタンをクリック
# 3. マイニング進捗を確認
# 4. 完了後、投稿者の残高増加を確認
```

## チェックリスト

- [ ] pallet-reaction スキャフォールド完了
- [ ] ReactionType, Reaction 型定義
- [ ] Storage定義（Reactions, ReactionStats, etc.）
- [ ] react extrinsic 実装
- [ ] PoW検証統合
- [ ] Runtime統合
- [ ] Genesis設定（初期報酬プール 10M MORAL）
- [ ] WebWorkerマイニング追加
- [ ] useReactionMining フック
- [ ] Page Visibility API統合
- [ ] ReactionButtonコンポーネント
- [ ] パレット単体テスト
- [ ] フロントエンドテスト
- [ ] 統合テスト

## 参考資料

- [spec.md](spec.md) - 機能仕様
- [research.md](research.md) - 技術調査
- [data-model.md](data-model.md) - データモデル
- [contracts/runtime-api.md](contracts/runtime-api.md) - Runtime API
- [contracts/interface.md](contracts/interface.md) - パレット間インターフェース
- `apps/blockchain/pallets/faucet/src/lib.rs` - PoW実装参考
- `apps/frontend/src/workers/crypto.ts` - WebWorker参考

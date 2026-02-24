# Quickstart: 013-slashing-repair 自己修復プロトコル

## Prerequisites

- Rust 1.75+ (stable channel)
- wasm-pack (for wasm-engine)
- pnpm 8+

## Build

### 1. wasm-engine (必須: regenerate_share関数追加後)

```bash
cd packages/wasm-engine
wasm-pack build --target web --out-dir pkg
```

### 2. Blockchain

```bash
cd apps/blockchain
cargo build --release
```

### 3. Storage Node

```bash
cd apps/storage-node
cargo build --release
```

## Test

### Unit Tests (pallet)

```bash
# pallet-storage全テスト
cargo test -p pallet-storage

# 特定テスト
cargo test -p pallet-storage test_slashing_after_failures
cargo test -p pallet-storage test_fragment_state_transitions
cargo test -p pallet-storage test_confirm_repair
```

### Unit Tests (storage-node)

```bash
cd apps/storage-node
cargo test

# 修復プロトコルのテスト
cargo test repair::
```

### Unit Tests (wasm-engine)

```bash
cd packages/wasm-engine
cargo test

# regenerate_share関数のテスト
cargo test kzg::vss::test_regenerate_share
```

### Integration Tests

```bash
# 修復プロトコル統合テスト（要: 3ノード起動）
pnpm test:repair

# 全統合テスト
pnpm test:integration
```

## Development Workflow

### 1. 機能追加時の標準手順

```bash
# 1. テスト環境起動
pnpm testnet:start

# 2. ログ監視（別ターミナル）
tail -f apps/blockchain/logs/*.log

# 3. storage-node起動（別ターミナル×3）
./target/release/anarchy-storage-node --config node1.toml
./target/release/anarchy-storage-node --config node2.toml
./target/release/anarchy-storage-node --config node3.toml
```

### 2. 実装順序（推奨）

1. **Phase 1: pallet-storage拡張**
   - `ProofRecord`にslashed, share_indexフィールド追加
   - `FragmentStates`ストレージ追加
   - Runtime API実装
   - `confirm_repair`, `evict_stale_holder` extrinsic

2. **Phase 2: wasm-engine**
   - `regenerate_share`関数実装

3. **Phase 3: storage-node**
   - 修復P2Pプロトコル（coordinator, donor, receiver）
   - 修復スケジューラ

4. **Phase 4: 統合テスト**
   - シェル統合テスト追加
   - 耐障害性テスト

### 3. デバッグ用コマンド

```bash
# Runtime API呼び出し（polkadot-js apps または curl）
curl -X POST http://localhost:9944 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"storageRepair_getAtRiskFragments","params":[]}'

# storage-nodeのRPC
curl http://localhost:3030 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"repair_status","params":[]}'
```

## Key Files

| Component | Path | Description |
|-----------|------|-------------|
| Pallet | `apps/blockchain/pallets/storage/src/lib.rs` | Storage pallet本体 |
| Runtime API | `apps/blockchain/pallets/storage/src/runtime_api.rs` | Runtime API定義 |
| VSS | `packages/wasm-engine/src/kzg/vss.rs` | VSS + regenerate_share |
| Repair Coordinator | `apps/storage-node/src/repair/coordinator.rs` | 修復コーディネータ |
| P2P Protocol | `apps/storage-node/src/repair/protocol.rs` | libp2p request-response |

## Configuration

### storage-node config.toml に追加

```toml
[repair]
# 修復スケジューラの実行間隔（秒）
check_interval_secs = 30

# 修復タイムアウト（秒）
repair_timeout_secs = 3600

# 同時修復可能な断片数
max_concurrent_repairs = 5

# k-of-n パラメータ
threshold_k = 3
total_shares_n = 5
```

### runtime config (genesis)

```rust
pallet_storage: StorageConfig {
    // スラッシュ率（50% = 5000 bp）
    slash_rate_basis_points: 5000,
    
    // 修復報酬プール割当（投稿手数料の90%）
    repair_pool_share_basis_points: 9000,
    
    // 引き出し下限（500 MORAL）
    min_withdrawal_amount: 500_000_000_000_000,
    
    // チャレンジ失敗許容回数
    max_failures_before_slash: 3,
}
```

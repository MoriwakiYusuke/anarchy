# Implementation Plan: 自己修復プロトコル

**Branch**: `013-slashing-repair` | **Date**: 2026-02-24 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/013-slashing-repair/spec.md`

## Summary

ストレージノードがオフラインになった際に、コンテンツ断片を自動的に再配布し、k-of-n閾値（3-of-5）を維持する自己修復プロトコル。報酬の積み立て方式とスラッシング（ペナルティ）メカニズムを導入し、ノードオペレーターに正常稼働のインセンティブを提供する。

**技術アプローチ**:
1. pallet-storage拡張: FragmentState、AccruedRewards、RepairRewardPool ストレージ追加
2. storage-node拡張: HealthMonitor、ShareRegenerator、RepairExecutor、StaleHolderGC モジュール追加
3. wasm-engine拡張: `regenerate_share` 関数追加（Lagrange補間で新シェア生成）

## Technical Context

**Language/Version**: Rust 1.75+ (stable channel with wasm32v1-none target)
**Primary Dependencies**: 
- Polkadot SDK stable2503 (FRAME pallets)
- libp2p (P2P networking)
- ark-bls12-381 (KZG proofs)
- wasm-pack (Wasm engine build)

**Storage**: 
- On-chain: FRAME StorageMap/StorageDoubleMap (FragmentState, AccruedRewards, ProofRecords拡張)
- Off-chain: ファイルシステム + RocksDB (storage-node)

**Testing**: 
- `cargo test -p pallet-storage` (パレット単体テスト)
- `cargo test` in apps/storage-node (ストレージノードテスト)
- `tests/integration/` (シェルベース統合テスト)

**Target Platform**: Linux server (ブロックチェーンノード + ストレージノード)
**Project Type**: Monorepo (blockchain + storage-node + wasm-engine)

**Performance Goals**:
- 修復完了: 60分以内（Repairing状態タイムアウト）
- GC正常化: 60分以内
- スラッシング実行: 1ブロック（6秒）以内

**Constraints**:
- 断片数: 10万〜100万+個（中〜大規模）
- ノード数: 50〜200+個

**Scale/Scope**:
- pallet-storage: 新規ストレージ3個、エクストリンシック3個、RPC追加2個
- storage-node: 新規モジュール5個
- wasm-engine: 新規関数1個

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Assessment |
|-----------|--------|------------|
| I. Network Anonymity | ✅ PASS | 本機能はオンチェーン/オフチェーンのストレージ管理のみ。既存のlibp2p + Tor/I2P統合に影響なし |
| II. Keyless UX | ✅ PASS | ストレージノードオペレーター向け機能。ユーザーの秘密鍵操作は発生しない |
| III. Client-Side Completion | ✅ PASS | 断片の再生成はストレージノード間で完結。エンドユーザーのクライアントには影響なし |
| IV. Zero-Trust Hydra | ✅ PASS | フロントエンド信頼モデルに変更なし |
| V. Economic Autonomy | ✅ PASS | スラッシングと報酬分配により、正直なノードオペレーターに最大のインセンティブを提供 |
| VI. Test-First Development | ✅ REQUIRED | パレット単体テスト + 統合テストを先に作成 |

**結果**: 全ゲート通過。Phase 0 に進む。

## Project Structure

### Documentation (this feature)

```text
specs/013-slashing-repair/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (Runtime API schemas)
└── tasks.md             # Phase 2 output
```

### Source Code (repository root)

```text
apps/blockchain/pallets/storage/src/
├── lib.rs               # 拡張: FragmentState, AccruedRewards, RepairRewardPool ストレージ
├── rewards.rs           # 拡張: claim_rewards, slash_node ロジック
├── repair.rs            # 新規: confirm_repair, evict_stale_holder ロジック
└── tests.rs             # 拡張: 修復・スラッシングテスト追加

apps/blockchain/runtime/src/
└── lib.rs               # Runtime API 追加

apps/storage-node/src/
├── health_monitor.rs    # 新規: FragmentState監視 + AtRisk検出
├── share_regenerator.rs # 新規: k個収集 → Lagrange → 新シェア生成
├── repair_executor.rs   # 新規: ノード選定 → Push配送
├── repair_reporter.rs   # 新規: confirm_repair提出
├── stale_holder_gc.rs   # 新規: 復帰ノード検出 + 自発的退出 + シェア削除
└── gc.rs                # 既存: スコアベースGC（変更なし、並列動作）

packages/wasm-engine/src/
├── kzg/
│   └── vss.rs           # 拡張: regenerate_share 関数追加
└── lib.rs               # 拡張: regenerate_share エクスポート

apps/blockchain/tests/integration/
└── repair_protocol_test.sh  # 新規: 修復プロトコル統合テスト
```

**Structure Decision**: Monorepo構造を維持。pallet-storage、storage-node、wasm-engineの3コンポーネントを拡張。

## Complexity Tracking

> Constitution Check に違反がないため、このセクションは不要。

N/A

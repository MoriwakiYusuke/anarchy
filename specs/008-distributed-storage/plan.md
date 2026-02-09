# Implementation Plan: Storage MVP - Phase 1

**Branch**: `008-distributed-storage` | **Date**: 2026-02-09 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/008-distributed-storage/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

分散ストレージシステムの最小実装（MVP）。データの保存場所（PeerID）をチェーンに登録し、libp2pで断片を送受信できる状態を目指す。報酬・罰則なしのPhase 1。

## Technical Context

**Language/Version**: Rust 1.75+ (Polkadot SDK stable2503), TypeScript 5.x (Frontend)
**Primary Dependencies**: 
- Pallet: `frame-support`, `frame-system`, `sp-runtime`, `sp-core`
- Daemon: `libp2p` (rust-libp2p), `subxt`, `tokio`, `sled`/`rocksdb`
**Storage**: 
- Pallet: Substrate StorageMap/StorageDoubleMap
- Daemon: ローカルファイルシステム + KVS (sled)
**Testing**: `cargo test` (Rust), E2Eスクリプト
**Target Platform**: Linux server (Daemon), WASM (Frontend SDK - 将来)
**Project Type**: Monorepo (`apps/blockchain/pallets/storage/`, `apps/storage-node/`)
**Performance Goals**: 断片転送 1MB/秒以上、PoS登録 6秒以内（1ブロック）
**Constraints**: デーモンメモリ 200MB未満（10GB断片保持時）
**Scale/Scope**: Phase 1はMVP、性善説（検証なし）

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### I. Network Anonymity（ネットワーク秘匿）

| チェック項目 | 状態 | 備考 |
|------------|------|------|
| libp2p通信はTor対応可能か | ✅ PASS | Phase 1はTorなしで開発、Phase 2+で006-libp2p-torと統合予定 |
| PeerIDはIPアドレスを漏らさないか | ✅ PASS | PeerIDはEd25519公開鍵由来、IPと独立 |
| チェーン上にIP情報は保存されないか | ✅ PASS | PeerID、Fragment ID、AccountIdのみ |

### II. Keyless UX（秘密鍵の排除）

| チェック項目 | 状態 | 備考 |
|------------|------|------|
| ユーザーに秘密鍵を扱わせないか | ⚠️ 部分PASS | ストレージノード運営者はキーペア必要（Phase 1許容） |

**Note**: ストレージノードは「運営者」向けでエンドユーザー向けではない。エンドユーザー（投稿者）はフロントエンドのみ使用し、秘密鍵を見ない。

### III. Client-Side Completion（クライアントサイド完結）

| チェック項目 | 状態 | 備考 |
|------------|------|------|
| 暗号化はクライアントで行うか | ✅ PASS | Phase 1はデータをそのまま保存（SSS/暗号化はPhase 2のWasm暗号エンジン） |
| 断片化はクライアントで行うか | ✅ PASS | Fragment IDはクライアントが生成 |

### IV. Zero-Trust Hydra（ゼロトラスト・フロントエンド）

| チェック項目 | 状態 | 備考 |
|------------|------|------|
| 悪意あるフロントエンドを想定しているか | ✅ PASS | チェーン登録はエクストリンシック署名で保護 |

### V. Economic Autonomy（経済的自律性）

| チェック項目 | 状態 | 備考 |
|------------|------|------|
| 報酬設計があるか | ⏸️ SKIP | Phase 1ではスコープ外、Phase 2で実装 |

### VI. Test-First Development

| チェック項目 | 状態 | 備考 |
|------------|------|------|
| テスト計画があるか | ✅ PASS | spec.mdにT-001〜T-203のテスト要件定義済み |

**Constitution Check Result**: ✅ PASS（Phase 1スコープで許容可能）

## Project Structure

### Documentation (this feature)

```text
specs/008-distributed-storage/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── storage-pallet.md
└── tasks.md             # Phase 2 output
```

### Source Code (repository root)

```text
apps/blockchain/
├── pallets/
│   └── storage/         # 新規: Storage Pallet
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
└── runtime/
    └── src/
        └── lib.rs       # 編集: pallet-storage統合

apps/storage-node/       # 新規: ストレージノードデーモン
├── Cargo.toml
└── src/
    ├── main.rs          # エントリポイント
    ├── config.rs        # 設定ファイル読み込み
    ├── storage/         # ローカルストレージ管理
    │   └── mod.rs
    ├── network/         # libp2pネットワーク
    │   └── mod.rs
    └── chain/           # チェーン連携 (subxt)
        └── mod.rs
```

**Structure Decision**: 既存のmonorepo構造に従い、`apps/blockchain/pallets/storage/`にパレット、`apps/storage-node/`にデーモンを配置。pallet-faucetと同じパターンを踏襲。

## Complexity Tracking

> 特になし - Constitution Checkに重大な違反なし

---

## Phase 0 Output: Research

**Status**: ✅ Complete  
**File**: [research.md](research.md)

### Key Decisions

| Topic | Decision | Rationale |
|-------|----------|-----------|
| P2Pプロトコル | libp2p request-response | 標準的、ポイント・ツー・ポイント転送に最適 |
| チェーンクライアント | subxt (Rust) | 型安全、公式Substrateクライアント |
| 断片ストレージ | 直接ファイルI/O + 階層ディレクトリ | 1MBサイズには単純で十分 |
| PeerID形式 | Ed25519 (12D3KooW...) | libp2p標準 |

---

## Phase 1 Output: Design Artifacts

### data-model.md
**Status**: ✅ Complete  
**File**: [data-model.md](data-model.md)

**Entities Defined**:
- `FragmentMetadata` - オンチェーン断片メタデータ
- `StorageNodeInfo` - オンチェーンノード情報
- `HoldingDeclaration` - 保持表明
- `LocalFragmentMeta` - オフチェーン断片メタデータ

### contracts/
**Status**: ✅ Complete  
**File**: [contracts/storage-pallet.md](contracts/storage-pallet.md)

**Extrinsics Defined**:
- `register_fragment` - 断片登録
- `register_node` - ノード登録
- `update_node` - ノード更新
- `unregister_node` - ノード登録解除
- `declare_holding` - 保持表明
- `revoke_holding` - 保持取消

### quickstart.md
**Status**: ✅ Complete  
**File**: [quickstart.md](quickstart.md)

**Contents**:
- パレットの最小実装テンプレート
- デーモンの最小実装テンプレート
- ローカルテスト手順

---

## Next Step: Phase 2 (tasks.md)

`/speckit.tasks`コマンドで実装タスクを生成:

```
/speckit.tasks
```

生成されるタスク構成（予定）:
1. pallet-storage基本実装
2. register_fragment / register_node
3. declare_holding / revoke_holding
4. Runtime統合
5. storage-node daemon スケルトン
6. libp2p swarm実装
7. fragment store実装
8. subxtチェーン連携
9. E2Eテスト

---

## Appendix: File Summary

| File | Purpose | Status |
|------|---------|--------|
| [spec.md](spec.md) | Feature specification | ✅ Complete |
| [plan.md](plan.md) | This implementation plan | ✅ Complete |
| [research.md](research.md) | Technical research | ✅ Complete |
| [data-model.md](data-model.md) | Data model definitions | ✅ Complete |
| [quickstart.md](quickstart.md) | Quick start guide | ✅ Complete |
| [contracts/storage-pallet.md](contracts/storage-pallet.md) | Pallet API contract | ✅ Complete |
| [tasks.md](tasks.md) | Implementation tasks | ⏳ Pending (`/speckit.tasks`) |
| [checklists/requirements.md](checklists/requirements.md) | Quality checklist | ✅ Complete |

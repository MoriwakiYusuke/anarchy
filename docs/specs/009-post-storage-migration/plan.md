# Implementation Plan: Post Storage Migration（オンチェーン・ダイエット）

**Branch**: `009-post-storage-migration` | **Date**: 2026-02-10 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/009-post-storage-migration/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

投稿コンテンツをブロックチェーンからオフチェーン分散ストレージへ移行する。フロントエンドでSSS（シャミアの秘密分散）とMerkleTree構築を行い、Blockchain NodeがMerkleProof検証後にStorage Nodeへ転送。チェーンにはMerkleRootとメタデータのみを記録し、ストレージコスト削減と大容量コンテンツ対応を実現。

**Clarification結果の反映**:
- k/n値: 固定（k=3, n=5）、システム設定のみ変更可
- 断片最大サイズ: 256KB
- リトライ: 3回後に別ノードへフォールバック
- コスト構成: 基本料金50% : サイズ係数30% : Storage報酬デポジット20%

## Technical Context

**Language/Version**: Rust 1.87 (stable2503), TypeScript 5.x (Next.js 15)  
**Primary Dependencies**: Polkadot SDK (stable2503), PAPI (polkadot-api), libp2p 0.54, wasm-bindgen, subxt, sharks, rs_merkle  
**Storage**: オンチェーン（MerkleRootのみ）、オフチェーン（Storage Node分散保存、断片最大256KB）  
**Testing**: `cargo test` (Rust), Jest (TypeScript)  
**Target Platform**: Browser (Wasm), Linux (Substrate Node / Storage Node)
**Project Type**: web (frontend + blockchain + storage-node)  
**Performance Goals**: 投稿作成から表示まで5秒以内、キャッシュヒット時1秒以内  
**Constraints**: k個以上のStorage Nodeオンラインで100%可用性、3回リトライ後フォールバック  
**Scale/Scope**: 1MB以上のコンテンツ対応、k=3/n=5固定

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原則 | 状態 | 評価 |
|-----|------|------|
| **I. Network Anonymity** | ✅ PASS | Blockchain Node ↔ Storage Nodeはlibp2p経由。Tor/I2P統合は別Phase（006-libp2p-tor）で対応済み |
| **II. Keyless UX** | ✅ PASS | 本機能は認証に影響しない。WebAuthn署名は維持 |
| **III. Client-Side Completion** | ✅ PASS | SSS分割・MerkleTree構築は**フロントエンドWasm**で完結。サーバーに平文を送らない |
| **IV. Zero-Trust Hydra** | ✅ PASS | Blockchain NodeがMerkleProof検証を実施。悪意あるフロントからの不正断片は数学的に拒否 |
| **V. Economic Autonomy** | ✅ PASS | 投稿コストの20%をStorage報酬デポジットに。declare_holding手数料を投稿者負担で賄う |
| **VI. Test-First Development** | ✅ PASS | 各コンポーネントにテスト要件あり（FR対応テスト） |

**Gate結果**: 全項目PASS。Phase 0 研究に進む。

### Post-Design Re-evaluation (Phase 1完了後)

| 原則 | 状態 | Phase 1設計での確認 |
|-----|------|---------------------|
| **I. Network Anonymity** | ✅ PASS | libp2p `/anarchy/fragment/1.0.0` プロトコル使用（research.md §4） |
| **II. Keyless UX** | ✅ PASS | 影響なし |
| **III. Client-Side Completion** | ✅ PASS | `packages/wasm-engine` でSSS + Merkle実装（data-model.md §3） |
| **IV. Zero-Trust Hydra** | ✅ PASS | `storage_uploadFragment` RPCでMerkleProof検証（contracts/storage-rpc.json） |
| **V. Economic Autonomy** | ✅ PASS | コスト構成 50:30:20 でデポジット確保 |
| **VI. Test-First Development** | ✅ PASS | quickstart.mdに検証手順記載 |

**最終Gate結果**: 全項目PASS。実装フェーズに進行可。

## Project Structure

### Documentation (this feature)

```text
specs/009-post-storage-migration/
├── plan.md              # This file
├── research.md          # Phase 0 output ✓
├── data-model.md        # Phase 1 output ✓
├── quickstart.md        # Phase 1 output ✓
├── contracts/           # Phase 1 output ✓
│   └── storage-rpc.json # カスタムRPC定義
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
apps/
├── blockchain/
│   ├── node/
│   │   └── src/
│   │       └── rpc/           # [NEW] カスタムRPC実装
│   │           ├── mod.rs
│   │           └── storage.rs # storage_uploadFragment, storage_getFragment等
│   ├── pallets/
│   │   └── post/
│   │       └── src/
│   │           └── lib.rs     # [MODIFY] Contents削除、PostContent構造体追加
│   └── runtime/
│       └── src/
│           └── lib.rs         # [MODIFY] カスタムRPC登録
├── storage-node/
│   └── src/
│       ├── lib.rs             # [MODIFY] libp2p request-response拡張
│       └── protocol/          # [NEW] Blockchain Node通信プロトコル
└── frontend/
    └── src/
        ├── hooks/
        │   └── useStorage.ts  # [NEW] storage_* RPC呼び出し
        └── workers/
            └── crypto.ts      # [NEW] Wasm Worker (SSS + Merkle)

packages/
└── wasm-engine/               # [NEW] Rust → Wasm暗号エンジン
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── sss.rs             # シャミア秘密分散
        └── merkle.rs          # MerkleTree構築・検証
```

**Structure Decision**: 既存のapps/構造を維持しつつ、packages/wasm-engineを新規追加。Blockchain NodeにカスタムRPCレイヤーを追加し、フロントエンドはPAPI経由で統一アクセス。

## Complexity Tracking

> 全Constitution Check項目がPASSのため、違反の正当化は不要。

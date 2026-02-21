# Implementation Plan: Critical Bug Fixes (HIGH Priority 13 Issues)

**Branch**: `012-critical-bug-fixes` | **Date**: 2026-02-21 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/012-critical-bug-fixes/spec.md`

## Summary

13件のHIGH優先度issue（セキュリティ脆弱性・重大バグ）を修正する。対象は5つのコンポーネント：
- **pallet-storage**: チャレンジスパム防止、期限切れ処理、報酬二重計上、不正fragment登録防止
- **node gossip**: DoS対策（接続数上限、レジストリサイズ上限）
- **wasm-engine**: RNG失敗時のエラーハンドリング、VSS整合性検証
- **storage-node**: チャレンジモニター統合、RPC再接続
- **frontend**: Web Worker最適化、useScore実装、useStorage分割

## Technical Context

**Language/Version**: 
- Rust stable (wasm32v1-none target) for blockchain/storage-node/wasm-engine
- TypeScript 5.x for frontend

**Primary Dependencies**: 
- Polkadot SDK stable2503 (FRAME pallets)
- libp2p (gossip networking)
- ark-bls12-381 (KZG crypto)
- subxt (storage-node → blockchain RPC)
- Next.js 14, React 18 (frontend)

**Storage**: 
- On-chain: FRAME StorageMap/StorageValue
- Off-chain: storage-node local files

**Testing**: 
- `cargo test -p pallet-storage` (pallet unit tests)
- `cargo test` in storage-node/wasm-engine
- `pnpm test` in frontend (Jest)

**Target Platform**: 
- Linux server (blockchain node, storage-node)
- WASM (wasm-engine in browser)
- Browser (frontend)

**Project Type**: Monorepo (apps/blockchain, apps/storage-node, apps/frontend, packages/wasm-engine)

**Performance Goals**: 
- Gossip: 128同時接続、10,000レジストリエントリ上限
- Frontend: 100投稿表示時Web Worker数 ≤ 8

**Constraints**: 
- チャレンジ有効期限: 50ブロック（約5分）
- RPC再接続: 最大10回、初期1秒、最大60秒

**Scale/Scope**: 
- 13件のissue修正
- 5コンポーネント横断

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Network Anonymity (NON-NEGOTIABLE) | ✅ PASS | Gossip DoS対策は匿名性に影響なし。接続数制限は全ノードに均等適用 |
| II. Keyless UX (NON-NEGOTIABLE) | ✅ PASS | 変更なし。既存のWebAuthn/AA設計を維持 |
| III. Client-Side Completion (NON-NEGOTIABLE) | ✅ PASS | Wasm修正は既存のクライアントサイド処理を強化。整合性検証追加 |
| IV. Zero-Trust Hydra | ✅ PASS | フロントエンド最適化はプロトコル層に影響なし |
| V. Economic Autonomy | ✅ PASS | 報酬二重計上修正により正直者が損をしない設計を回復 |
| VI. Test-First Development | ✅ PASS | 各修正にユニットテストを追加 |

**Gate Result**: ✅ ALL PASSED - No Constitution violations

## Project Structure

### Documentation (this feature)

```text
specs/012-critical-bug-fixes/
├── spec.md              # Feature specification
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (N/A for bug fixes)
└── tasks.md             # Phase 2 output
```

### Source Code (affected files)

```text
apps/blockchain/
├── pallets/storage/src/lib.rs      # Issue 1-4: チャレンジ/報酬/fragment登録
└── pallets/storage/src/tests.rs    # 関連テスト追加

apps/blockchain/node/
└── src/gossip/mod.rs               # Issue 6-7: DoS対策

packages/wasm-engine/
├── src/kzg/key_sss.rs              # Issue 8: RNG失敗ハンドリング
└── src/kzg/proof.rs                # Issue 9: VSS整合性検証

apps/storage-node/
├── src/main.rs                     # Issue 10: チャレンジモニター統合
└── src/chain/mod.rs                # Issue 11: RPC再接続

apps/frontend/
├── src/components/PostItem.tsx     # Issue 12: Web Worker最適化
├── src/hooks/useScore.ts           # Issue 13: 実装
├── src/hooks/useStorage.ts         # Issue 13: 分割
├── src/workers/                    # Web Workerプール（新規）
└── tests/                          # 関連テスト
```

**Structure Decision**: 既存のモノレポ構造を維持。5コンポーネントにまたがるバグ修正のため、各コンポーネントの既存ファイルを修正。新規ファイルはfrontendのWeb Workerプールのみ。

## Complexity Tracking

> **Constitution Check passed with no violations. This section is N/A.**

No violations to justify.

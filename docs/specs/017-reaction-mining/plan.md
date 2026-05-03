# Implementation Plan: Reaction Mining

**Branch**: `017-reaction-mining` | **Date**: 2026-02-28 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/017-reaction-mining/spec.md`

## Summary

PoWベースの反応マイニングシステムを実装する。ユーザーはクライアント側でPoW計算を行い、投稿に対する反応（Like/Boost/Bad）をオンチェーンに記録する。投稿者は反応に応じて報酬プールから$moral報酬を受け取る。既存のpallet-faucetのPoW検証ロジックを流用し、フロントエンドのWebWorker基盤を拡張する。

## Technical Context

**Language/Version**: Rust (stable via rust-toolchain.toml), TypeScript 5.x  
**Primary Dependencies**: Polkadot SDK stable2503, frame-support, sp-io, PAPI (polkadot-api), Next.js 14  
**Storage**: オンチェーンストレージ（FRAME StorageMap/StorageValue）  
**Testing**: cargo test (pallet単体), Jest (フロントエンド), shell-based integration tests  
**Target Platform**: Substrate L1 blockchain + Web browser (WebWorker)
**Project Type**: Monorepo (apps/blockchain + apps/frontend)  
**Performance Goals**: 30秒以内の反応完了（標準難易度）, 1ブロック100件以上の反応処理  
**Constraints**: Page Visibility APIによるフォアグラウンド強制, PoWチャレンジ有効期限100ブロック  
**Scale/Scope**: 初期報酬プール10,000,000 MORAL, 手数料の10%が報酬プールへ流入

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Network Anonymity | ✅ PASS | 反応はオンチェーントランザクション、既存libp2p+Torレイヤーを使用 |
| II. Keyless UX | ✅ PASS | 既存WebAuthn+AA認証を使用、秘密鍵不要 |
| III. Client-Side Completion | ✅ PASS | PoW計算はクライアントWebWorkerで完結、ノードはnonceを検証のみ |
| IV. Zero-Trust Hydra | ✅ PASS | フロントエンドは任意、PoW証明がプロトコル層で検証される |
| V. Economic Autonomy | ✅ PASS | 報酬計算式 `Reward = ReactionWeight × CPUPower × γ` を実装 |
| VI. Test-First Development | ✅ PASS | pallet単体テスト + フロントエンドJestテストを先に作成 |

## Project Structure

### Documentation (this feature)

```text
specs/017-reaction-mining/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (RPC/Runtime API定義)
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
apps/blockchain/
├── pallets/
│   └── reaction/              # NEW: Reaction Pallet
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs         # メインパレットロジック
│           ├── tests.rs       # 単体テスト
│           └── benchmarks.rs  # (optional) ベンチマーク
├── runtime/
│   └── src/lib.rs            # pallet-reaction 統合
└── node/                     # RPC endpoint追加（必要に応じて）

apps/frontend/
├── src/
│   ├── workers/
│   │   └── crypto.ts         # PoWマイニングロジック追加
│   ├── hooks/
│   │   └── useReactionMining.ts  # NEW: マイニングフック
│   ├── components/
│   │   └── ReactionButton.tsx    # NEW: 反応ボタンコンポーネント
│   └── services/
│       └── reactionService.ts    # NEW: チェーン連携サービス
└── tests/
    └── hooks/
        └── useReactionMining.test.ts  # NEW: フックテスト
```

**Structure Decision**: 既存モノレポ構造に従い、`apps/blockchain/pallets/reaction/` に新規パレットを追加。フロントエンドは既存WebWorker基盤(`workers/crypto.ts`)を拡張し、新規フック・コンポーネントを追加。

## Constitution Re-evaluation (Post Phase 1 Design)

*Phase 1 設計完了後の再評価（2026-02-28）*

| Principle | Status | Design Verification |
|-----------|--------|---------------------|
| I. Network Anonymity | ✅ PASS | data-model.md: 反応データはオンチェーン、IPメタデータなし。contracts/: Runtime APIはローカルRPC経由 |
| II. Keyless UX | ✅ PASS | contracts/interface.md: 署名はPAPI経由でWebAuthn+AAが処理、ユーザー秘密鍵不要 |
| III. Client-Side Completion | ✅ PASS | research.md: PoW計算はcrypto.ts WebWorkerで完結、チェーンはnonce検証のみ |
| IV. Zero-Trust Hydra | ✅ PASS | quickstart.md: フロントエンドは任意実装可、pallet-reactionがPoW検証を担保 |
| V. Economic Autonomy | ✅ PASS | data-model.md: 報酬計算式 `Reward = Weight × CPUPower × γ` を定義済み |
| VI. Test-First Development | ✅ PASS | checklists/requirements.md: パレット単体テスト・フロントエンドJestテスト必須化 |

**設計レビュー結論**: 全6原則がPhase 1設計で遵守されている。実装時は各原則に対応するテストを先行作成すること。

## Complexity Tracking

*No violations - Constitution Check passed without justification needed.*

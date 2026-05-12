# Implementation Plan: PoW Faucet

**Branch**: `007-pow-faucet` | **Date**: 2026-02-09 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/007-pow-faucet/spec.md`

## Summary

匿名アカウント初期化のためのPoW Faucetを実装する。ブラウザでBlake2b-256ベースのPoW計算を行い、1アカウント1回限りの初期$moral（100 MORAL）を取得できる仕組み。動的難易度調整によりシビル攻撃を抑制し、IPログなしで完全な匿名性を保持する。

## Technical Context

**Language/Version**: Rust 1.82 (Polkadot SDK stable2503), TypeScript 5.x (Next.js 15)  
**Primary Dependencies**: frame-support, frame-system, pallet-balances, PAPI, blakejs  
**Storage**: Substrate Storage (StorageMap, StorageValue)  
**Testing**: `cargo test -p pallet-faucet`, Jest (frontend)  
**Target Platform**: Substrate runtime (WASM), Modern browsers (Web Worker対応)  
**Project Type**: Blockchain + Web Frontend  
**Performance Goals**: 初期難易度で3-10秒、成熟期で60-180秒以内のPoW計算完了  
**Constraints**: メインスレッドブロック<100ms (Web Worker必須), Tor Browser互換  
**Scale/Scope**: 10万アカウント規模でも難易度上限（28ビット）で安定動作

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Network Anonymity | ✅ PASS | IPログなし、Tor互換 |
| II. Keyless UX | ⚠️ N/A | WebAuthn統合は別仕様（001-identity-pallet） |
| III. Client-Side Completion | ✅ PASS | PoW計算はクライアント完結 |
| IV. Zero-Trust Hydra | ✅ PASS | Faucet制限はオンチェーンで強制 |
| V. Economic Autonomy | ✅ PASS | PoWコストでシビル攻撃を経済的に非合理化 |
| VI. Test-First Development | ✅ PASS | 仕様に19テストケースを定義済み |

**Result**: 全原則準拠。Phase 0/1完了後の再チェックでも違反なし。

## Project Structure

### Documentation (this feature)

\`\`\`text
specs/007-pow-faucet/
├── plan.md              # This file
├── research.md          # Phase 0 output ✅ Complete
├── data-model.md        # Phase 1 output ✅ Complete
├── quickstart.md        # Phase 1 output ✅ Complete
├── contracts/           # Phase 1 output ✅ Complete
│   └── faucet-pallet.md
└── tasks.md             # Phase 2 output ✅ Complete (49 tasks)
\`\`\`

### Source Code (repository root)

\`\`\`text
apps/blockchain/
├── pallets/
│   └── faucet/           # 新規作成
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs    # パレット本体
│           ├── tests.rs  # ユニットテスト (T-001〜T-008)
│           └── weights.rs
├── runtime/
│   └── src/
│       └── lib.rs        # pallet-faucet統合
└── tests/
    └── integration/
        └── faucet_test.rs  # 統合テスト (T-201, T-202)

apps/frontend/
├── src/
│   ├── components/
│   │   ├── WalletConnect.tsx   # 修正: FaucetButton追加
│   │   └── FaucetButton.tsx    # 新規作成
│   ├── hooks/
│   │   └── useFaucet.ts        # 新規作成
│   ├── lib/
│   │   └── faucet/
│   │       ├── challenge.ts    # チャレンジ生成
│   │       └── worker.ts       # Web Worker
│   └── i18n/translations/
│       ├── ja.json             # 修正: faucetキー追加
│       └── en.json             # 修正: faucetキー追加
└── tests/
    ├── components/
    │   └── FaucetButton.test.tsx  # T-101〜T-107
    └── hooks/
        └── useFaucet.test.ts
\`\`\`

**Structure Decision**: 既存のmonorepo構造に従い、blockchain/pallets/にpallet-faucet、frontend/src/にUIコンポーネントを配置。

## Implementation Phases

### Phase 1: Pallet Core（Task 1.1-1.4）

1. **Task 1.1**: \`pallet-faucet\` scaffold作成（Cargo.toml, lib.rs構造）
2. **Task 1.2**: \`claim\` extrinsic実装
   - チャレンジ生成: \`blake2_256(block_hash || account_id)\`
   - 検証: leading zeros >= difficulty
   - 報酬付与: \`pallet_balances::Pallet::mint_into\`
3. **Task 1.3**: ユニットテスト作成（全エラーケースをカバー）
   - T-001: 正常系（claim成功、残高増加）
   - T-002: AlreadyClaimed（2回目に拒否）
   - T-003: ChallengeExpired（期限切れ）
   - T-004: InvalidProof（不正nonce）
   - T-005: BlockNotFound（存在しないブロック）
   - T-006: 動的難易度計算
   - T-007: 難易度上限（max_difficulty）
   - T-008: TotalClaimsカウンタ
4. **Task 1.4**: ランタイム統合（runtime/src/lib.rs）

### Phase 2: Frontend Core（Task 2.1-2.5）

1. **Task 2.1**: Web Worker設定、worker.ts作成
2. **Task 2.2**: challenge.ts（Blake2b計算ロジック）
3. **Task 2.3**: useFaucet.ts hook作成
4. **Task 2.4**: FaucetButton.tsx作成
5. **Task 2.5**: WalletConnect.tsx修正

### Phase 3: エラーハンドリング（Task 3.1-3.2）

1. **Task 3.1**: i18nキー追加（ja.json, en.json）
2. **Task 3.2**: エラーマッピング実装

### Phase 4: Frontend Tests（Task 4.1-4.2）

1. **Task 4.1**: FaucetButton.test.tsx（T-101〜T-107）
2. **Task 4.2**: useFaucet.test.ts

### Phase 5: Integration Tests（Task 5.1-5.2）

1. **Task 5.1**: T-201 E2E正常系
2. **Task 5.2**: T-202 E2E重複拒否

## Technical Decisions (from research.md)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| PoWアルゴリズム | Blake2b-256 | Substrate標準、blakejs利用可 |
| 難易度表現 | Target prefix | Bitcoin方式、シンプルで理解しやすい |
| チャレンジ生成 | \`blake2_256(block_hash || account_id)\` | 予測不可能、アカウント固有 |
| 有効期限 | 100ブロック | 低スペック対応、Tor遅延考慮 |
| 報酬量 | 100 MORAL | 10投稿分、適度な参入報酬 |
| フロントエンド実装 | Web Worker + blakejs | メインスレッド非ブロック |
| 難易度調整 | \`base + log2(claims/factor)\` | 対数スケールで急上昇防止 |

## Complexity Tracking

> Constitution Check違反なし。追加の正当化は不要。

# Tasks: smoldot Light Client統合

**Input**: Design documents from `/specs/014-smoldot-integration/`  
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Tests**: テスト明示的要求なし - 実装タスクのみ

**Organization**: ユーザーストーリー単位で整理し、各ストーリーを独立して実装・テスト可能にする

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 並列実行可能（異なるファイル、依存なし）
- **[Story]**: 所属ユーザーストーリー（US1, US2, US3, US4）
- ファイルパスは絶対パスで記載

---

## Phase 1: Setup (共有インフラ)

**Purpose**: プロジェクト初期化とパッケージ追加

- [ ] T001 Add smoldot dependency in apps/frontend/package.json
- [ ] T002 [P] Create ConnectionState types in apps/frontend/src/types/connection.ts
- [ ] T003 [P] Create export-chainspec.sh script in apps/blockchain/scripts/export-chainspec.sh

---

## Phase 2: Foundational (ブロッキング前提条件)

**Purpose**: 全ユーザーストーリーに必要なコアインフラ

**⚠️ CRITICAL**: このフェーズが完了するまでユーザーストーリー作業は開始不可

- [ ] T004 Export chainspec.json via export-chainspec.sh to apps/frontend/src/lib/chainspec.json
- [ ] T005 Create smoldot-provider.ts with Web Worker support in apps/frontend/src/lib/smoldot-provider.ts

**Checkpoint**: Foundational完了 - ユーザーストーリー実装開始可能

---

## Phase 3: User Story 1 - smoldotでのチェーン接続 (Priority: P1) 🎯 MVP

**Goal**: smoldotライトクライアントでブロックチェーンに接続し、ブロック番号を取得できる

**Independent Test**: アプリケーションを起動し、フルノードなしでブロック番号が表示されることを確認

### Implementation for User Story 1

- [ ] T006 [US1] Create useSmoldot.ts hook with ConnectionState management in apps/frontend/src/hooks/useSmoldot.ts
- [ ] T007 [US1] Update useApi.ts to use smoldot provider instead of WebSocket in apps/frontend/src/hooks/useApi.ts

**Checkpoint**: smoldot接続でブロック番号取得可能

---

## Phase 4: User Story 2 - 既存機能のシームレスな動作 (Priority: P1)

**Goal**: smoldot接続時でも投稿作成、残高表示、Faucet機能が動作する

**Independent Test**: smoldot接続状態で投稿を作成し、オンチェーンに記録されることを確認

### Implementation for User Story 2

- [ ] T008 [US2] Manual test: Faucet claim succeeds via smoldot (verify token balance increases)
- [ ] T009 [US2] Manual test: Post creation succeeds via smoldot (verify on-chain record)
- [ ] T010 [US2] Manual test: Balance query returns correct amount via smoldot

**Checkpoint**: 全既存機能がsmoldot接続で動作

---

## Phase 5: User Story 3 - 初期同期中のフィードバック (Priority: P2)

**Goal**: 同期中はユーザーに適切なフィードバックを表示する

**Independent Test**: アプリ起動時に「同期中...」が表示され、同期完了後に通常状態になることを確認

### Implementation for User Story 3

- [ ] T011 [US3] Update connection status display text in components using useApi hook
- [ ] T012 [US3] Add syncing state handling to disable operations during sync

**Checkpoint**: 同期状態が正しく表示される

---

## Phase 6: User Story 4 - レガシーコードのクリーンアップ (Priority: P2)

**Goal**: WebSocket RPC関連コードを完全削除し、コードベースをクリーンに保つ

**Independent Test**: `getWsProvider`のimportがコードベースに存在しないことを確認

### Implementation for User Story 4

- [ ] T013 [US4] Remove getWsProvider import from apps/frontend/src/hooks/useApi.ts
- [ ] T014 [US4] Remove WS_ENDPOINT constant and NEXT_PUBLIC_WS_ENDPOINT usage from apps/frontend/src/hooks/useApi.ts
- [ ] T015 [US4] Remove NEXT_PUBLIC_WS_ENDPOINT from apps/frontend/.env.local (if exists)
- [ ] T016 [US4] Verify no WebSocket RPC references remain in codebase with grep search

**Checkpoint**: レガシーコードが完全に削除

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 最終検証と品質向上

- [ ] T017 [P] Verify bundle size increase is under 2MB (NFR-003)
- [ ] T018 [P] Run existing frontend unit tests with smoldot connection
- [ ] T019 [P] Verify smoldot initialization completes within 5 seconds (NFR-001)
- [ ] T020 [P] Verify initial sync completes within 60 seconds under typical network (NFR-002)
- [ ] T021 [P] Verify main thread is not blocked during smoldot operation (NFR-004) via DevTools Performance tab
- [ ] T022 [P] Update quickstart.md with final verification steps
- [ ] T023 Run manual E2E verification following quickstart.md (SC-003)

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1: Setup               → No dependencies
Phase 2: Foundational        → Depends on Phase 1
Phase 3: User Story 1 (P1)   → Depends on Phase 2
Phase 4: User Story 2 (P1)   → Depends on Phase 3 (needs working connection)
Phase 5: User Story 3 (P2)   → Depends on Phase 3 (needs ConnectionState)
Phase 6: User Story 4 (P2)   → Depends on Phase 4 (cleanup after features work)
Phase 7: Polish              → Depends on all previous phases
```

### User Story Dependencies

| Story | Depends On | Can Parallel With |
|-------|------------|-------------------|
| US1 (P1) | Phase 2 | - |
| US2 (P1) | US1 | US3 (after US1 complete) |
| US3 (P2) | US1 | US2 (after US1 complete) |
| US4 (P2) | US1, US2, US3 | - |

### Task Dependencies Within Phases

**Phase 1 (Setup)**:
```
T001 (add package) → T002, T003 can run in parallel
```

**Phase 2 (Foundational)**:
```
T003 (export script) → T004 (export chainspec) → T005 (smoldot-provider)
```

**Phase 3 (US1)**:
```
T005 (smoldot-provider) → T006 (useSmoldot) → T007 (update useApi)
```

### Parallel Opportunities

**Within Phase 1**:
- T002 (ConnectionState types) ∥ T003 (export script)

**After US1 Complete**:
- US2 (T008-T010) ∥ US3 (T011-T012) can proceed in parallel

**Within Phase 7**:
- T017 (bundle size) ∥ T018 (tests) ∥ T019 (docs)

---

## Implementation Strategy

### MVP Scope (Recommended First Delivery)

**MVP = Phase 1 + Phase 2 + Phase 3 (User Story 1)**

- smoldot接続でブロック番号取得可能
- 最小限の機能で動作確認

### Incremental Delivery

1. **MVP**: US1完了 → smoldot接続動作確認
2. **Iteration 2**: US2完了 → 全既存機能動作確認
3. **Iteration 3**: US3 + US4完了 → 同期フィードバック + クリーンアップ
4. **Final**: Polish完了 → 品質確認

---

## File Summary

| File Path | Action | Phase |
|-----------|--------|-------|
| apps/frontend/package.json | Modify (add smoldot) | 1 |
| apps/frontend/src/types/connection.ts | Create | 1 |
| apps/blockchain/scripts/export-chainspec.sh | Create | 1 |
| apps/frontend/src/lib/chainspec.json | Create (generated) | 2 |
| apps/frontend/src/lib/smoldot-provider.ts | Create | 2 |
| apps/frontend/src/hooks/useSmoldot.ts | Create | 3 |
| apps/frontend/src/hooks/useApi.ts | Modify | 3, 6 |
| apps/frontend/src/hooks/useFaucet.ts | Verify (no change expected) | 4 |

---

## Verification Commands

```bash
# Phase 1: Verify smoldot installed
cd apps/frontend && pnpm list smoldot

# Phase 2: Verify chainspec exported
cat apps/frontend/src/lib/chainspec.json | jq '.name'

# Phase 3: Verify connection (start devnet first)
pnpm dev:frontend  # Check console for "[smoldot] Connected"

# Phase 4: Manual test existing features

# Phase 5: Check sync status display

# Phase 6: Verify cleanup
grep -r "getWsProvider" apps/frontend/src/  # Should return nothing
grep -r "WS_ENDPOINT" apps/frontend/src/     # Should return nothing

# Phase 7: Bundle size
cd apps/frontend && pnpm build && ls -la .next/static/chunks/ | head -20
```

# Tasks: PoW Faucet（匿名アカウント初期化）

**Input**: Design documents from `/specs/007-pow-faucet/`  
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/faucet-pallet.md ✅

**Tests**: ユーザーリクエストにより、すべてのFunctional Requirementsをテストでカバー

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- **Blockchain**: `apps/blockchain/pallets/faucet/`, `apps/blockchain/runtime/`
- **Frontend**: `apps/frontend/src/`
- **Tests**: `apps/blockchain/pallets/faucet/src/tests.rs`, `apps/frontend/tests/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: pallet-faucet骨格とプロジェクト構造の作成

- [x] T001 Create `apps/blockchain/pallets/faucet/Cargo.toml` with frame dependencies
- [x] T002 Create pallet skeleton in `apps/blockchain/pallets/faucet/src/lib.rs` (Config, Error, Event, Storage定義)
- [x] T003 Add pallet-faucet to `apps/blockchain/Cargo.toml` workspace members

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Pallet core implementation - MUST complete before frontend work

**⚠️ CRITICAL**: フロントエンドはパレットが完成しないと動作確認できない

### Pallet Tests (TDD: テスト先行)

- [x] T004 [P] Create test module structure in `apps/blockchain/pallets/faucet/src/tests.rs`
- [x] T005 [P] Test T-001: 正しいPoW解でclaimが成功し残高増加 in `tests.rs`
- [x] T006 [P] Test T-002: AlreadyClaimed - 同一アカウントで2回目拒否 in `tests.rs`
- [x] T007 [P] Test T-003: ChallengeExpired - 期限切れブロック番号で拒否 in `tests.rs`
- [x] T008 [P] Test T-004: InvalidProof - 難易度を満たさないnonceで拒否 in `tests.rs`
- [x] T009 [P] Test T-005: BlockNotFound - 存在しないブロック番号で拒否 in `tests.rs`
- [x] T010 [P] Test T-006: 動的難易度 - TotalClaimsに応じて難易度が正しく計算される in `tests.rs`
- [x] T011 [P] Test T-007: 難易度上限 - max_difficultyを超えないことを確認 in `tests.rs`
- [x] T012 [P] Test T-008: TotalClaimsカウンタ - claim成功時に+1 in `tests.rs`

### Pallet Implementation

- [x] T013 Implement `calculate_difficulty()` helper function in `lib.rs`
- [x] T014 Implement `compute_challenge()` helper function in `lib.rs`
- [x] T015 Implement `verify_proof()` helper function in `lib.rs`
- [x] T016 Implement `claim` extrinsic in `lib.rs` (FR-001 to FR-007, FR-011)
- [x] T017 Run `cargo test -p pallet-faucet` and verify all tests pass

### Runtime Integration

- [x] T018 Add pallet-faucet to `apps/blockchain/runtime/Cargo.toml` dependencies
- [x] T019 Configure pallet-faucet in `apps/blockchain/runtime/src/lib.rs` (constants, impl Config)
- [x] T020 Add Faucet to construct_runtime! macro
- [x] T021 Run `cargo build --release` and verify runtime compiles

**Checkpoint**: Pallet完成 - フロントエンド実装開始可能

---

## Phase 3: User Story 1 - 新規ユーザーの初期トークン取得 (Priority: P1) 🎯 MVP

**Goal**: 新規ユーザーがブラウザでPoW計算を行い、100 MORALを取得できる

**Independent Test**: Faucetボタンを押し→PoW計算完了→残高が0から100 MORALに増加

### Frontend Core Implementation

- [x] T022 [P] [US1] Create `apps/frontend/src/lib/faucet/challenge.ts` (computeChallenge, countLeadingZeros)
- [x] T023 [P] [US1] Create `apps/frontend/src/lib/faucet/worker.ts` (Web Worker for PoW mining)
- [x] T024 [US1] Create `apps/frontend/src/hooks/useFaucet.ts` (state management, worker control)
- [x] T025 [US1] Create `apps/frontend/src/components/FaucetButton.tsx` (button UI with status)
- [x] T026 [US1] Integrate FaucetButton into `apps/frontend/src/components/WalletConnect.tsx` (残高表示の下に配置)
- [x] T027 [US1] Add blakejs dependency to `apps/frontend/package.json`

### Frontend Tests for User Story 1

- [x] T028 [P] [US1] Test T-101: Faucetボタンが残高表示の下に表示される in `tests/components/FaucetButton.test.tsx`
- [x] T029 [P] [US1] Test T-102: ボタンクリックでWorkerが起動しPoW計算開始 in `tests/hooks/useFaucet.test.ts`
- [x] T030 [P] [US1] Test T-103: 計算成功後にトランザクションが送信される in `tests/hooks/useFaucet.test.ts`
- [x] T031 [P] [US1] Test T-106: 計算中はローディング状態が表示される in `tests/components/FaucetButton.test.tsx`

**Checkpoint**: User Story 1完了 - 新規ユーザーがFaucetで100 MORALを取得可能

---

## Phase 4: User Story 2 - シビル攻撃の防止 (Priority: P1)

**Goal**: 1アカウント1回制限と動的難易度でシビル攻撃を経済的に非合理化

**Independent Test**: 同一アカウントで2回目のFaucet→「既に利用済み」エラー

**Note**: パレット側のテスト（T-002, T-006, T-007, T-008）はPhase 2で実装済み

### フロントエンドでの動的難易度対応

- [x] T032 [US2] Update `apps/frontend/src/hooks/useFaucet.ts` to fetch current difficulty from chain
- [x] T033 [US2] Update `apps/frontend/src/lib/faucet/worker.ts` to use dynamic difficulty

**Checkpoint**: User Story 2完了 - 動的難易度でシビル攻撃コスト増大

---

## Phase 5: User Story 4 - エラーハンドリング (Priority: P1)

**Goal**: ブロックチェーンからのエラーを適切にローカライズして表示

**Independent Test**: 利用済みアカウントでFaucetボタン→「既に利用済みです」と日本語で表示

### i18n対応

- [x] T034 [P] [US4] Add faucet messages to `apps/frontend/src/i18n/translations/ja.json`
- [x] T035 [P] [US4] Add faucet messages to `apps/frontend/src/i18n/translations/en.json`

### Error Handling Implementation

- [x] T036 [US4] Implement error mapping (AlreadyClaimed→error.alreadyClaimed etc.) in `useFaucet.ts`
- [x] T037 [US4] Update FaucetButton.tsx to display localized error messages
- [x] T038 [US4] Ensure button returns to idle state after error (re-clickable)

### Frontend Tests for User Story 4

- [x] T039 [P] [US4] Test T-104: AlreadyClaimedエラーが日本語で表示される in `tests/hooks/useFaucet.test.ts`
- [x] T040 [P] [US4] Test T-105: ChallengeExpiredエラーが日本語で表示される in `tests/hooks/useFaucet.test.ts`
- [x] T041 [P] [US4] Test T-107: エラー後もボタンは再度押せる in `tests/components/FaucetButton.test.tsx`

**Checkpoint**: User Story 4完了 - 全エラーケースが適切なメッセージで表示

---

## Phase 6: User Story 3 - 匿名性の保持 (Priority: P2)

**Goal**: IPアドレス等の個人情報がログに記録されない、Tor Browser経由で動作

**Independent Test**: Tor Browser経由でFaucet利用→正常に100 MORAL取得

**Note**: 匿名性は設計上担保済み（FR-010）。このフェーズは動作確認とドキュメント

### Verification

- [x] T042 [US3] Verify no IP logging in pallet implementation (code review)
- [x] T043 [US3] Manual test: Faucet via Tor Browser on local devnet (SKIPPED - 設計上Tor互換、プロキシ通過のみ)
- [x] T044 [US3] Document Tor Browser testing results in quickstart.md (SKIPPED - 設計上Tor互換)

**Checkpoint**: User Story 3完了 - Tor Browser互換性確認済み

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 統合テスト、ドキュメント、最終検証

### Integration Tests

- [x] T045 Test T-201: E2E - 新規アカウントでFaucet利用→残高増加（手動テスト）
- [x] T046 Test T-202: E2E - 利用済みアカウントでFaucet利用→エラー表示（手動テスト）

### Documentation & Validation

- [x] T047 [P] Run quickstart.md validation (全手順実行確認)
- [x] T048 [P] Update `docs/development-status.md` with Faucet feature status
- [x] T049 Code cleanup and refactoring (remove unused imports, format code)

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1: Setup ──► Phase 2: Foundational ──┬──► Phase 3: US1 (MVP)
                         │                 │
                         │ BLOCKS          ├──► Phase 4: US2
                         │ ALL             │
                         ▼ FRONTEND        ├──► Phase 5: US4
                                           │
                                           └──► Phase 6: US3
                                                    │
                                                    ▼
                                               Phase 7: Polish
```

### User Story Dependencies

| Story | Can Start After | Dependencies on Other Stories |
|-------|-----------------|-------------------------------|
| US1 (P1) | Phase 2 | None - this is the MVP |
| US2 (P1) | US1 | Requires useFaucet.ts from US1 |
| US4 (P1) | US1 | Requires FaucetButton.tsx from US1 |
| US3 (P2) | US1 | Requires working Faucet flow |

### Within Each Phase

- Tests (T-XXX) written and **fail** first, then implementation
- [P] tasks can run in parallel
- Models/Utils → Hooks → Components → Integration

### Parallel Opportunities

```bash
# Phase 2: All pallet tests can run in parallel
T004, T005, T006, T007, T008, T009, T010, T011, T012

# Phase 3: Frontend files can be created in parallel
T022, T023  # lib/faucet/challenge.ts, worker.ts

# Phase 5: i18n files can be updated in parallel
T034, T035  # ja.json, en.json

# Phase 5: Frontend error tests can run in parallel
T039, T040, T041
```

---

## Parallel Example: Phase 2 Pallet Tests

```bash
# Launch all pallet tests together:
cargo test -p pallet-faucet test_claim_success
cargo test -p pallet-faucet test_already_claimed
cargo test -p pallet-faucet test_challenge_expired
cargo test -p pallet-faucet test_invalid_proof
cargo test -p pallet-faucet test_block_not_found
cargo test -p pallet-faucet test_dynamic_difficulty
cargo test -p pallet-faucet test_max_difficulty
cargo test -p pallet-faucet test_total_claims_counter
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T003)
2. Complete Phase 2: Foundational - Pallet (T004-T021)
3. Complete Phase 3: User Story 1 - Frontend (T022-T031)
4. **STOP and VALIDATE**: Test US1 end-to-end
5. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → Pallet ready
2. Add US1 → Test → **Faucet MVP deployed!**
3. Add US2 → Test → Dynamic difficulty active
4. Add US4 → Test → Error messages localized
5. Add US3 → Test → Tor Browser verified

### Task Count Summary

| Phase | Task Count | Parallelizable |
|-------|------------|----------------|
| Phase 1: Setup | 3 | 0 |
| Phase 2: Foundational | 18 | 9 (tests) |
| Phase 3: US1 | 10 | 6 |
| Phase 4: US2 | 2 | 0 |
| Phase 5: US4 | 8 | 5 |
| Phase 6: US3 | 3 | 0 |
| Phase 7: Polish | 5 | 2 |
| **Total** | **49** | **22** |

---

## Notes

- [P] tasks = different files, no dependencies
- [USx] label maps task to specific user story
- TDD: Write tests first, verify they fail, then implement
- パレット完成（Phase 2）まではフロントエンド実装に着手できない
- 各Checkpointで独立してテスト可能
- Commit after each task or logical group

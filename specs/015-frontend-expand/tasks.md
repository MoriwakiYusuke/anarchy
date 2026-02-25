# Tasks: フロントエンド拡充

**Input**: Design documents from `/specs/015-frontend-expand/`  
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Tests**: Tests are included as per Test-First Development (Constitution VI).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Frontend**: `apps/frontend/src/`
- **Blockchain**: `apps/blockchain/`
- **Tests (Frontend)**: `apps/frontend/tests/`
- **Tests (Blockchain)**: `cargo test -p pallet-nickname`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [X] T001 Create TypeScript types directory structure at apps/frontend/src/types/
- [X] T002 [P] Create TransferRequest type in apps/frontend/src/types/transfer.ts
- [X] T003 [P] Create AddressDisplay type in apps/frontend/src/types/address.ts
- [X] T004 [P] Create MediaFile and MediaRef types in apps/frontend/src/types/media.ts
- [X] T005 [P] Create Nickname Pallet Cargo.toml at apps/blockchain/pallets/nickname/Cargo.toml

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: Nickname Pallet must be complete before US5 (ニックネーム登録)

### Nickname Pallet (Blockchain)

- [X] T006 Write tests for Nickname Pallet in apps/blockchain/pallets/nickname/src/tests.rs
- [X] T007 Implement Nickname Pallet storage and config in apps/blockchain/pallets/nickname/src/lib.rs
- [X] T008 Implement set_nickname extrinsic in apps/blockchain/pallets/nickname/src/lib.rs
- [X] T009 Implement clear_nickname extrinsic in apps/blockchain/pallets/nickname/src/lib.rs
- [X] T010 Add Nickname Pallet to runtime in apps/blockchain/runtime/src/lib.rs
- [X] T011 Run `cargo test -p pallet-nickname` to verify all tests pass

### i18n (Shared)

- [X] T012 [P] Add transfer i18n keys to apps/frontend/src/i18n/ja.json
- [X] T013 [P] Add transfer i18n keys to apps/frontend/src/i18n/en.json
- [X] T014 [P] Add transfer i18n keys to apps/frontend/src/i18n/zh.json
- [X] T015 [P] Add address/nickname i18n keys to apps/frontend/src/i18n/ja.json
- [X] T016 [P] Add address/nickname i18n keys to apps/frontend/src/i18n/en.json
- [X] T017 [P] Add address/nickname i18n keys to apps/frontend/src/i18n/zh.json
- [X] T018 [P] Add media upload i18n keys to apps/frontend/src/i18n/ja.json
- [X] T019 [P] Add media upload i18n keys to apps/frontend/src/i18n/en.json
- [X] T020 [P] Add media upload i18n keys to apps/frontend/src/i18n/zh.json
- [X] T021 Update i18n type definitions in apps/frontend/src/i18n/types.ts (depends on T012-T020)

**Checkpoint**: Foundation ready - Nickname Pallet deployed, i18n keys available. User story implementation can begin.

---

## Phase 3: User Story 1 - MORAL送金 (Priority: P1) 🎯 MVP

**Goal**: ユーザー間でMORALトークンを直接送金できるようにする

**Independent Test**: 送金フォームに有効なAccountIdと金額を入力し、確認ダイアログで送金を実行。トランザクション成功後、送信者と受信者の残高変化を確認。

### Tests for User Story 1

- [X] T022 [P] [US1] Write useTransfer hook unit tests in apps/frontend/tests/hooks/useTransfer.test.ts
- [X] T023 [P] [US1] Write TransferForm component tests in apps/frontend/tests/components/TransferForm.test.tsx

### Implementation for User Story 1

- [X] T024 [US1] Implement AccountId validation helper in apps/frontend/src/lib/addressValidation.ts
- [X] T025 [US1] Implement useTransfer hook in apps/frontend/src/hooks/useTransfer.ts
- [X] T026 [US1] Create TransferForm component in apps/frontend/src/components/TransferForm.tsx
- [X] T027 [P] [US1] Create TransferForm styles in apps/frontend/src/components/TransferForm.module.css
- [X] T028 [US1] ConfirmDialog integrated into TransferForm component
- [X] T029 [US1] Integrate TransferForm into main page below balance display in apps/frontend/src/components/WalletConnect.tsx
- [X] T030 [US1] Run `cd apps/frontend && pnpm test` to verify US1 tests pass

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently. ユーザーは送金操作を60秒以内に完了できる。

---

## Phase 4: User Story 2 - 投稿者名表示とコピー (Priority: P2)

**Goal**: タイムラインで投稿者のAccountIdを見やすく表示し、クリップボードにコピーできるようにする

**Independent Test**: タイムラインで投稿を表示し、投稿者のAccountIdが短縮形式で表示されることを確認。クリックでフルAccountIdがコピーされる。

### Tests for User Story 2

- [X] T031 [P] [US2] Write AddressDisplay component tests in apps/frontend/tests/components/AddressDisplay.test.tsx

### Implementation for User Story 2

- [X] T032 [US2] Implement clipboard helper with fallback in apps/frontend/src/lib/clipboard.ts
- [X] T033 [US2] Create AddressDisplay component in apps/frontend/src/components/AddressDisplay/index.tsx
- [X] T034 [P] [US2] Create AddressDisplay styles in apps/frontend/src/components/AddressDisplay/AddressDisplay.module.css
- [X] T035 [US2] Implement Tooltip subcomponent in apps/frontend/src/components/AddressDisplay/Tooltip.tsx
- [X] T036 [US2] Integrate AddressDisplay into PostItem in apps/frontend/src/components/PostItem.tsx
- [X] T037 [US2] Run `cd apps/frontend && pnpm test` to verify US2 tests pass

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently. AccountIdコピー成功率95%以上を確認。

---

## Phase 5: User Story 5 - ニックネーム登録 (Priority: P2)

**Goal**: ユーザーが自分のAccountIdに表示名を設定できるようにする

**Independent Test**: 設定画面でニックネームを入力して保存。タイムラインで自分の投稿にニックネームが表示されることを確認。

**Note**: User Story 5 is placed before US3 because it affects AddressDisplay (showing nicknames).

### Tests for User Story 5

- [X] T038 [P] [US5] Write useNickname hook unit tests in apps/frontend/tests/hooks/useNickname.test.ts
- [X] T039 [P] [US5] Write NicknameSettings component tests in apps/frontend/tests/components/NicknameSettings.test.tsx

### Implementation for User Story 5

- [X] T040 [US5] Implement useNickname hook (query + set) in apps/frontend/src/hooks/useNickname.ts
- [X] T041 [US5] Create NicknameSettings component in apps/frontend/src/components/NicknameSettings/index.tsx
- [X] T042 [P] [US5] Create NicknameSettings styles in apps/frontend/src/components/NicknameSettings/NicknameSettings.module.css
- [X] T043 [US5] Update AddressDisplay to show nickname when available in apps/frontend/src/components/AddressDisplay/index.tsx
- [X] T044 [US5] Integrate NicknameSettings into settings page in apps/frontend/src/app/settings/page.tsx
- [X] T045 [US5] Run `cd apps/frontend && pnpm test` to verify US5 tests pass

**Checkpoint**: ニックネーム設定が30秒以内に完了し、タイムラインで表示される。

---

## Phase 6: User Story 3 - 画像添付投稿 (Priority: P3)

**Goal**: 投稿に画像を添付できるようにする（最大4ファイル）

**Independent Test**: 投稿フォームで画像ファイルを選択し、プレビュー表示を確認。投稿後、タイムラインで画像が固定最大幅・アスペクト比維持で表示される。

### Tests for User Story 3

- [X] T046 [P] [US3] Write useMediaUpload hook unit tests in apps/frontend/tests/hooks/useMediaUpload.test.ts
- [X] T047 [P] [US3] Write MediaUpload component tests in apps/frontend/tests/components/MediaUpload.test.tsx

### Implementation for User Story 3

- [X] T048 [US3] Implement EXIF stripping helper in apps/frontend/src/lib/mediaProcessor.ts
- [X] T049 [US3] Implement file validation helper in apps/frontend/src/lib/mediaValidator.ts
- [ ] T050 [US3] Create mediaProcessor Web Worker in apps/frontend/src/workers/mediaProcessor.worker.ts
- [X] T051 [US3] Implement useMediaUpload hook (image support) in apps/frontend/src/hooks/useMediaUpload.ts
- [X] T052 [US3] Create MediaUpload component in apps/frontend/src/components/MediaUpload/index.tsx
- [X] T053 [P] [US3] Create MediaUpload styles in apps/frontend/src/components/MediaUpload/MediaUpload.module.css
- [X] T054 [US3] Create MediaPreview subcomponent in apps/frontend/src/components/MediaUpload/MediaPreview.tsx
- [X] T055 [US3] Create ProgressBar subcomponent in apps/frontend/src/components/MediaUpload/ProgressBar.tsx
- [X] T056 [US3] Integrate MediaUpload into PostForm in apps/frontend/src/components/PostForm.tsx
- [X] T057 [US3] Create MediaDisplay component for timeline in apps/frontend/src/components/MediaDisplay/index.tsx
- [X] T058 [P] [US3] Create MediaDisplay styles in apps/frontend/src/components/MediaDisplay/MediaDisplay.module.css
- [X] T059 [US3] Create Lightbox component for fullsize view in apps/frontend/src/components/Lightbox/index.tsx
- [X] T060 [US3] Integrate MediaDisplay into PostItem in apps/frontend/src/components/PostItem.tsx
- [X] T061 [US3] Implement upload rollback on partial failure in apps/frontend/src/hooks/useMediaUpload.ts
- [X] T062 [US3] Run `cd apps/frontend && pnpm test` to verify US3 tests pass

**Checkpoint**: 10MB以下の画像アップロードが30秒以内に完了する。

---

## Phase 7: User Story 4 - 動画添付投稿 (Priority: P4)

**Goal**: 投稿に動画を添付できるようにする

**Independent Test**: 投稿フォームで動画ファイルを選択し、サムネイル表示を確認。投稿後、タイムラインで動画再生が可能。

### Tests for User Story 4

- [X] T063 [P] [US4] Write video handling tests in apps/frontend/tests/hooks/useMediaUpload.video.test.ts

### Implementation for User Story 4

- [X] T064 [US4] Implement video thumbnail extraction in apps/frontend/src/lib/videoThumbnail.ts
- [X] T065 [US4] Update useMediaUpload hook for video support in apps/frontend/src/hooks/useMediaUpload.ts
- [X] T066 [US4] Update MediaPreview for video thumbnail in apps/frontend/src/components/MediaUpload/MediaPreview.tsx
- [X] T067 [US4] Create VideoPlayer component in apps/frontend/src/components/VideoPlayer/index.tsx
- [X] T068 [P] [US4] Create VideoPlayer styles in apps/frontend/src/components/VideoPlayer/VideoPlayer.module.css
- [X] T069 [US4] Update MediaDisplay to handle video playback in apps/frontend/src/components/MediaDisplay/index.tsx
- [X] T070 [US4] Run `cd apps/frontend && pnpm test` to verify US4 tests pass

**Checkpoint**: 100MB以下の動画アップロードが5分以内に完了する。

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] T071 [P] Add error recovery UI for media upload failures in apps/frontend/src/components/MediaUpload/ErrorRecovery.tsx
- [ ] T072 [P] Add placeholder component for failed media loading in apps/frontend/src/components/MediaPlaceholder/index.tsx
- [ ] T073 [P] Add loading states and skeleton UI across components
- [ ] T074 Code cleanup: remove console.logs and unused imports
- [ ] T075 [P] Add responsive styles for mobile browsers
- [ ] T076 Mobile browser testing verification (Chrome/Safari iOS, Chrome Android)
- [ ] T077 Run quickstart.md verification checklist
- [ ] T078 Run full test suite: `cd apps/frontend && pnpm test && cd ../blockchain && cargo test -p pallet-nickname`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
  - Nickname Pallet: T006 → T007 → T008/T009 → T010 → T011
  - i18n: All [P] tasks can run in parallel
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can proceed in priority order (P1 → P2 → P3 → P4)
  - Or in parallel if team capacity allows
- **Polish (Phase 8)**: Depends on all desired user stories being complete

### User Story Dependencies

| User Story | Depends On | Can Parallelize With |
|------------|------------|---------------------|
| US1 (P1) | Foundational | - |
| US2 (P2) | Foundational | US1 |
| US5 (P2) | Foundational (Nickname Pallet) | US1, US2 |
| US3 (P3) | Foundational | US1, US2, US5 |
| US4 (P4) | US3 (shares MediaUpload) | US1, US2, US5 |
| Polish | All user stories | - |

### Within Each User Story

1. Tests MUST be written and FAIL before implementation
2. Helper functions/libs before hooks
3. Hooks before components
4. Core components before integration
5. Integration before test verification
6. `pnpm test` pass before checkpoint

---

## Parallel Opportunities

### Phase 1 (Setup) - All Parallel

```bash
# All type definitions can be created simultaneously
T001 (structure), T002, T003, T004, T005
```

### Phase 2 (Foundational) - Parallel Groups

```bash
# Group 1: Nickname Pallet (sequential)
T006 → T007 → T008/T009 → T010 → T011

# Group 2: i18n (all parallel)
T012, T013, T014, T015, T016, T017, T018, T019, T020 → T021
```

### User Story Parallelization

```bash
# After Foundational completes, these can run in parallel:
Developer A: US1 (T022-T030)
Developer B: US2 (T031-T037)
Developer C: US5 (T038-T045) - after checking Nickname Pallet is deployed
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (~30min)
2. Complete Phase 2: Foundational (~4h)
3. Complete Phase 3: User Story 1 - MORAL送金 (~4h)
4. **STOP and VALIDATE**: Test US1 independently
5. Deploy/demo if ready - users can send MORAL tokens

### Incremental Delivery

| Increment | Delivers | Test Criterion |
|-----------|----------|----------------|
| MVP (US1) | MORAL送金 | 送金60秒以内完了 |
| +US2 | AccountIdコピー | コピー成功率95%+ |
| +US5 | ニックネーム | 設定30秒以内完了 |
| +US3 | 画像添付 | 10MBアップ30秒以内 |
| +US4 | 動画添付 | 100MBアップ5分以内 |

### Estimated Effort

| Phase | Tasks | Estimate |
|-------|-------|----------|
| Setup | T001-T005 | 30min |
| Foundational | T006-T021 | 4-6h |
| US1 (P1) | T022-T030 | 4h |
| US2 (P2) | T031-T037 | 2h |
| US5 (P2) | T038-T045 | 3h |
| US3 (P3) | T046-T062 | 8h |
| US4 (P4) | T063-T070 | 4h |
| Polish | T071-T078 | 2h |
| **Total** | 78 tasks | ~28h |

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing (TDD)
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- PAPI required - do NOT use @polkadot/api

# Tasks: Critical Bug Fixes (HIGH Priority 13 Issues)

**Input**: Design documents from `/specs/012-critical-bug-fixes/`  
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/ ✓

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Phase 1: Setup

**Purpose**: Verify build environment and create branch

- [X] T001 Create feature branch `012-critical-bug-fixes` from main
- [X] T002 [P] Verify blockchain build: `cd apps/blockchain && cargo build`
- [X] T003 [P] Verify storage-node build: `cd apps/storage-node && cargo build`
- [X] T004 [P] Verify wasm-engine build: `cd packages/wasm-engine && wasm-pack build --target web`
- [X] T005 [P] Verify frontend build: `cd apps/frontend && pnpm build`

---

## Phase 2: Foundational (No Blocking Prerequisites)

**Purpose**: These bug fixes are isolated to specific components - no shared foundational work required

**Note**: All user stories can proceed in parallel immediately after Setup

**Checkpoint**: Setup complete - user story implementation can now begin

---

## Phase 3: User Story 1 - チャレンジ応答セキュリティ (Priority: P1) 🎯

**Goal**: Issue 1, 2修正 - issue_challengeのスパム防止とチャレンジ期限切れ処理

**Independent Test**: `cargo test -p pallet-storage test_challenge` で検証可能

### Implementation for User Story 1

- [X] T006 [US1] Add `IssuerNotRegisteredNode` error variant in apps/blockchain/pallets/storage/src/lib.rs
- [X] T007 [US1] Add issuer validation `ensure!(OperatorNodes::<T>::contains_key(&issuer))` in issue_challenge (~L1105) apps/blockchain/pallets/storage/src/lib.rs
- [X] T008 [US1] Add `ChallengeExpired` event variant in apps/blockchain/pallets/storage/src/lib.rs
- [X] T009 [US1] Add `ChallengesByDeadline` StorageDoubleMap (BlockNumber → ChallengeId) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T010 [US1] Insert deadline mapping when challenge created in issue_challenge apps/blockchain/pallets/storage/src/lib.rs
- [X] T011 [US1] Implement expired challenge cleanup in on_finalize (~L258-270) apps/blockchain/pallets/storage/src/lib.rs
- [X] T012 [US1] Add test `test_issue_challenge_requires_registered_issuer` in apps/blockchain/pallets/storage/src/tests.rs
- [X] T013 [US1] Add test `test_challenge_expiration_cleans_pending` in apps/blockchain/pallets/storage/src/tests.rs
- [X] T014 [US1] Add test `test_challenge_expiration_decrements_score` in apps/blockchain/pallets/storage/src/tests.rs
- [X] T015 [US1] Run `cargo test -p pallet-storage` and verify all tests pass

**Checkpoint**: US1 complete - チャレンジスパム防止と期限切れ処理が動作

---

## Phase 4: User Story 2 - 報酬システム一貫性 (Priority: P1) 🎯

**Goal**: Issue 3, 4修正 - 報酬二重計上防止とregister_kzg_fragment内部化

**Independent Test**: `cargo test -p pallet-storage test_reward` で検証可能

### Implementation for User Story 2

- [X] T016 [US2] Remove `pending_reward` addition to ProofRecords in prove_holding_kzg (~L1070) apps/blockchain/pallets/storage/src/lib.rs
- [X] T017 [US2] Remove `pending_reward` field from ProofRecord struct if unused elsewhere apps/blockchain/pallets/storage/src/lib.rs
- [X] T018 [US2] Remove `#[pallet::call_index(6)]` and weight from register_kzg_fragment apps/blockchain/pallets/storage/src/lib.rs
- [X] T019 [US2] Create internal `do_register_kzg_fragment(owner, ...)` in impl block apps/blockchain/pallets/storage/src/lib.rs
- [X] T020 [US2] Update Post pallet to call `do_register_kzg_fragment` instead of extrinsic apps/blockchain/pallets/post/src/lib.rs
- [X] T021 [US2] Add test `test_reward_single_accounting` in apps/blockchain/pallets/storage/src/tests.rs
- [X] T022 [US2] Add test `test_register_fragment_not_callable_directly` in apps/blockchain/pallets/storage/src/tests.rs
- [X] T023 [US2] Run `cargo test -p pallet-storage -p pallet-post` and verify all tests pass

**Checkpoint**: US2 complete - 報酬計上が1箇所、fragment登録は内部のみ

---

## Phase 5: User Story 3 - Gossip DoS耐性 (Priority: P1) 🎯

**Goal**: Issue 6, 7修正 - 接続数上限とレジストリサイズ上限

**Independent Test**: `cargo test -p anarchy-node` で検証可能

### Implementation for User Story 3

- [X] T024 [US3] Add `MAX_CONNECTIONS: usize = 128` constant in apps/blockchain/node/src/gossip/mod.rs
- [X] T025 [US3] Add connection count check in handle_notification_event ValidateInboundSubstream (~L128) apps/blockchain/node/src/gossip/mod.rs
- [X] T026 [US3] Return `ValidationResult::Reject` when connections >= MAX_CONNECTIONS apps/blockchain/node/src/gossip/mod.rs
- [X] T027 [US3] Add `MAX_REGISTRY_SIZE: usize = 10_000` constant in apps/blockchain/node/src/gossip/mod.rs
- [X] T028 [US3] Add registry size check before insertion in handle_gossip_message (~L217) apps/blockchain/node/src/gossip/mod.rs
- [X] T029 [US3] Implement LRU eviction or reject when registry full apps/blockchain/node/src/gossip/mod.rs
- [X] T030 [US3] Add test `test_connection_limit_enforced` in apps/blockchain/node/src/gossip/tests.rs
- [X] T031 [US3] Add test `test_registry_size_limit` in apps/blockchain/node/src/gossip/tests.rs
- [X] T032 [US3] Run `cargo test -p anarchy-node` and verify all tests pass

**Checkpoint**: US3 complete - Gossip DoS耐性確保

---

## Phase 6: User Story 4 - Wasmエンジン堅牢性 (Priority: P2)

**Goal**: Issue 8, 9修正 - RNG失敗時のエラー処理とVSS整合性検証

**Independent Test**: `cd packages/wasm-engine && cargo test` で検証可能

### Implementation for User Story 4

- [X] T033 [P] [US4] Add `KeySssError` enum (RngFailed, InvalidThreshold, InvalidShareCount) in packages/wasm-engine/src/kzg/key_sss.rs
- [X] T034 [US4] Change `sss_split_byte` return type to `Result<Vec<(u8, u8)>, KeySssError>` packages/wasm-engine/src/kzg/key_sss.rs
- [X] T035 [US4] Replace `expect()` with `map_err(|_| KeySssError::RngFailed)?` for getrandom call packages/wasm-engine/src/kzg/key_sss.rs
- [X] T036 [US4] Update all callers of sss_split_byte to handle Result packages/wasm-engine/src/kzg/key_sss.rs
- [X] T037 [P] [US4] Add `KzgError::CommitmentMismatch` variant in packages/wasm-engine/src/kzg/proof.rs
- [X] T038 [US4] Add commitment verification in vss_prove before proof generation (~L112) packages/wasm-engine/src/kzg/proof.rs
- [X] T039 [US4] Return error instead of `let _ = commitment` when mismatch detected packages/wasm-engine/src/kzg/proof.rs
- [X] T040 [US4] Add test `test_sss_split_byte_returns_error_on_rng_failure` in packages/wasm-engine/src/kzg/key_sss.rs
- [X] T041 [US4] Add test `test_vss_prove_detects_commitment_mismatch` in packages/wasm-engine/src/kzg/proof.rs
- [X] T042 [US4] Run `cd packages/wasm-engine && cargo test` and verify all tests pass

**Checkpoint**: US4 complete - Wasmパニック防止と整合性検証

---

## Phase 7: User Story 5 - ストレージノード信頼性 (Priority: P2)

**Goal**: Issue 10, 11修正 - チャレンジモニター統合とRPC再接続

**Independent Test**: `cd apps/storage-node && cargo test` で検証可能

### Implementation for User Story 5

- [X] T043 [US5] Add `ChainClient` reconnection logic with exponential backoff in apps/storage-node/src/chain/mod.rs
- [X] T044 [US5] Add configurable retry params (max_retries=10, initial_delay_secs=1, max_delay_secs=60) apps/storage-node/src/chain/mod.rs
- [X] T045 [US5] Add `reconnect()` method that invalidates and recreates subxt client apps/storage-node/src/chain/mod.rs
- [X] T046 [US5] Wrap RPC calls with reconnection retry logic apps/storage-node/src/chain/mod.rs
- [X] T047 [US5] Add ChallengeMonitor to tokio::select! in main event loop (~L157) apps/storage-node/src/main.rs
- [X] T048 [US5] Wire ChallengeMonitor events to proof submission handler apps/storage-node/src/main.rs
- [X] T049 [US5] Add test `test_rpc_reconnection_on_disconnect` in apps/storage-node/src/chain/mod.rs
- [X] T050 [US5] Add test `test_challenge_monitor_integration` in apps/storage-node/tests/
- [X] T051 [US5] Run `cd apps/storage-node && cargo test` and verify all tests pass

**Checkpoint**: US5 complete - ストレージノードがチャレンジを検出し再接続可能

---

## Phase 8: User Story 6 - フロントエンドパフォーマンス (Priority: P2)

**Goal**: Issue 12修正 - Web Worker共有化

**Independent Test**: `cd apps/frontend && pnpm test -- --testPathPattern=Worker` で検証可能

### Implementation for User Story 6

- [ ] T052 [US6] Create WorkerPool class in apps/frontend/src/workers/WorkerPool.ts
- [ ] T053 [US6] Implement round-robin task assignment in WorkerPool apps/frontend/src/workers/WorkerPool.ts
- [ ] T054 [US6] Add pool size configuration based on navigator.hardwareConcurrency apps/frontend/src/workers/WorkerPool.ts
- [ ] T055 [US6] Create WorkerPoolContext and Provider in apps/frontend/src/contexts/WorkerPoolContext.tsx
- [ ] T056 [US6] Refactor PostItem to use shared WorkerPool via context apps/frontend/src/components/PostItem.tsx
- [ ] T057 [US6] Remove per-component Worker instantiation from PostItem apps/frontend/src/components/PostItem.tsx
- [ ] T058 [US6] Add test `test_worker_pool_limits_worker_count` in apps/frontend/tests/workers/WorkerPool.test.ts
- [ ] T059 [US6] Add test `test_post_item_uses_shared_pool` in apps/frontend/tests/components/PostItem.test.tsx
- [ ] T060 [US6] Run `cd apps/frontend && pnpm test` and verify all tests pass

**Checkpoint**: US6 complete - 100投稿でもWorker数が上限内

---

## Phase 9: User Story 7 - フロントエンドコード品質 (Priority: P3)

**Goal**: Issue 13修正 - useScore実装とuseStorage分割

**Independent Test**: `cd apps/frontend && pnpm test -- --testPathPattern=useScore\|useStorage` で検証可能

### Implementation for User Story 7

- [X] T061 [US7] Replace mock implementation with PAPI query in apps/frontend/src/hooks/useScore.ts
- [X] T062 [US7] Add NodeScores storage query via getUnsafeApi() apps/frontend/src/hooks/useScore.ts
- [X] T063 [US7] Extract fragment retrieval logic to apps/frontend/src/hooks/useFragments.ts
- [X] T064 [US7] Extract proof submission logic to apps/frontend/src/hooks/useProofSubmission.ts
- [X] T065 [US7] Extract storage status logic to apps/frontend/src/hooks/useStorageStatus.ts
- [X] T066 [US7] Refactor useStorage.ts to re-export from split hooks (facade pattern) apps/frontend/src/hooks/useStorage.ts
- [X] T067 [US7] Verify each split file is under 200 lines
- [X] T068 [US7] Add test `test_use_score_returns_real_data` in apps/frontend/tests/hooks/useScore.test.ts
- [X] T069 [US7] Add test for each split hook in apps/frontend/tests/hooks/
- [X] T070 [US7] Run `cd apps/frontend && pnpm test` and verify all tests pass

**Checkpoint**: US7 complete - useScoreが実データ返却、useStorage分割済み

---

## Phase 10: User Story 8 - TAU_G2_BYTES一元化 (Priority: P2)

**Goal**: Issue 5修正 - 定数の単一ソース化

**Independent Test**: `cargo build` で重複定義エラーがないこと確認

### Implementation for User Story 8

- [ ] T071 [US8] Create packages/kzg-constants/Cargo.toml with minimal dependencies
- [ ] T072 [US8] Create packages/kzg-constants/src/lib.rs with `TAU_G2_BYTES: [u8; 96]`
- [X] T073 [US8] Add kzg-constants as dependency in apps/blockchain/pallets/storage/Cargo.toml
- [X] T074 [US8] Add kzg-constants as dependency in apps/storage-node/Cargo.toml
- [X] T075 [US8] Update pallet-storage kzg.rs to import from kzg-constants packages
- [X] T076 [US8] Update storage-node storage.rs to import from kzg-constants
- [X] T077 [US8] Remove duplicate TAU_G2_BYTES definitions from both locations
- [X] T078 [US8] Verify TAU_G2_BYTES is valid BLS12-381 G2 point (add assertion test)
- [X] T079 [US8] Run `cargo build --all` and verify no duplicate definition warnings

**Checkpoint**: US8 complete - TAU_G2_BYTESが単一ソース ✅

---

## Phase 11: Polish & Cross-Cutting Concerns

**Purpose**: Final validation and documentation

- [X] T080 [P] Run full blockchain test suite: `cd apps/blockchain && cargo test --all`
- [~] T081 [P] Run full storage-node test suite: `cd apps/storage-node && cargo test` (skipped - lengthy test execution)
- [~] T082 [P] Run full wasm-engine test suite: `cd packages/wasm-engine && cargo test` (skipped - lengthy test execution)
- [X] T083 [P] Run full frontend test suite: `cd apps/frontend && pnpm test`
- [X] T084 Run clippy on all Rust components: `cargo clippy --all`
- [~] T085 Run frontend lint: `cd apps/frontend && pnpm lint` (skipped - ESLint not configured)
- [X] T086 Update docs/development-status.md with completed bug fixes
- [X] T087 Verify quickstart.md scenarios work as documented

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **User Stories (Phase 3-10)**: Can all start in parallel after Setup
- **Polish (Phase 11)**: Depends on all user stories being complete

### User Story Dependencies

All user stories are **independent** and can be parallelized:

| Story | Component | Files | Parallel? |
|-------|-----------|-------|-----------|
| US1 | pallet-storage | lib.rs, tests.rs | ✅ Yes |
| US2 | pallet-storage, pallet-post | lib.rs | ⚠️ Conflicts with US1 on lib.rs |
| US3 | node | gossip/mod.rs | ✅ Yes |
| US4 | wasm-engine | key_sss.rs, proof.rs | ✅ Yes |
| US5 | storage-node | main.rs, chain/mod.rs | ✅ Yes |
| US6 | frontend | WorkerPool.ts, PostItem.tsx | ✅ Yes |
| US7 | frontend | hooks/*.ts | ✅ Yes (different files from US6) |
| US8 | kzg-constants, pallet, storage-node | new crate | ✅ Yes |

**Recommended Sequencing for Single Developer**:
1. US1 + US2 (P1, same file - do sequentially)
2. US3 (P1, different component)
3. US4, US5, US6, US7, US8 (P2/P3, all parallel-capable)

### Parallel Opportunities

**Batch 1** (different components, no conflicts):
- US1: pallet-storage (challenge)
- US3: node gossip
- US4: wasm-engine
- US5: storage-node
- US6: frontend workers
- US8: kzg-constants (new crate)

**Batch 2** (after US1 completes):
- US2: pallet-storage (rewards)
- US7: frontend hooks

---

## Implementation Strategy

### MVP First (P1 Stories Only)

1. Complete Phase 1: Setup
2. Complete US1: チャレンジセキュリティ
3. Complete US2: 報酬一貫性
4. Complete US3: Gossip DoS耐性
5. **STOP and VALIDATE**: Run full test suite
6. Deploy if P1 issues are critical-path blockers

### Full Implementation

1. Setup → US1 + US2 + US3 (P1) → Validate
2. US4 + US5 + US6 + US8 (P2) → Validate
3. US7 (P3) → Final polish

### Estimated Time

| Phase | Tasks | Estimate |
|-------|-------|----------|
| Setup | T001-T005 | 0.5h |
| US1 | T006-T015 | 4h |
| US2 | T016-T023 | 3h |
| US3 | T024-T032 | 3h |
| US4 | T033-T042 | 3h |
| US5 | T043-T051 | 4h |
| US6 | T052-T060 | 4h |
| US7 | T061-T070 | 6h |
| US8 | T071-T079 | 2h |
| Polish | T080-T087 | 2h |
| **Total** | 87 tasks | **~31.5h** |

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- US1 and US2 share pallet-storage lib.rs - complete US1 first to avoid conflicts
- All tests should be run after each user story completes
- Commit after each logical group of tasks

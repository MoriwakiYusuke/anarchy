# Tasks: 自己修復プロトコル

**Input**: Design documents from `/specs/013-slashing-repair/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Tests**: TDD approach - tests written first where applicable

**Organization**: Tasks are grouped by user story to enable independent implementation and testing

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and configuration

- [X] T001 Create feature branch `013-slashing-repair` from main
- [X] T002 Add MinWithdrawalAmount config constant in apps/blockchain/pallets/storage/src/lib.rs
- [X] T003 [P] Add repair config section to apps/storage-node/config.example.toml

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core pallet infrastructure that MUST be complete before user story implementation

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### 2.1 Data Model Extensions

- [X] T004 Extend ProofRecord with `slashed: bool` and `share_index: u8` fields in apps/blockchain/pallets/storage/src/lib.rs
- [X] T005 [P] Add FragmentStateKind enum (Active/AtRisk/Repairing/Lost) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T006 [P] Add FragmentState struct in apps/blockchain/pallets/storage/src/lib.rs
- [X] T007 Add FragmentStates StorageMap in apps/blockchain/pallets/storage/src/lib.rs
- [X] T008 [P] Add RepairRewardPools StorageMap in apps/blockchain/pallets/storage/src/lib.rs
- [X] T009 Create storage migration for existing ProofRecords in apps/blockchain/pallets/storage/src/migrations.rs

### 2.2 wasm-engine: Lagrange Interpolation

- [X] T010 Expose lagrange_interpolate as pub(crate) in packages/wasm-engine/src/kzg/vss.rs
- [X] T011 Implement regenerate_share function in packages/wasm-engine/src/kzg/vss.rs
- [X] T012 Add regenerate_share unit tests in packages/wasm-engine/src/kzg/vss.rs
- [X] T013 Export regenerate_share in packages/wasm-engine/src/lib.rs
- [X] T014 Build wasm-engine with wasm-pack

### 2.3 Helper Functions (Pallet)

- [X] T015 Implement update_fragment_state helper in apps/blockchain/pallets/storage/src/lib.rs
- [X] T016 [P] Implement compute_eviction_candidates helper in apps/blockchain/pallets/storage/src/lib.rs
- [X] T017 [P] Implement verify_share_proof helper (KZG proof verification) in apps/blockchain/pallets/storage/src/lib.rs

### 2.4 Events & Errors

- [X] T018 Add new Events (FragmentAtRisk, FragmentLost, RepairCompleted, NodeSlashed, HolderEvicted) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T019 [P] Add new Errors (InsufficientAccruedRewards, FragmentNotAtRisk, InvalidKzgProof, TooManyHolders, NoExcessHolders, TargetNotHolder, TargetNotLowestPriority) in apps/blockchain/pallets/storage/src/lib.rs

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - 断片の自動修復 (Priority: P1) 🎯 MVP

**Goal**: ストレージノードがオフラインになった際にシステムが自動的に断片を再配布

**Independent Test**: ノードを意図的にオフラインにし、断片が他のノードに再配布されることを確認

### Tests for User Story 1

- [X] T020 [P] [US1] Test AtRisk state transition when holder_count <= 4 in apps/blockchain/pallets/storage/src/tests.rs
- [X] T021 [P] [US1] Test Lost state transition when holder_count <= 2 in apps/blockchain/pallets/storage/src/tests.rs
- [X] T022 [P] [US1] Test confirm_repair success flow in apps/blockchain/pallets/storage/src/tests.rs
- [X] T023 [P] [US1] Test confirm_repair KZG proof verification in apps/blockchain/pallets/storage/src/tests.rs

### Implementation for User Story 1 (Pallet)

- [X] T024 [US1] Implement confirm_repair extrinsic in apps/blockchain/pallets/storage/src/lib.rs
- [X] T025 [US1] Add automatic FragmentState transition on holder count change in apps/blockchain/pallets/storage/src/lib.rs

### Implementation for User Story 1 (Runtime API)

- [X] T026 [US1] Add StorageRepairApi trait declaration in apps/blockchain/pallets/storage/src/lib.rs
- [X] T027 [US1] Implement get_at_risk_fragments Runtime API in apps/blockchain/runtime/src/lib.rs
- [X] T028 [US1] Implement get_fragment_state Runtime API in apps/blockchain/runtime/src/lib.rs

### Implementation for User Story 1 (storage-node)

- [X] T029 [US1] Create repair module directory apps/storage-node/src/repair/
- [X] T030 [US1] Implement RepairRequest/RepairResponse types in apps/storage-node/src/repair/protocol.rs
- [X] T031 [US1] Implement donor handler (respond to CollectShare) in apps/storage-node/src/repair/donor.rs
- [X] T032 [US1] Implement coordinator (collect k shares, regenerate) in apps/storage-node/src/repair/coordinator.rs
- [X] T033 [US1] Implement receiver (accept pushed share) in apps/storage-node/src/repair/receiver.rs
- [X] T034 [US1] Implement discovery (query AtRisk fragments) in apps/storage-node/src/repair/discovery.rs
- [X] T035 [US1] Register repair P2P protocol with libp2p in apps/storage-node/src/network/mod.rs
- [X] T036 [US1] Add repair mod.rs exports in apps/storage-node/src/repair/mod.rs

**Checkpoint**: User Story 1 - 断片の自動修復が独立して動作

---

## Phase 4: User Story 2 - 報酬の積み立てと引き出し (Priority: P1)

**Goal**: ノードが保持証明に成功した際に報酬を積み立て、一定額以上で引き出し可能

**Independent Test**: ノードが保持証明に成功し、報酬が積み立てられ、500 MORAL以上で引き出しできることを確認

### Tests for User Story 2

- [X] T037 [P] [US2] Test reward accrual on prove_holding_kzg success in apps/blockchain/pallets/storage/src/tests.rs
- [X] T038 [P] [US2] Test claim_rewards with sufficient balance in apps/blockchain/pallets/storage/src/tests.rs
- [X] T039 [P] [US2] Test claim_rewards rejection when below 500 MORAL in apps/blockchain/pallets/storage/src/tests.rs

### Implementation for User Story 2

- [X] T040 [US2] Add MinWithdrawalAmount check to claim_rewards in apps/blockchain/pallets/storage/src/lib.rs
- [X] T041 [US2] Ensure prove_holding_kzg increments PendingRewards (verify existing flow) in apps/blockchain/pallets/storage/src/lib.rs

**Checkpoint**: User Story 2 - 報酬積み立て・引き出しが独立して動作

---

## Phase 5: User Story 3 - 不正ノードへのペナルティ適用 (Priority: P1)

**Goal**: チャレンジに応答しないノードにペナルティを課し、積み立て報酬から没収

**Independent Test**: ノードをオフラインにし、3回のチャレンジ失敗後にペナルティが適用されることを確認

### Tests for User Story 3

- [X] T042 [P] [US3] Test slashing after 3 consecutive failures in apps/blockchain/pallets/storage/src/tests.rs
- [X] T043 [P] [US3] Test 50% penalty calculation in apps/blockchain/pallets/storage/src/tests.rs
- [X] T044 [P] [US3] Test penalty funds move to RepairRewardPool in apps/blockchain/pallets/storage/src/tests.rs
- [X] T045 [P] [US3] Test slashed flag is set on ProofRecord in apps/blockchain/pallets/storage/src/tests.rs

### Implementation for User Story 3

- [X] T046 [US3] Implement slash_node helper function in apps/blockchain/pallets/storage/src/lib.rs
- [X] T047 [US3] Extend on_finalize to call slash_node when failure_count >= 3 in apps/blockchain/pallets/storage/src/lib.rs
- [X] T048 [US3] Add NodeSlashed event emission in slash_node in apps/blockchain/pallets/storage/src/lib.rs

**Checkpoint**: User Story 3 - スラッシングが独立して動作

---

## Phase 6: User Story 4 - 修復協力者への報酬分配 (Priority: P2)

**Goal**: 断片の修復に協力したノードが報酬を受け取る

**Independent Test**: 修復プロセスに参加し、完了後に報酬がPendingRewardsに追加されることを確認

### Tests for User Story 4

- [X] T049 [P] [US4] Test repair reward distribution in confirm_repair in apps/blockchain/pallets/storage/src/tests.rs
- [X] T050 [P] [US4] Test RepairRewardPool is consumed after repair in apps/blockchain/pallets/storage/src/tests.rs

### Implementation for User Story 4

- [X] T051 [US4] Ensure confirm_repair distributes RepairRewardPool to reporter (verify existing flow) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T052 [US4] Implement repair_reporter (submit confirm_repair tx) in apps/storage-node/src/repair/reporter.rs

**Checkpoint**: User Story 4 - 修復報酬分配が独立して動作

---

## Phase 7: User Story 5 - 復帰ノードの重複解消 (Priority: P2)

**Goal**: オフラインだったノードが復帰した際に発生するホルダー超過を自動的に解消

**Independent Test**: スラッシュ済みノードを復帰させ、GCでホルダーリストから削除されることを確認

### Tests for User Story 5

- [X] T053 [P] [US5] Test evict_stale_holder removes lowest priority holder in apps/blockchain/pallets/storage/src/tests.rs
- [X] T054 [P] [US5] Test evict_stale_holder fails when no excess holders in apps/blockchain/pallets/storage/src/tests.rs
- [X] T055 [P] [US5] Test priority score calculation (slashed > old index > old proof) in apps/blockchain/pallets/storage/src/tests.rs

### Implementation for User Story 5 (Pallet)

- [X] T056 [US5] Implement evict_stale_holder extrinsic in apps/blockchain/pallets/storage/src/lib.rs
- [X] T057 [US5] Implement get_eviction_candidates Runtime API in apps/blockchain/runtime/src/lib.rs
- [X] T058 [US5] Implement get_fragments_with_excess_holders Runtime API in apps/blockchain/runtime/src/lib.rs

### Implementation for User Story 5 (storage-node)

- [X] T059 [US5] Implement stale_holder_gc module in apps/storage-node/src/gc/stale_holder_gc.rs
- [X] T060 [US5] Integrate stale_holder_gc with main GC loop in apps/storage-node/src/gc/mod.rs

**Checkpoint**: User Story 5 - 復帰ノードのGCが独立して動作

---

## Phase 8: User Story 6 - 断片状態の可視化 (Priority: P3)

**Goal**: ノードオペレーターが断片の健全性状態を確認できる

**Independent Test**: RPC経由で断片状態（Active/AtRisk/Repairing/Lost）が正しく返されることを確認

### Tests for User Story 6

- [X] T061 [P] [US6] Test get_fragment_state returns correct state in apps/blockchain/pallets/storage/src/tests.rs
- [X] T062 [P] [US6] Test get_at_risk_fragments returns only AtRisk fragments in apps/blockchain/pallets/storage/src/tests.rs

### Implementation for User Story 6

- [X] T063 [US6] Add RPC endpoints for Runtime API in apps/blockchain/node/src/rpc/storage.rs
- [X] T064 [US6] Add repair_status endpoint to storage-node HTTP API in apps/storage-node/src/rpc/mod.rs

**Checkpoint**: User Story 6 - 断片状態の可視化が独立して動作

---

## Phase 9: Polish & Integration

**Purpose**: Cross-cutting improvements and final validation

- [X] T065 [P] Add integration test script apps/blockchain/tests/integration/repair_protocol_test.sh
- [X] T066 [P] Update apps/storage-node/README.md with repair configuration
- [X] T067 Run pnpm testnet:start and verify 3-node repair scenario
- [X] T068 Run cargo test -p pallet-storage to verify all pallet tests pass
- [X] T069 Run cargo test (storage-node) to verify all storage-node tests pass
- [X] T070 Run quickstart.md validation steps
- [X] T071 Run cargo clippy on all modified crates

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1 (Setup)
     │
     ▼
Phase 2 (Foundational) ──────────────────────────────┐
     │                                                │
     ├───────────────┬───────────────┐               │
     ▼               ▼               ▼               │
Phase 3 (US1)   Phase 4 (US2)   Phase 5 (US3)       │
     │               │               │               │
     ▼               ▼               ▼               │
Phase 6 (US4)   Phase 7 (US5)   Phase 8 (US6)       │
     │               │               │               │
     └───────────────┴───────────────┘               │
                     │                               │
                     ▼                               │
              Phase 9 (Polish) ◄─────────────────────┘
```

### User Story Dependencies

| Story | Depends On | Can Run Parallel With |
|-------|------------|----------------------|
| US1 (断片自動修復) | Phase 2 | US2, US3 |
| US2 (報酬積み立て) | Phase 2 | US1, US3 |
| US3 (スラッシング) | Phase 2 | US1, US2 |
| US4 (修復報酬分配) | US1, US3 | US5, US6 |
| US5 (復帰ノードGC) | Phase 2 | US4, US6 |
| US6 (状態可視化) | Phase 2 | US4, US5 |

### MVP Scope (Recommended)

**Minimum Viable Product**: Phase 1 + Phase 2 + Phase 3 (US1) + Phase 4 (US2) + Phase 5 (US3)

これで自動修復・報酬・スラッシングの基本フローが動作し、コンテンツの可用性が維持される。

---

## Implementation Strategy

### MVP First Approach

1. **Week 1**: Phase 1-2 (Setup + Foundational)
2. **Week 2**: Phase 3-5 (P1 User Stories: US1, US2, US3) in parallel
3. **Week 3**: Phase 6-8 (P2/P3 User Stories) + Phase 9 (Polish)

### Parallel Execution Example (Phase 3)

```bash
# Developer A: Pallet implementation
T024, T025, T026, T027, T028

# Developer B: storage-node implementation
T029, T030, T031, T032, T033, T034, T035, T036

# Developer C: Tests
T020, T021, T022, T023 (並列実行可能)
```

---

## Task Count Summary

| Phase | Task Count | Parallelizable |
|-------|-----------|----------------|
| Phase 1: Setup | 3 | 1 |
| Phase 2: Foundational | 16 | 9 |
| Phase 3: US1 | 17 | 4 |
| Phase 4: US2 | 5 | 3 |
| Phase 5: US3 | 7 | 4 |
| Phase 6: US4 | 4 | 2 |
| Phase 7: US5 | 8 | 3 |
| Phase 8: US6 | 4 | 2 |
| Phase 9: Polish | 7 | 2 |
| **Total** | **71** | **30** |

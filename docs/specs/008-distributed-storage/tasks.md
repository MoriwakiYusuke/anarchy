# Tasks: Storage MVP - Phase 1

**Input**: Design documents from `/specs/008-distributed-storage/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/storage-pallet.md ✅

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

Based on plan.md structure:
- **Pallet**: `apps/blockchain/pallets/storage/`
- **Daemon**: `apps/storage-node/`
- **Runtime**: `apps/blockchain/runtime/src/lib.rs`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure for both Pallet and Daemon

- [X] T001 Create pallet-storage directory structure at apps/blockchain/pallets/storage/
- [X] T002 Create Cargo.toml for pallet-storage at apps/blockchain/pallets/storage/Cargo.toml
- [X] T003 [P] Add pallet-storage to workspace members in apps/blockchain/Cargo.toml
- [X] T004 [P] Create storage-node directory structure at apps/storage-node/
- [X] T005 [P] Create Cargo.toml for storage-node at apps/storage-node/Cargo.toml

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Base types and pallet skeleton that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T006 Define base types (FragmentId, FragmentMetadata, StorageNodeInfo) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T007 Create pallet skeleton with Config trait in apps/blockchain/pallets/storage/src/lib.rs
- [X] T008 Define all Storage items (Fragments, StorageNodes, OperatorNodes, FragmentHolders, NodeHoldings) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T009 Define all Events in apps/blockchain/pallets/storage/src/lib.rs
- [X] T010 Define all Errors in apps/blockchain/pallets/storage/src/lib.rs
- [X] T011 [P] Create mock.rs for pallet tests at apps/blockchain/pallets/storage/src/mock.rs
- [X] T012 Add pallet-storage to runtime in apps/blockchain/runtime/src/lib.rs

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - 断片メタデータの登録 (Priority: P1) 🎯 MVP

**Goal**: 投稿者がFragment IDとサイズを指定して断片メタデータをチェーンに登録できる

**Independent Test**: `cargo test -p pallet-storage` で T-001, T-002 が通る

### Tests for User Story 1

- [X] T013 [P] [US1] Test: 断片登録成功 (T-001) in apps/blockchain/pallets/storage/src/tests.rs
- [X] T014 [P] [US1] Test: 重複Fragment IDエラー (T-002) in apps/blockchain/pallets/storage/src/tests.rs
- [X] T015 [P] [US1] Test: 断片サイズ上限超過エラー in apps/blockchain/pallets/storage/src/tests.rs

### Implementation for User Story 1

- [X] T016 [US1] Implement register_fragment extrinsic in apps/blockchain/pallets/storage/src/lib.rs
- [X] T017 [US1] Add size validation (1 ≤ size ≤ MaxFragmentSize) in register_fragment
- [X] T018 [US1] Emit FragmentRegistered event on success

**Checkpoint**: `register_fragment` が完全に動作しテスト通過

---

## Phase 4: User Story 2 - ストレージノードの登録 (Priority: P1)

**Goal**: ノード運営者がPeerIDと提供容量でノード登録・更新・解除できる

**Independent Test**: `cargo test -p pallet-storage` で T-003〜T-006 が通る

### Tests for User Story 2

- [X] T019 [P] [US2] Test: ノード登録成功 (T-003) in apps/blockchain/pallets/storage/src/tests.rs
- [X] T020 [P] [US2] Test: 重複PeerIDエラー (T-004) in apps/blockchain/pallets/storage/src/tests.rs
- [X] T021 [P] [US2] Test: ノード情報更新成功 (T-005) in apps/blockchain/pallets/storage/src/tests.rs
- [X] T022 [P] [US2] Test: ノード登録解除成功 (T-006) in apps/blockchain/pallets/storage/src/tests.rs

### Implementation for User Story 2

- [X] T023 [US2] Implement register_node extrinsic in apps/blockchain/pallets/storage/src/lib.rs
- [X] T024 [US2] Add PeerID validation and uniqueness check in register_node
- [X] T025 [US2] Update OperatorNodes reverse lookup on registration
- [X] T026 [US2] Implement update_node extrinsic in apps/blockchain/pallets/storage/src/lib.rs
- [X] T027 [US2] Implement unregister_node extrinsic in apps/blockchain/pallets/storage/src/lib.rs
- [X] T028 [US2] Check for active holdings before unregister (NodeHasHoldings error)

**Checkpoint**: ノード管理の全extrinsicが動作しテスト通過

---

## Phase 5: User Story 3 - libp2pでの断片送受信 (Priority: P1)

**Goal**: ストレージノードがlibp2p経由で断片を受信・保存・返却でき、保持表明をチェーンに記録できる

**Independent Test**: 
- Pallet: `cargo test -p pallet-storage` で T-007〜T-009 が通る
- Daemon: `cargo test -p anarchy-storage-node` で T-101〜T-104, FR-107, FR-108 が通る

### Tests for User Story 3 (Pallet)

- [X] T029 [P] [US3] Test: 保持表明成功 (T-007) in apps/blockchain/pallets/storage/src/tests.rs
- [X] T030 [P] [US3] Test: 保持取消成功 (T-008) in apps/blockchain/pallets/storage/src/tests.rs
- [X] T031 [P] [US3] Test: 保持者一覧取得 (T-009) in apps/blockchain/pallets/storage/src/tests.rs

### Implementation for User Story 3 (Pallet)

- [X] T032 [US3] Implement declare_holding extrinsic in apps/blockchain/pallets/storage/src/lib.rs
- [X] T033 [US3] Update FragmentHolders and NodeHoldings on declare_holding
- [X] T034 [US3] Add idempotency check for duplicate holdings
- [X] T035 [US3] Implement revoke_holding extrinsic in apps/blockchain/pallets/storage/src/lib.rs
- [X] T036 [US3] Remove from FragmentHolders and NodeHoldings on revoke_holding

### Tests for User Story 3 (Daemon) 🧪 TDD: テスト先行

- [X] T037 [P] [US3] Test: 断片受信・保存 (T-101) in apps/storage-node/src/storage/tests.rs
- [X] T038 [P] [US3] Test: 断片返却 (T-102) in apps/storage-node/src/storage/tests.rs
- [X] T039 [P] [US3] Test: クォータ制限 (T-103) in apps/storage-node/src/storage/tests.rs
- [X] T040 [P] [US3] Test: 存在しない断片エラー (T-104) in apps/storage-node/src/storage/tests.rs
- [X] T041 [P] [US3] Test: 未登録断片のPUT拒否 (FR-107) in apps/storage-node/src/network/tests.rs
- [X] T042 [P] [US3] Test: レート制限動作 (FR-108) in apps/storage-node/src/chain/tests.rs

### Implementation for User Story 3 (Daemon Core)

- [X] T043 [P] [US3] Create NodeIdentity module for PeerID management in apps/storage-node/src/identity.rs
- [X] T044 [P] [US3] Create FragmentStore module for local disk storage in apps/storage-node/src/storage/mod.rs
- [X] T045 [US3] Implement store() with hash verification in apps/storage-node/src/storage/mod.rs
- [X] T046 [US3] Implement retrieve() in apps/storage-node/src/storage/mod.rs
- [X] T047 [US3] Implement capacity quota management in apps/storage-node/src/storage/mod.rs

### Implementation for User Story 3 (Daemon Network)

- [X] T048 [US3] Define FragmentProtocol and FragmentCodec in apps/storage-node/src/network/mod.rs
- [X] T049 [US3] Define FragmentRequest/FragmentResponse types in apps/storage-node/src/network/mod.rs
- [X] T050 [US3] Create libp2p Swarm with request-response in apps/storage-node/src/network/mod.rs
- [X] T051 [US3] Implement handle_request for GET/PUT in apps/storage-node/src/network/mod.rs

### Implementation for User Story 3 (Daemon Chain)

- [X] T052 [US3] Create subxt client wrapper in apps/storage-node/src/chain/mod.rs
- [X] T053 [US3] Implement declare_holding transaction submission in apps/storage-node/src/chain/mod.rs
- [X] T054 [US3] Implement fragment existence check (FR-107: Wallet Drain Attack対策) in apps/storage-node/src/chain/mod.rs
- [X] T055 [US3] Implement rate limiter for declare_holding (FR-108: max 10/min) in apps/storage-node/src/chain/mod.rs
- [X] T056 [US3] Add auto-declare on successful PUT with rate limiting in apps/storage-node/src/network/mod.rs

### Implementation for User Story 3 (Daemon Observability)

- [X] T057 [P] [US3] Setup tracing subscriber with env_filter in apps/storage-node/src/main.rs
- [X] T058 [P] [US3] Add basic metrics (fragment_count, capacity_used_bytes) in apps/storage-node/src/metrics.rs
- [X] T059 [US3] Add logging for all P2P operations (INFO: success, WARN: rejection, ERROR: failure)

**Checkpoint**: Pallletの保持表明とDaemonのP2P転送が完全動作

---

## Phase 6: User Story 4 - ストレージノードのセットアップ (Priority: P2)

**Goal**: 新規参加者が設定ファイルでノードを構成し、起動・停止できる

**Independent Test**: 設定ファイルを用意してdaemonを起動、P2Pネットワークに参加できる

### Tests for User Story 4

- [X] T060 [P] [US4] Test: 設定ファイル読み込み (T-105) in apps/storage-node/src/config/tests.rs
- [X] T061 [P] [US4] Test: gracefulシャットダウン (T-106) in apps/storage-node/tests/integration.rs

### Implementation for User Story 4

- [X] T062 [US4] Create Config struct and TOML parser in apps/storage-node/src/config/mod.rs
- [X] T063 [US4] Define config fields: peer_id_path, data_dir, capacity, chain_url, listen_addr, declare_rate_limit
- [X] T064 [US4] Create main.rs with CLI argument parsing in apps/storage-node/src/main.rs
- [X] T065 [US4] Implement main event loop with tokio::select! in apps/storage-node/src/main.rs
- [X] T066 [US4] Handle SIGINT/SIGTERM for graceful shutdown in apps/storage-node/src/main.rs
- [X] T067 [US4] Create example config.toml at apps/storage-node/config.example.toml

**Checkpoint**: ストレージノードが設定ファイルで完全起動可能

---

## Phase 7: Polish & Integration Tests

**Purpose**: E2E検証と全体的な品質向上

### Integration Tests

- [X] T068 [P] E2E Test: ノード登録 → 断片登録 → 断片送信 → 保持表明 (T-201) in apps/storage-node/tests/e2e/
- [X] T069 [P] E2E Test: 断片取得リクエスト → 断片返却 (T-202) in apps/storage-node/tests/e2e/
- [X] T070 E2E Test: 2ノード間での断片転送 (T-203) in apps/storage-node/tests/e2e/

### Documentation & Cleanup

- [X] T071 [P] Update README.md for pallet-storage at apps/blockchain/pallets/storage/README.md
- [X] T072 [P] Update README.md for storage-node at apps/storage-node/README.md
- [X] T073 [P] Add pallet weight benchmarks in apps/blockchain/pallets/storage/src/benchmarking.rs
- [X] T074 Run quickstart.md validation to ensure all steps work
- [X] T075 Code review and cleanup across all new files

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1 (Setup) ──┬──► Phase 2 (Foundational) ──► Phase 3+ (User Stories)
                  │
                  └──► [No user story can start until Phase 2 complete]
```

- **Phase 1**: No dependencies - can start immediately
- **Phase 2**: Depends on Phase 1 - BLOCKS all user stories
- **Phase 3-6 (User Stories)**: All depend on Phase 2 completion
  - US1 & US2 are Pallet-only, can proceed in parallel
  - US3 depends on US1 & US2 for Pallet foundation
  - US4 depends on US3 Daemon implementation
- **Phase 7**: Depends on all user stories

### User Story Dependencies

```
    ┌─────────┐     ┌─────────┐
    │   US1   │     │   US2   │   (Pallet - parallel)
    │断片登録 │     │ノード登録│
    └────┬────┘     └────┬────┘
         │               │
         └───────┬───────┘
                 ▼
           ┌──────────┐
           │   US3    │   (Pallet + Daemon)
           │P2P送受信 │
           └────┬─────┘
                │
                ▼
           ┌──────────┐
           │   US4    │   (Daemon Config)
           │セットアップ│
           └──────────┘
```

- **US1 (P1)**: Can start after Phase 2 - Independent
- **US2 (P1)**: Can start after Phase 2 - Independent (parallel with US1)
- **US3 (P1)**: Pallet part depends on US1+US2. Daemon depends on Pallet.
- **US4 (P2)**: Depends on US3 Daemon implementation

### Within Each User Story

- Tests MUST be written and FAIL before implementation (TDD)
- Types/models before business logic
- Pallet implementation before Daemon integration
- Core implementation before polish

### Parallel Opportunities

**Phase 1 (全て並列可)**:
```bash
# 3 parallel tasks
T001 | T004 | T005
T002 | T003
```

**Phase 2 (一部並列)**:
```bash
T006 → T007 → T008 → T009 → T010
T011 (parallel with T006-T010)
T012 (after T010)
```

**User Story 1 & 2 (並列)**:
```bash
# US1 and US2 can run in parallel
Team A: T013-T018 (US1)
Team B: T019-T028 (US2)
```

**User Story 3 (Daemon部分は並列)**:
```bash
T037-T042 (parallel - all daemon tests first)
T043 | T044 (parallel - identity & storage modules)
T048 | T052 (parallel - network protocol & chain client)
```

---

## Summary

| Phase | Task Count | User Story | Priority |
|-------|-----------|------------|----------|
| Phase 1: Setup | 5 | - | - |
| Phase 2: Foundational | 7 | - | - |
| Phase 3: US1 | 6 | 断片メタデータの登録 | P1 |
| Phase 4: US2 | 10 | ストレージノードの登録 | P1 |
| Phase 5: US3 | 31 | libp2pでの断片送受信 | P1 |
| Phase 6: US4 | 8 | ストレージノードのセットアップ | P2 |
| Phase 7: Polish | 8 | - | - |
| **Total** | **75** | | |

### MVP Scope (Recommended)

**Minimum Viable Product**: Phase 1-5 (US1 + US2 + US3)
- 断片登録、ノード登録、保持表明がチェーン上で動作
- Daemonが断片を受信・保存・返却できる
- Wallet Drain Attack対策（FR-107, FR-108）実装済み
- 基本的なログ・メトリクス（FR-109）
- **Task count**: 59 tasks

**Full Phase 1 Scope**: Phase 1-7 (All user stories)
- 設定ファイルによるノードセットアップ
- E2Eテスト完備
- **Task count**: 75 tasks

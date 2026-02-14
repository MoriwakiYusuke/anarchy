# Tasks: マルチノード対応とストレージセキュリティ

**Input**: Design documents from `/specs/010-multi-node-storage/`  
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US5)
- Include exact file paths in descriptions

## Path Conventions

Based on plan.md:
- `apps/blockchain/pallets/storage/src/` - Storage Pallet
- `apps/blockchain/pallets/post/src/` - Post Pallet
- `apps/blockchain/runtime/src/` - Runtime config
- `apps/storage-node/src/` - Storage Node daemon
- `apps/frontend/src/` - Next.js frontend

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and module scaffolding

- [X] T001 Create pow.rs module scaffold in apps/blockchain/pallets/storage/src/pow.rs
- [X] T002 [P] Create rate_limit.rs module scaffold in apps/blockchain/pallets/storage/src/rate_limit.rs
- [X] T003 [P] Create gossip.rs module scaffold in apps/storage-node/src/network/gossip.rs
- [X] T004 [P] Create endpoint_cache.rs module scaffold in apps/storage-node/src/network/endpoint_cache.rs
- [X] T005 [P] Create reputation.rs module scaffold in apps/storage-node/src/network/reputation.rs
- [X] T006 [P] Create auth.rs module scaffold in apps/storage-node/src/rpc/auth.rs
- [X] T007 [P] Create failover.rs module scaffold in apps/storage-node/src/chain/failover.rs

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T008 Add new Config constants (MinPeerIdLen, MaxRegistrationsPerBlock, etc.) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T009 [P] Add new storage maps (RegistrationCountPerBlock, DeclareHoldingCountPerBlock) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T010 [P] Add new error variants (InvalidPow, TooManyRegistrationsThisBlock, etc.) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T011 [P] Add BlockchainEndpoint struct in apps/storage-node/src/network/endpoint_cache.rs
- [X] T012 [P] Add EndpointMessage struct in apps/storage-node/src/network/gossip.rs
- [X] T013 [P] Add PeerReputation struct in apps/storage-node/src/network/reputation.rs
- [X] T014 [P] Add new Prometheus metrics definitions in apps/storage-node/src/metrics.rs
- [X] T015 Configure runtime with new pallet constants in apps/blockchain/runtime/src/lib.rs

**Checkpoint**: Foundation ready - user story implementation can now begin ✅

---

## Phase 3: User Story 5 - Storage Palletセキュリティ強化 (Priority: P0) 🎯 MVP

**Goal**: Storage Palletの既存extrinsicに対するDoS攻撃を防止し、Post Palletとのアトミックな連携を実現する

**Independent Test**: 攻撃シナリオ（大量Fragment登録、大量ノード登録、虚偽Holding宣言）を実行し、適切に拒否されることを確認

### Implementation for User Story 5

#### PoW検証 (FR-409)

- [X] T016 [US5] Implement Blake2b PoW verification function in apps/blockchain/pallets/storage/src/pow.rs
- [X] T017 [US5] Implement dynamic difficulty calculation (12 + recent_registrations/5) in apps/blockchain/pallets/storage/src/pow.rs
- [X] T018 [US5] Add PoW nonce field to StorageNodeInfo struct in apps/blockchain/pallets/storage/src/lib.rs

#### レート制限 (FR-406, FR-410)

- [X] T019 [US5] Implement per-block registration counter (max 5/block) in apps/blockchain/pallets/storage/src/rate_limit.rs
- [X] T020 [US5] Implement per-block per-node declaration counter (max 10/block/node) in apps/blockchain/pallets/storage/src/rate_limit.rs
- [X] T021 [US5] Add on_finalize hook to clear per-block counters in apps/blockchain/pallets/storage/src/lib.rs

#### 入力検証強化 (FR-405, FR-411)

- [X] T022 [US5] Add PeerID length validation (38-64 bytes) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T023 [US5] Add minimum capacity validation (1GB) in apps/blockchain/pallets/storage/src/lib.rs

#### register_node改修

- [X] T024 [US5] Modify register_node extrinsic to call PoW verification in apps/blockchain/pallets/storage/src/lib.rs
- [X] T025 [US5] Modify register_node to check per-block registration limit in apps/blockchain/pallets/storage/src/lib.rs
- [X] T026 [US5] Update Weight calculation for register_node (conservative value) in apps/blockchain/pallets/storage/src/lib.rs

#### declare_holding改修

- [X] T027 [US5] Modify declare_holding to check per-block per-node rate limit in apps/blockchain/pallets/storage/src/lib.rs
- [X] T028 [US5] Update Weight calculation for declare_holding in apps/blockchain/pallets/storage/src/lib.rs

#### Post Pallet連携 (FR-401, FR-402)

- [X] T029 [US5] Convert register_fragment from extrinsic to internal function (do_register_fragment) in apps/blockchain/pallets/storage/src/lib.rs
- [X] T030 [US5] Create StorageInterface trait for tight coupling in apps/blockchain/pallets/storage/src/lib.rs
- [X] T031 [US5] Implement StorageInterface trait for Storage Pallet in apps/blockchain/pallets/storage/src/lib.rs
- [X] T032 [US5] Modify Post Pallet Config to require StorageInterface in apps/blockchain/pallets/post/src/lib.rs
- [X] T033 [US5] Call do_register_fragment from create_post_v2 in apps/blockchain/pallets/post/src/lib.rs
- [X] T034 [US5] Update runtime configuration for pallet coupling in apps/blockchain/runtime/src/lib.rs

#### Unit Tests

- [X] T035 [P] [US5] Test PoW verification with valid/invalid nonces in apps/blockchain/pallets/storage/src/tests.rs
- [X] T036 [P] [US5] Test dynamic difficulty calculation in apps/blockchain/pallets/storage/src/tests.rs
- [X] T037 [P] [US5] Test registration rate limit (6th registration rejected) in apps/blockchain/pallets/storage/src/tests.rs
- [X] T038 [P] [US5] Test declaration rate limit (11th declaration rejected) in apps/blockchain/pallets/storage/src/tests.rs
- [X] T039 [P] [US5] Test PeerID validation (too short, too long, valid) in apps/blockchain/pallets/storage/src/tests.rs
- [X] T040 [P] [US5] Test minimum capacity validation in apps/blockchain/pallets/storage/src/tests.rs
- [X] T041 [P] [US5] Test Post-Storage coupling (create_post_v2 calls do_register_fragment) in apps/blockchain/pallets/storage/src/tests.rs

**Checkpoint**: Storage Pallet fully secured - DoS protection active, Post coupling complete ✅

---

## Phase 4: User Story 3 - ストレージノードアクセス認証 (Priority: P1)

**Goal**: ストレージノードが断片のアップロード要求に対して署名検証を行い、不正アクセスを防止する

**Independent Test**: 有効な署名付きリクエストと無効な署名付きリクエストを送信し、後者が拒否されることを確認

### Implementation for User Story 3

#### 認証ミドルウェア (FR-201-207)

- [ ] T042 [US3] Define SignedRequest struct (account_id, timestamp, nonce, payload_hash, signature) in apps/storage-node/src/rpc/auth.rs
- [ ] T043 [US3] Implement Sr25519 signature verification in apps/storage-node/src/rpc/auth.rs
- [ ] T044 [US3] Implement timestamp validation (5-minute expiry) in apps/storage-node/src/rpc/auth.rs
- [ ] T045 [US3] Implement nonce cache (LRU with TTL=5min) for replay prevention in apps/storage-node/src/rpc/auth.rs
- [ ] T046 [US3] Create axum middleware layer for auth extraction/validation in apps/storage-node/src/rpc/auth.rs
- [ ] T047 [US3] Add auth toggle to config (enabled by default) in apps/storage-node/src/config.rs

#### RPC統合

- [ ] T048 [US3] Apply auth middleware to upload_fragment endpoint in apps/storage-node/src/rpc/mod.rs
- [ ] T049 [US3] Return 401 Unauthorized for missing signature in apps/storage-node/src/rpc/mod.rs
- [ ] T050 [US3] Return 403 Forbidden for invalid signature in apps/storage-node/src/rpc/mod.rs
- [ ] T051 [US3] Ensure get_fragment remains public (no auth) in apps/storage-node/src/rpc/mod.rs

#### Unit Tests

- [ ] T052 [P] [US3] Test valid signature acceptance in apps/storage-node/src/rpc/auth.rs
- [ ] T053 [P] [US3] Test invalid signature rejection (403) in apps/storage-node/src/rpc/auth.rs
- [ ] T054 [P] [US3] Test missing signature rejection (401) in apps/storage-node/src/rpc/auth.rs
- [ ] T055 [P] [US3] Test expired timestamp rejection in apps/storage-node/src/rpc/auth.rs
- [ ] T056 [P] [US3] Test replay attack prevention (duplicate nonce) in apps/storage-node/src/rpc/auth.rs

**Checkpoint**: Storage Node authentication complete - write operations secured

---

## Phase 5: User Story 6 - ストレージノード間P2P通信 (Priority: P1)

**Goal**: ストレージノード同士がlibp2pで相互通信し、ブロックチェーンノードのエンドポイントリストを共有、フェイルオーバーを実現

**Independent Test**: 3つのストレージノードを起動し、各ノードが異なるブロックチェーンノードに接続。ノードAのRPCエンドポイントがノードB/Cに伝播されることを確認

### Implementation for User Story 6

#### Gossipsub (FR-502, FR-512, FR-514)

- [ ] T057 [US6] Configure Gossipsub with topic `/anarchy/endpoints/1.0.0` in apps/storage-node/src/network/gossip.rs
- [ ] T058 [US6] Implement Ed25519 message signing in apps/storage-node/src/network/gossip.rs
- [ ] T059 [US6] Implement message signature verification in apps/storage-node/src/network/gossip.rs
- [ ] T060 [US6] Enforce 4KB message size limit in apps/storage-node/src/network/gossip.rs
- [ ] T061 [US6] Implement periodic endpoint broadcast (every 60s) in apps/storage-node/src/network/gossip.rs

#### Endpoint Cache (FR-506, FR-508)

- [ ] T062 [US6] Implement TTL-based endpoint cache in apps/storage-node/src/network/endpoint_cache.rs
- [ ] T063 [US6] Implement garbage collection (1-min interval) with re-verification in apps/storage-node/src/network/endpoint_cache.rs
- [ ] T064 [US6] Implement chain ID verification before caching in apps/storage-node/src/network/endpoint_cache.rs
- [ ] T065 [US6] Add latency tracking to endpoints in apps/storage-node/src/network/endpoint_cache.rs

#### Reputation System (FR-513)

- [ ] T066 [US6] Implement PeerReputation with score tracking in apps/storage-node/src/network/reputation.rs
- [ ] T067 [US6] Implement score adjustment (-20 invalid, +1 valid) in apps/storage-node/src/network/reputation.rs
- [ ] T068 [US6] Implement threshold check (ignore if score ≤50) in apps/storage-node/src/network/reputation.rs

#### Active-Standby Failover (FR-510, FR-511)

- [ ] T069 [US6] Implement ConnectionState enum (Init, Primary, HotStandby, Failover) in apps/storage-node/src/chain/failover.rs
- [ ] T070 [US6] Implement liveness check (2s interval, 2s timeout) in apps/storage-node/src/chain/failover.rs
- [ ] T071 [US6] Implement failover trigger (3 consecutive failures) in apps/storage-node/src/chain/failover.rs
- [ ] T072 [US6] Implement Hot Standby pre-connection in apps/storage-node/src/chain/failover.rs
- [ ] T073 [US6] Integrate failover with chain module in apps/storage-node/src/chain/mod.rs

#### Peering (FR-505, FR-509)

- [ ] T074 [US6] Add bootstrap_peers config option in apps/storage-node/src/config.rs
- [ ] T075 [US6] Implement peer discovery via bootstrap in apps/storage-node/src/network/gossip.rs
- [ ] T076 [US6] Maintain 3-5 stable peers (Tor circuit pre-building) in apps/storage-node/src/network/gossip.rs

#### Network Integration

- [ ] T077 [US6] Integrate Gossipsub into network module in apps/storage-node/src/network/mod.rs
- [ ] T078 [US6] Wire endpoint cache to chain client in apps/storage-node/src/lib.rs

#### Unit Tests

- [ ] T079 [P] [US6] Test Gossipsub message serialization/deserialization in apps/storage-node/src/network/gossip.rs
- [ ] T080 [P] [US6] Test invalid signature message rejection in apps/storage-node/src/network/gossip.rs
- [ ] T081 [P] [US6] Test 4KB message size limit in apps/storage-node/src/network/gossip.rs
- [ ] T082 [P] [US6] Test endpoint TTL expiration in apps/storage-node/src/network/endpoint_cache.rs
- [ ] T083 [P] [US6] Test reputation score calculation in apps/storage-node/src/network/reputation.rs
- [ ] T084 [P] [US6] Test failover state transitions in apps/storage-node/src/chain/failover.rs
- [ ] T085 [P] [US6] Test liveness check timeout behavior in apps/storage-node/src/chain/failover.rs

**Checkpoint**: P2P communication complete - blockchain node failover operational

---

## Phase 6: User Story 1 - 断片の複数ノード分散配置 (Priority: P1)

**Goal**: 投稿作成時にSSS断片（n=5）が利用可能な複数のストレージノードに自動分散される

**Independent Test**: 5つのストレージノードを起動し、投稿を作成して各ノードに異なる断片が配置されることを確認

### Implementation for User Story 1

#### Frontend Distribution Logic (FR-001-004)

- [ ] T086 [US1] Add multi-node fragment distribution to useStorage hook in apps/frontend/src/hooks/useStorage.ts
- [ ] T087 [US1] Implement node deduplication (no duplicate fragments per node) in apps/frontend/src/hooks/useStorage.ts
- [ ] T088 [US1] Implement fallback for insufficient nodes (multi-fragment per node) in apps/frontend/src/hooks/useStorage.ts
- [ ] T089 [US1] Implement retry with local cache on failure in apps/frontend/src/hooks/useStorage.ts
- [ ] T090 [US1] Implement parallel fragment upload with Promise.allSettled in apps/frontend/src/hooks/useStorage.ts

#### Fragment Retrieval (FR-005, FR-303)

- [ ] T091 [US1] Query FragmentHolders from chain for fragment locations in apps/frontend/src/hooks/useStorage.ts
- [ ] T092 [US1] Implement parallel fragment retrieval from multiple nodes in apps/frontend/src/hooks/useStorage.ts
- [ ] T093 [US1] Implement k-of-n reconstruction (any 3 of 5 fragments) in apps/frontend/src/hooks/useStorage.ts

#### Unit Tests

- [ ] T094 [P] [US1] Test fragment distribution across 5 nodes in apps/frontend/tests/hooks/useStorage.test.ts
- [ ] T095 [P] [US1] Test fallback with 3 nodes (some get multiple fragments) in apps/frontend/tests/hooks/useStorage.test.ts
- [ ] T096 [P] [US1] Test recovery with 2 nodes offline in apps/frontend/tests/hooks/useStorage.test.ts

**Checkpoint**: Multi-node distribution complete - fragments distributed across storage nodes

---

## Phase 7: User Story 2 - ノード選択ロジック設定 (Priority: P2)

**Goal**: ユーザーが断片配置時のノード選択方式を設定でき、ネットワーク状況に応じた最適化が可能になる

**Independent Test**: 各選択方式で投稿を作成し、断片配置パターンが方式ごとに異なることを確認

### Implementation for User Story 2

#### Node Selection Strategies (FR-101-105)

- [ ] T097 [US2] Define NodeSelectionStrategy enum (Random, RoundRobin, NearestNode) in apps/frontend/src/stores/storageSettings.ts
- [ ] T098 [US2] Implement Random selection (default, privacy-focused) in apps/frontend/src/hooks/useStorage.ts
- [ ] T099 [US2] Implement RoundRobin selection in apps/frontend/src/hooks/useStorage.ts
- [ ] T100 [US2] Implement NearestNode selection with ping latency measurement in apps/frontend/src/hooks/useStorage.ts
- [ ] T101 [US2] Filter offline nodes from selection candidates in apps/frontend/src/hooks/useStorage.ts

#### Settings UI (FR-103)

- [ ] T102 [US2] Add node selection strategy setting to storage settings store in apps/frontend/src/stores/storageSettings.ts
- [ ] T103 [US2] Create NodeSelectionSettings component with strategy dropdown in apps/frontend/src/components/NodeSelectionSettings.tsx
- [ ] T104 [US2] Integrate settings component into settings page in apps/frontend/src/app/settings/page.tsx

#### Unit Tests

- [ ] T105 [P] [US2] Test Random selection produces varied distribution in apps/frontend/tests/hooks/useStorage.test.ts
- [ ] T106 [P] [US2] Test RoundRobin produces even distribution in apps/frontend/tests/hooks/useStorage.test.ts
- [ ] T107 [P] [US2] Test NearestNode prioritizes low-latency nodes in apps/frontend/tests/hooks/useStorage.test.ts

**Checkpoint**: Node selection strategies complete - users can optimize distribution

---

## Phase 8: User Story 4 - 断片配置状態の可視化 (Priority: P3)

**Goal**: フロントエンドで投稿の各断片がどのノードに配置されているかを確認でき、健全性ステータスが表示される

**Independent Test**: 投稿詳細画面で断片配置情報が表示されることを確認

### Implementation for User Story 4

#### Fragment Status Component (FR-302)

- [ ] T108 [US4] Create FragmentStatus component displaying fragment-to-node mapping in apps/frontend/src/components/FragmentStatus.tsx
- [ ] T109 [US4] Add node reachability indicator (online/offline) in apps/frontend/src/components/FragmentStatus.tsx
- [ ] T110 [US4] Add health status badge (e.g., "3/5 fragments reachable") in apps/frontend/src/components/FragmentStatus.tsx
- [ ] T111 [US4] Style component with warning state for degraded health in apps/frontend/src/components/FragmentStatus.tsx

#### Integration

- [ ] T112 [US4] Integrate FragmentStatus into post detail view in apps/frontend/src/app/post/[id]/page.tsx
- [ ] T113 [US4] Add periodic health check refresh (every 30s) in apps/frontend/src/components/FragmentStatus.tsx

#### Unit Tests

- [ ] T114 [P] [US4] Test FragmentStatus renders all 5 fragments in apps/frontend/tests/components/FragmentStatus.test.tsx
- [ ] T115 [P] [US4] Test health warning displays when nodes offline in apps/frontend/tests/components/FragmentStatus.test.tsx

**Checkpoint**: Fragment visualization complete - users can monitor distribution health

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] T116 [P] Update README.md with multi-node storage documentation in apps/storage-node/README.md
- [ ] T117 [P] Update config.example.toml with new auth and P2P settings in apps/storage-node/config.example.toml
- [ ] T118 [P] Add integration test: 3-node fragment distribution in apps/blockchain/tests/integration/test_multi_node.sh
- [ ] T119 [P] Add integration test: P2P endpoint propagation in apps/blockchain/tests/integration/test_p2p_gossip.sh
- [ ] T120 [P] Add integration test: failover under node failure in apps/blockchain/tests/integration/test_failover.sh
- [ ] T121 Run quickstart.md validation scenarios
- [ ] T122 Performance profiling: fragment upload <500ms target
- [ ] T123 Code review and cleanup across all modified files

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **US5 Security (Phase 3)**: P0 priority - START HERE after Foundational
- **US3 Auth (Phase 4)**: P1 priority - can start after Foundational, independent of US5
- **US6 P2P (Phase 5)**: P1 priority - can start after Foundational, independent of US3/US5
- **US1 Distribution (Phase 6)**: P1 priority - depends on US3 (auth) for uploads
- **US2 Selection (Phase 7)**: P2 priority - depends on US1 completion
- **US4 Visualization (Phase 8)**: P3 priority - depends on US1 for fragment data
- **Polish (Phase 9)**: Final phase after all user stories complete

### User Story Dependencies

```
            ┌─────────────┐
            │ Foundational│
            └──────┬──────┘
                   │
    ┌──────────────┼──────────────┬─────────────┐
    ▼              ▼              ▼             │
┌───────┐     ┌───────┐     ┌───────┐          │
│ US5   │     │ US3   │     │ US6   │          │
│ P0    │     │ P1    │     │ P1    │          │
└───────┘     └───┬───┘     └───────┘          │
                  │                             │
                  ▼                             │
             ┌───────┐                         │
             │ US1   ├─────────────────────────┤
             │ P1    │                         │
             └───┬───┘                         │
    ┌────────────┴────────────┐               │
    ▼                         ▼               │
┌───────┐                 ┌───────┐           │
│ US2   │                 │ US4   │           │
│ P2    │                 │ P3    │           │
└───────┘                 └───────┘           │
                                              │
                          ┌─────────┐         │
                          │ Polish  │◀────────┘
                          └─────────┘
```

### Parallel Opportunities

**Setup Phase (all [P]):**
```bash
# Can run in parallel after T001
T002, T003, T004, T005, T006, T007
```

**Foundational Phase (all [P]):**
```bash
# Can run in parallel after T008
T009, T010, T011, T012, T013, T014
```

**User Story Parallelization:**
```bash
# After Foundational, these can proceed in parallel:
# - US5 (T016-T041) - Pallet security
# - US3 (T042-T056) - Storage node auth  
# - US6 (T057-T085) - P2P communication

# Within each story, tests marked [P] can run in parallel
```

---

## Implementation Strategy

### MVP Scope (Recommended)

**Minimum Viable Product**: Complete User Story 5 (P0) only

This provides:
- DoS protection for Storage Pallet
- PoW verification for node registration
- Rate limiting for declarations
- Post-Storage pallet coupling

**Phase 1 Delivery**: Add US3 + US6 + US1 for full multi-node support

**Full Feature**: Add US2 + US4 for complete user experience

### Task Count Summary

| Phase | Story | Task Count |
|-------|-------|------------|
| Setup | - | 7 |
| Foundational | - | 8 |
| Phase 3 | US5 (P0) | 26 |
| Phase 4 | US3 (P1) | 15 |
| Phase 5 | US6 (P1) | 29 |
| Phase 6 | US1 (P1) | 11 |
| Phase 7 | US2 (P2) | 11 |
| Phase 8 | US4 (P3) | 8 |
| Polish | - | 8 |
| **Total** | | **123** |

### Estimated Time (Single Developer)

- Setup + Foundational: 1 day
- US5 (P0 Security): 3-4 days
- US3 (P1 Auth): 2 days
- US6 (P1 P2P): 4-5 days
- US1 (P1 Distribution): 2 days
- US2 (P2 Selection): 1-2 days
- US4 (P3 Visualization): 1 day
- Polish: 1-2 days

**Total Estimate**: 15-18 days

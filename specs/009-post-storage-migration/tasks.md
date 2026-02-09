# Tasks: Post Storage Migration（オンチェーン・ダイエット）

**Input**: Design documents from `/specs/009-post-storage-migration/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/storage-rpc.json ✓, quickstart.md ✓

**Tests**: テスト重視で実装。各User Storyにテスト→実装の順序。

**Organization**: User Story単位でタスクをグループ化し、独立した実装・テストを可能に。

---

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 並列実行可（異なるファイル、依存関係なし）
- **[Story]**: User Story所属（US1, US2, US3, US4）
- ファイルパスを明記

---

## Phase 1: Setup (共有インフラ)

**Purpose**: プロジェクト初期化とWasm暗号エンジンの基盤構築

- [X] T001 Create packages/wasm-engine directory structure
- [X] T002 Initialize Cargo.toml with dependencies (sharks, rs_merkle, blake2, wasm-bindgen) in packages/wasm-engine/Cargo.toml
- [X] T003 [P] Configure wasm-pack build settings in packages/wasm-engine/
- [X] T004 [P] Add wasm-engine to pnpm workspace in pnpm-workspace.yaml
- [X] T005 [P] Configure Cargo workspace to include packages/wasm-engine in root Cargo.toml (N/A - using packages/)

---

## Phase 2: Foundational (ブロッキング前提条件)

**Purpose**: すべてのUser Storyに必要なコア基盤

**⚠️ CRITICAL**: このPhase完了までUser Story作業は開始不可

### Tests for Foundational ⚠️

> **NOTE: テストを先に書き、実装前にFAIL確認**

- [X] T006 [P] Unit test for SSS split/recover in packages/wasm-engine/src/sss.rs (test_split_recover, test_insufficient_shares)
- [X] T007 [P] Unit test for MerkleTree build/verify in packages/wasm-engine/src/merkle.rs (test_build_tree, test_proof_verify, test_proof_reject_invalid)
- [X] T008 [P] Unit test for PostContent struct encoding in apps/blockchain/pallets/post/src/lib.rs (test_post_content_encode_decode)

### Implementation for Foundational

- [X] T009 [P] Implement Blake2bHasher for rs_merkle in packages/wasm-engine/src/merkle.rs
- [X] T010 [P] Implement SSS split function (k=3, n=5) in packages/wasm-engine/src/sss.rs
- [X] T011 [P] Implement SSS recover function in packages/wasm-engine/src/sss.rs
- [X] T012 [P] Implement MerkleTree build function in packages/wasm-engine/src/merkle.rs
- [X] T013 [P] Implement MerkleProof generate function in packages/wasm-engine/src/merkle.rs
- [X] T014 [P] Implement MerkleProof verify function in packages/wasm-engine/src/merkle.rs
- [X] T015 Create Wasm entry points with wasm-bindgen in packages/wasm-engine/src/lib.rs
- [X] T016 Build and verify Wasm output with wasm-pack in packages/wasm-engine/
- [X] T017 Add PostContent struct to pallet-post in apps/blockchain/pallets/post/src/lib.rs
- [X] T018 Add ContentRefs StorageMap to pallet-post in apps/blockchain/pallets/post/src/lib.rs
- [X] T019 Remove Contents StorageMap from pallet-post in apps/blockchain/pallets/post/src/lib.rs (replaced with ContentRefs)

**Checkpoint**: Wasm暗号エンジン動作確認、pallet-post新ストレージ構造準備完了

---

## Phase 3: User Story 1 - 投稿作成（新フロー）(Priority: P1) 🎯 MVP

**Goal**: フロントエンドでSSS分割・Merkle構築、Blockchain Node経由でStorage Nodeへアップロード、チェーンにMerkleRootのみ記録

**Independent Test**: テストネットで投稿を作成し、チェーン上にコンテンツ本体が保存されず、MerkleRootのみが記録されることを確認

### Tests for User Story 1 ⚠️

> **NOTE: テストを先に書き、実装前にFAIL確認**

- [X] T020 [P] [US1] Unit test for create_post with V2 params in apps/blockchain/pallets/post/src/tests.rs
- [X] T021 [P] [US1] Unit test for cost calculation (50:30:20) in apps/blockchain/pallets/post/src/tests.rs
- [X] T022 [P] [US1] Unit test for k/n validation (ensure k > 0 && k <= n) in apps/blockchain/pallets/post/src/tests.rs
- [X] T022b [P] [US1] Unit test for storage deposit allocation (20% of cost → Storage reward pool for declare_holding) in apps/blockchain/pallets/post/src/tests.rs
- [X] T023 [P] [US1] RPC test for storage_uploadFragment in apps/blockchain/node/src/rpc/storage.rs (test_merkle_proof_verification)
- [X] T024 [P] [US1] RPC test for MerkleProof validation (accept valid, reject invalid) in apps/blockchain/node/src/rpc/storage.rs
- [SKIP] T025 [P] [US1] Integration test for libp2p fragment forwarding in apps/blockchain/node/tests/integration/ (SKIP: libp2p未接続、localhost RPCで対応)
- [X] T026 [P] [US1] Frontend unit test for useStorage.uploadFragment in apps/frontend/tests/hooks/useStorage.test.ts

### Implementation for User Story 1

- [X] T027 [US1] Modify create_post to accept (merkle_root, k, n, total_size) in apps/blockchain/pallets/post/src/lib.rs (create_post_v2)
- [X] T028 [US1] Implement cost calculation (base 50% + size 30% + deposit 20%) in apps/blockchain/pallets/post/src/lib.rs
- [X] T029 [P] [US1] Create StorageApi trait definition in apps/blockchain/node/src/rpc/storage.rs
- [X] T030 [P] [US1] Implement storage_uploadFragment RPC method in apps/blockchain/node/src/rpc/storage.rs
- [X] T031 [US1] Implement MerkleProof verification in storage_uploadFragment in apps/blockchain/node/src/rpc/storage.rs
- [X] T032 [US1] Add libp2p NetworkService to FullDeps in apps/blockchain/node/src/rpc/mod.rs (stub: None)
- [X] T033 [US1] Implement fragment forwarding to Storage Node via HTTP in apps/blockchain/node/src/rpc/storage.rs (StorageNodeClient実装)
- [X] T034 [US1] Register StorageApi in create_full() in apps/blockchain/node/src/rpc/mod.rs
- [X] T035 [P] [US1] Create crypto.ts Web Worker for Wasm invocation in apps/frontend/src/workers/crypto.ts
- [X] T036 [P] [US1] Create useStorage hook with uploadFragment in apps/frontend/src/hooks/useStorage.ts
- [X] T037 [US1] Integrate Wasm SSS split + Merkle build in useStorage in apps/frontend/src/hooks/useStorage.ts
- [X] T038 [US1] Implement parallel fragment upload (n=5) with retry (3x + fallback) in apps/frontend/src/hooks/useStorage.ts
- [X] T039 [US1] Update post creation UI to use new V2 flow in apps/frontend/src/components/CreatePost.tsx (or equivalent)

**Checkpoint**: 投稿作成時にMerkleRootのみがチェーンに記録され、断片がStorage Nodeに保存されることを確認

---

## Phase 4: User Story 2 - 投稿表示（断片取得・復元）(Priority: P1)

**Goal**: Storage Nodeからk個以上の断片を取得し、SSSで元データを復元して表示

**Independent Test**: 既存の分散保存された投稿を表示し、元のコンテンツが正しく復元されることを確認

### Tests for User Story 2 ⚠️

> **NOTE: テストを先に書き、実装前にFAIL確認**

- [X] T040 [P] [US2] RPC test for storage_getFragment in apps/blockchain/node/src/rpc/storage.rs (tests module)
- [X] T041 [P] [US2] RPC test for storage_getPostInfo in apps/blockchain/node/src/rpc/storage.rs (tests module)
- [SKIP] T042 [P] [US2] Integration test for fragment retrieval from Storage Node in apps/blockchain/node/tests/integration/ (SKIP: Storage Node起動環境必要)
- [X] T043 [P] [US2] Frontend unit test for useStorage.getFragment in apps/frontend/tests/hooks/useStorage.test.ts
- [X] T044 [P] [US2] Frontend unit test for SSS recover (k fragments) in apps/frontend/tests/hooks/useStorage.test.ts
- [X] T045 [P] [US2] Frontend test for partial availability (k of n nodes online) in apps/frontend/tests/hooks/useStorage.test.ts

### Implementation for User Story 2

- [X] T046 [US2] Implement storage_getFragment RPC method in apps/blockchain/node/src/rpc/storage.rs (StorageNodeClient経由)
- [X] T047 [US2] Implement storage_getPostInfo RPC method in apps/blockchain/node/src/rpc/storage.rs
- [X] T048 [US2] Implement storage_listHolders RPC method in apps/blockchain/node/src/rpc/storage.rs (スタブ実装、将来インデクサー連携)
- [X] T049 [US2] Add getFragment, getPostInfo to useStorage hook in apps/frontend/src/hooks/useStorage.ts
- [X] T050 [US2] Implement parallel fragment fetch (k of n) in useStorage in apps/frontend/src/hooks/useStorage.ts
- [X] T051 [US2] Integrate Wasm SSS recover in useStorage in apps/frontend/src/hooks/useStorage.ts
- [X] T052 [US2] Handle partial availability (k個未満時エラー表示) in apps/frontend/src/hooks/useStorage.ts
- [X] T053 [US2] Update post display component to use V2 fetch in apps/frontend/src/components/PostContent.tsx (or equivalent)

**Checkpoint**: 分散保存された投稿が正しく復元・表示されることを確認

---

## Phase 5: User Story 3 - Storage Node断片受信・保持表明 (Priority: P1)

**Goal**: Storage NodeがBlockchain Nodeから断片を受信し、保存後にdeclare_holdingを送信

**Independent Test**: Blockchain Node経由で断片をアップロードし、declare_holdingがチェーンに記録されることを確認

### Tests for User Story 3 ✅

> **NOTE: テストを先に書き、実装前にFAIL確認**

- [X] T054 [P] [US3] Unit test for fragment storage to disk in apps/storage-node/src/storage/tests.rs
- [X] T055 [P] [US3] Unit test for declare_holding subxt call in apps/storage-node/src/chain/tests.rs
- [X] T056 [P] [US3] Integration test for libp2p fragment receive in apps/storage-node/tests/integration/
- [X] T057 [P] [US3] Integration test for full flow (receive → store → declare) in apps/storage-node/tests/integration/

### Implementation for User Story 3

- [X] T058 [US3] Extend libp2p request-response handler for fragment upload in apps/storage-node/src/network/mod.rs
- [X] T059 [US3] Implement fragment disk storage (fragments/{post_id}/{index}.bin) in apps/storage-node/src/storage/mod.rs
- [X] T060 [US3] Implement automatic declare_holding via subxt after storage in apps/storage-node/src/chain/mod.rs
- [X] T061 [US3] Handle disk full error and return proper response in apps/storage-node/src/storage/mod.rs
- [X] T062 [US3] Implement fragment retrieval for get requests in apps/storage-node/src/network/mod.rs

**Checkpoint**: 断片がStorage Nodeに保存され、declare_holdingがチェーンに記録されることを確認

---

## Phase 6: User Story 4 - ローカルキャッシュによる高速表示 (Priority: P2) [SKIPPED]

**Goal**: 復元済みコンテンツをローカルキャッシュし、再訪問時に高速表示

**Status**: ⏭️ SKIPPED - フロントエンド複雑化を避けるため将来実装予定

**Independent Test**: 同じ投稿を2回表示し、2回目はネットワークリクエストなしで即座に表示されることを確認

### Tests for User Story 4 ⏭️

> **NOTE: テストを先に書き、実装前にFAIL確認**

- [SKIP] T063 [P] [US4] Unit test for cache write in apps/frontend/tests/hooks/useStorage.test.ts
- [SKIP] T064 [P] [US4] Unit test for cache read (hit) in apps/frontend/tests/hooks/useStorage.test.ts
- [SKIP] T065 [P] [US4] Unit test for LRU eviction in apps/frontend/tests/hooks/useStorage.test.ts

### Implementation for User Story 4

- [SKIP] T066 [US4] Implement IndexedDB/localStorage cache layer in apps/frontend/src/lib/cache.ts
- [SKIP] T067 [US4] Implement LRU eviction policy in apps/frontend/src/lib/cache.ts
- [SKIP] T068 [US4] Integrate cache in useStorage.getFragment (check cache first) in apps/frontend/src/hooks/useStorage.ts
- [SKIP] T069 [US4] Write to cache after successful SSS recover in apps/frontend/src/hooks/useStorage.ts

**Checkpoint**: 2回目の投稿表示がキャッシュヒットで1秒以内

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 複数User Storyにまたがる改善

- [X] T070 [P] Update API documentation for new RPC methods in docs/ (README.md updated with storage-node & wasm-engine sections)
- [SKIP] T071 [P] Add JSDoc comments to useStorage hook in apps/frontend/src/hooks/useStorage.ts (コード内コメントで十分)
- [SKIP] T072 [P] Add rustdoc comments to StorageApi in apps/blockchain/node/src/rpc/storage.rs (コード内コメントで十分)
- [SKIP] T073 Performance optimization: parallel fragment operations tuning (将来最適化)
- [X] T074 Security hardening: input validation, size limits (256KB per fragment)
- [SKIP] T075 Run quickstart.md validation end-to-end (includes SC verification: cost reduction ≤50%, 1MB+ support, ≤5s latency) (手動検証済み)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 依存なし - 即開始可
- **Foundational (Phase 2)**: Setup完了後 - **全User Storyをブロック**
- **User Stories (Phase 3-6)**: Foundational完了後に開始可
  - US1 (投稿作成) → US2 (投稿表示) → US3 (Storage Node) → US4 (キャッシュ)
  - US1とUS3は並列可能（異なるコンポーネント）
  - US2はUS1完了後（投稿が必要）
  - US4はUS2完了後（表示機能が必要）
- **Polish (Phase 7)**: 全User Story完了後

### User Story Dependencies

```
Phase 1 (Setup)
    │
    ▼
Phase 2 (Foundational) ──────────────────────────────┐
    │                                                 │
    ├─────────────────────┬───────────────────────────┤
    ▼                     ▼                           │
Phase 3 (US1)        Phase 5 (US3)                    │
投稿作成              Storage Node                     │
    │                     │                           │
    ▼                     │                           │
Phase 4 (US2) ◀───────────┘                           │
投稿表示（US1, US3が必要）                              │
    │                                                 │
    ▼                                                 │
Phase 6 (US4)                                         │
キャッシュ（US2が必要）                                 │
    │                                                 │
    ▼                                                 │
Phase 7 (Polish) ◀────────────────────────────────────┘
```

### Parallel Opportunities

**Phase 2 (Foundational)内**:
- T006, T007, T008: 全テスト並列
- T009〜T014: 実装も並列（異なるファイル）
- T017, T018, T019: pallet-post変更は順序あり

**Phase 3 (US1)内**:
- T020〜T026: 全テスト並列
- T029, T030: RPC定義・実装並列
- T035, T036: Frontend Worker/Hook並列

**Phase 4, 5, 6**: 各Phase内のテストは並列実行可

---

## Parallel Example: Phase 2

```bash
# テストファイル並列起動
Task: T006 "Unit test for SSS split/recover in packages/wasm-engine/src/sss.rs"
Task: T007 "Unit test for MerkleTree build/verify in packages/wasm-engine/src/merkle.rs"
Task: T008 "Unit test for PostContent struct encoding in pallets/post"

# Wasm実装並列起動
Task: T009 "Implement Blake2bHasher for rs_merkle in packages/wasm-engine/src/merkle.rs"
Task: T010 "Implement SSS split function in packages/wasm-engine/src/sss.rs"
Task: T011 "Implement SSS recover function in packages/wasm-engine/src/sss.rs"
```

## Parallel Example: User Story 1 (Phase 3)

```bash
# 全テスト並列起動（FAIL確認用）
Task: T020〜T026 "All US1 tests"

# 並列実装（異なるファイル）
Task: T029 "Create StorageApi trait definition in apps/blockchain/node/src/rpc/storage.rs"
Task: T035 "Create crypto.ts Web Worker in apps/frontend/src/workers/crypto.ts"
Task: T036 "Create useStorage hook in apps/frontend/src/hooks/useStorage.ts"
```

---

## Implementation Strategy

### MVP First (User Story 1 のみ)

1. Phase 1: Setup 完了
2. Phase 2: Foundational 完了（**必須 - 全Storyをブロック**）
3. Phase 3: User Story 1 完了
4. **STOP & VALIDATE**: 投稿作成→MerkleRoot記録を独立テスト
5. 準備できればデプロイ/デモ

### Incremental Delivery

1. Setup + Foundational → 基盤完成
2. US1 (投稿作成) → 独立テスト → デプロイ (MVP!)
3. US3 (Storage Node) → US2 (投稿表示) → 独立テスト → デプロイ
4. US4 (キャッシュ) → 独立テスト → デプロイ
5. 各Storyが以前のStoryを壊さずに価値を追加

### Test-Driven Approach (テスト重視)

各User Storyで:
1. テストタスク（T0XX [P] [USn]）を**先に実行**
2. テストが**FAIL**することを確認
3. 実装タスクを実行
4. テストが**PASS**することを確認
5. 次のUser Storyへ

---

## Notes

- [P] = 異なるファイル、依存関係なし
- [Story] = User Story所属（トレーサビリティ用）
- 各User Storyは独立して完了・テスト可能
- **テストが先に失敗することを確認してから実装**
- タスクまたは論理グループ完了後にコミット
- 任意のCheckpointでStoryを独立検証するために停止可
- 避ける: 曖昧なタスク、同一ファイル競合、Story間の独立性を壊す依存関係

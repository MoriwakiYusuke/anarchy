# Tasks: KZG-VSS 保持証明・報酬システム

**Input**: Design documents from `/specs/011-kzg-proof-rewards/`
**Prerequisites**: plan.md (✓), spec.md (✓), research.md (✓), data-model.md (✓), contracts/ (✓)

**Tests**: Included as specified in spec.md (TDD approach per Constitution VI)

## TDD Policy (Constitution VI)

> **テストファースト原則**: 各User Storyは「テスト作成 → テスト失敗確認 → 実装 → テストパス」の順で進行する。
> **実装ブロック**: テストセクションの全タスクが完了し、テストが（意図的に）失敗する状態になるまで、Implementationセクションに進んではならない。

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story (US1-US5) this task belongs to
- File paths from plan.md Project Structure

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization, dependencies, Trusted Setup

- [x] T001 Add arkworks dependencies to `packages/wasm-engine/Cargo.toml` (ark-bls12-381, ark-poly, ark-poly-commit) and remove sharks dependency
- [x] T002 [P] Download Ethereum KZG Ceremony SRS and add to `packages/wasm-engine/srs/mainnet.bin`
- [x] T003 [P] Add flate2 (gzip) dependency to `packages/wasm-engine/Cargo.toml`
- [x] T004 [P] Create `packages/wasm-engine/src/kzg/mod.rs` module structure
- [x] T005 [P] Add new storage types to `apps/blockchain/pallets/storage/src/lib.rs` (Fragment, Challenge, ProofRecord) and create `tests/` directory with `mod.rs`
- [x] T006 [P] Create frontend service stubs `apps/frontend/src/services/kzg-vss.ts` and `compression.ts`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure required by ALL user stories

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T007 Implement SRS loading in `packages/wasm-engine/src/kzg/srs.rs` (FR-003, T-007)
- [x] T008 [P] Implement compression/decompression in `packages/wasm-engine/src/kzg/compression.rs` (FR-306, T-008, T-009)
- [x] T009 Implement BLS12-381 scalar encoding (32-byte chunks) in `packages/wasm-engine/src/kzg/encoding.rs`
- [x] T010 [P] Add `RewardPoolBalance` storage and 90/10 split logic to `apps/blockchain/pallets/storage/src/lib.rs` (FR-113, FR-114)
- [x] T011 Build wasm-pack target and verify module loads: `cd packages/wasm-engine && wasm-pack build --target web`
- [x] T081 ⚠️ **no_std PoC検証**: `apps/blockchain/pallets/storage/`で`ark-poly-commit`のno_stdコンパイルテストを実施。wasm32-unknown-unknownターゲットでコンパイル成功を確認。

**Checkpoint**: Foundation ready - KZG primitives available, user story implementation can begin

> **⚠️ T081 Decision Point**: no_std検証が失敗した場合、以下の選択肢をユーザーに提示:
> 1. Off-chain Worker実装（KZG検証をオフチェーンで実行、追加工数+2-3日）
> 2. フロントエンドのみでKZG検証（オンチェーン検証を断念）
> 3. 代替ライブラリ調査（blst等、追加調査時間必要）

---

## Phase 3: User Story 1 - 投稿の暗号学的断片化 (Priority: P1) 🎯 MVP

**Goal**: クライアント側でKZG-VSSによる3-of-5シェア生成、コミットメントのオンチェーン保存、復元機能

**Independent Test**: テスト投稿を作成し、KZGコミットメントがオンチェーンに保存され、3個以上のシェアから元データが復元できることを確認

### Tests for User Story 1

**Acceptance Scenario Coverage:**

| AS# | Scenario | Test Task | spec.md Ref |
|-----|----------|-----------|-------------|
| AS1-1 | Submit → 5シェア生成 | T012 | T-001 |
| AS1-2 | アップロード → コミットメント記録 | T018 | T-201 |
| AS1-3 | 投稿費用 → 90%プール/10%バーン | T017 | T-108 |
| AS1-4 | 3個シェア → 復元成功 | T013 | T-002 |
| AS1-5 | 2個シェア → 復元失敗 | T014 | T-003 |

- [X] T012 [P] [US1] Unit test: `vss_split` で3-of-5シェア生成 in `packages/wasm-engine/tests/kzg_tests.rs` (T-001)
- [X] T013 [P] [US1] Unit test: `vss_recover` で3個のシェアから復元成功 in `packages/wasm-engine/tests/kzg_tests.rs` (T-002)
- [X] T014 [P] [US1] Unit test: `vss_recover` で2個のシェアでは復元失敗 in `packages/wasm-engine/tests/kzg_tests.rs` (T-003)
- [X] T015 [P] [US1] Unit test: 圧縮→分割→復元→解凍ラウンドトリップ in `packages/wasm-engine/tests/kzg_tests.rs` (T-008)
- [X] T016 [P] [US1] Unit test: 32KB超データの分割処理 in `packages/wasm-engine/tests/kzg_tests.rs` (T-006)
- [X] T017 [P] [US1] Pallet test: `register_fragment` で90%報酬プール/10%バーン in `apps/blockchain/pallets/storage/tests/kzg_tests.rs` (T-108)
- [X] T018 [P] [US1] Integration test: E2E 投稿作成→KZG-VSS分割→アップロード→コミットメント保存 (T-201)

### Implementation for User Story 1

- [X] T019 [US1] Implement polynomial construction from data in `packages/wasm-engine/src/kzg/vss.rs` (FR-002)
- [X] T020 [US1] Implement `vss_split` function (Lagrange shares + KZG commitment) in `packages/wasm-engine/src/kzg/vss.rs` (FR-002, FR-004)
- [X] T021 [US1] Implement `vss_recover` function (Lagrange interpolation) in `packages/wasm-engine/src/kzg/vss.rs` (FR-005)
- [ ] T022 [US1] **BLOCKED** Multi-segment support for >32KB data - Requires design decision (Option A: segment-per-polynomial, Option B: symmetric encryption + key sharing)
- [X] T023 [US1] Export Wasm bindings via wasm-bindgen in `packages/wasm-engine/src/lib.rs`
- [X] T024 [US1] Implement `register_kzg_fragment` extrinsic in `apps/blockchain/pallets/storage/src/lib.rs` (FR-102)
- [X] T025 [US1] Integrate KZG-VSS in frontend post creation in `apps/frontend/src/services/kzg-vss.ts` (FR-301, FR-306)
- [X] T026 [US1] Integrate KZG-VSS recovery in frontend post viewing in `apps/frontend/src/services/kzg-vss.ts` (FR-302, FR-307)

**Checkpoint**: US1完了 - 投稿のKZG-VSS断片化と復元が機能

---

## Phase 4: User Story 2 - 保持証明の提出と検証 (Priority: P1)

**Goal**: ストレージノードがKZG proofを提出し、オンチェーンで検証される

**Independent Test**: ストレージノードを起動し、チェーンからのチャレンジに対して有効な証明を提出し、検証成功を確認

### Tests for User Story 2

**Acceptance Scenario Coverage:**

| AS# | Scenario | Test Task | spec.md Ref |
|-----|----------|-----------|-------------|
| AS2-1 | チャレンジ → proof提出 | T033 | T-202 |
| AS2-2 | 有効proof → 検証成功 | T029 | T-101 |
| AS2-3 | 無効proof → 検証失敗 | T030 | T-102 |
| AS2-4 | シェア削除 → 未応答カウント | T032 | T-107 |

- [ ] T027 [P] [US2] Unit test: `vss_prove` で有効なKZG proof生成 in `packages/wasm-engine/tests/kzg_tests.rs` (T-004)
- [ ] T028 [P] [US2] Unit test: 不正シェア値でKZG proof検証失敗 in `packages/wasm-engine/tests/kzg_tests.rs` (T-005)
- [ ] T029 [P] [US2] Pallet test: `prove_holding_kzg` で有効な証明が検証される in `apps/blockchain/pallets/storage/tests/kzg_tests.rs` (T-101)
- [ ] T030 [P] [US2] Pallet test: 無効な証明で `InvalidKzgProof` エラー in `apps/blockchain/pallets/storage/tests/kzg_tests.rs` (T-102)
- [ ] T031 [P] [US2] Pallet test: チャレンジ生成がランダムに動作 in `apps/blockchain/pallets/storage/tests/kzg_tests.rs` (T-106)
- [ ] T032 [P] [US2] Pallet test: 未応答カウントが正しく増加 in `apps/blockchain/pallets/storage/tests/kzg_tests.rs` (T-107)
- [ ] T033 [P] [US2] Integration test: E2E チャレンジ発行→証明提出→検証成功 (T-202 partial)

**⛔ IMPLEMENTATION BLOCKED**: 上記テストが全て作成され、意図的に失敗する状態になるまで実装に進まない

### Implementation for User Story 2

- [ ] T034 [US2] Implement `vss_prove` function in `packages/wasm-engine/src/kzg/proof.rs` (FR-004)
- [ ] T035 [US2] Implement `verify_kzg_proof` function in `packages/wasm-engine/src/kzg/proof.rs`
- [ ] T036 [US2] Implement KZG verification logic (no_std) in `apps/blockchain/pallets/storage/src/kzg.rs` (FR-101)
- [ ] T037 [US2] Implement `prove_holding_kzg` extrinsic in `apps/blockchain/pallets/storage/src/lib.rs` (FR-101, FR-104)
- [ ] T038 [US2] Implement `issue_challenge` extrinsic in `apps/blockchain/pallets/storage/src/challenge.rs` (FR-103)
- [ ] T039 [US2] Implement challenge monitoring in `apps/storage-node/src/challenge.rs` (FR-202)
- [ ] T040 [US2] Implement KZG proof generation in storage node in `apps/storage-node/src/prover.rs` (FR-201)
- [ ] T041 [US2] Implement automatic proof submission in `apps/storage-node/src/challenge.rs` (FR-202, FR-205)
- [ ] T042 [US2] Implement failure counting and warning flag in `apps/blockchain/pallets/storage/src/lib.rs` (FR-105)

**Checkpoint**: US2完了 - 保持証明の提出と検証が機能

---

## Phase 5: User Story 3 - 保持報酬の分配 (Priority: P2)

**Goal**: 保持証明成功ノードに報酬プールから$moralが分配される

**Independent Test**: 保持証明成功後、ノードオペレーターのウォレット残高が増加することを確認

### Tests for User Story 3

**Acceptance Scenario Coverage:**

| AS# | Scenario | Test Task | spec.md Ref |
|-----|----------|-----------|-------------|
| AS3-1 | 証明成功(閾値以上) → 報酬計算 | T043 | T-103 |
| AS3-2 | 大きいデータ → 高い報酬 | T075 | — |
| AS3-3 | 複数断片 → 報酬累積 | T076 | — |
| AS3-4 | 閾値未満 → 報酬0 | T051 | T-104 (US4) |
| AS3-5 | プール枯渇 → 按分 | T044 | — |

- [ ] T043 [P] [US3] Pallet test: スコア閾値以上で報酬計算（データサイズ依存） in `apps/blockchain/pallets/storage/tests/kzg_tests.rs` (T-103)
- [ ] T044 [P] [US3] Pallet test: 報酬プール枯渇時に按分分配 in `apps/blockchain/pallets/storage/tests/kzg_tests.rs`
- [ ] T045 [P] [US3] Integration test: E2E 保持証明成功→報酬分配 (T-202 complete)
- [ ] T075 [P] [US3] Pallet test: 大きいデータサイズ→高い報酬（1KB vs 10KB比較） in `apps/blockchain/pallets/storage/tests/kzg_tests.rs`
- [ ] T076 [P] [US3] Pallet test: 複数断片保持→報酬累積 in `apps/blockchain/pallets/storage/tests/kzg_tests.rs`

**⛔ IMPLEMENTATION BLOCKED**: 上記テストが全て作成され、意図的に失敗する状態になるまで実装に進まない

### Implementation for User Story 3

- [ ] T046 [US3] Implement reward calculation `base_reward_per_byte × data_size` in `apps/blockchain/pallets/storage/src/rewards.rs` (FR-109)
- [ ] T047 [US3] Implement pending reward accumulation in ProofRecord in `apps/blockchain/pallets/storage/src/rewards.rs`
- [ ] T048 [US3] Implement `claim_reward` extrinsic in `apps/blockchain/pallets/storage/src/lib.rs` (FR-108)
- [ ] T049 [US3] Implement 24-hour batch processing (Off-chain Worker or hook) in `apps/blockchain/pallets/storage/src/rewards.rs`
- [ ] T050 [US3] Add `BaseRewardPerByte` config parameter in `apps/blockchain/pallets/storage/src/lib.rs`

**Checkpoint**: US3完了 - 報酬分配が機能

---

## Phase 6: User Story 4 - スコア閾値による自然な忘却 (Priority: P2)

**Goal**: スコア閾値未満のデータは報酬0→GC→復元不可能

**Independent Test**: スコアが閾値を下回る状態をシミュレートし、一定期間後にシェアが取得不能になることを確認

### Tests for User Story 4

**Acceptance Scenario Coverage:**

| AS# | Scenario | Test Task | spec.md Ref |
|-----|----------|-----------|-------------|
| AS4-1 | スコア閾値未満 → 報酬0 | T051 | T-104 |
| AS4-2 | 報酬0 → GC候補 | T052 | T-105 |
| AS4-3 | 3個未満 → 復元失敗 | T053 | T-203 |
| AS4-4 | スコア回復 → 保持継続 | T054 | T-204 |

- [ ] T051 [P] [US4] Pallet test: スコア閾値未満で報酬が0になる in `apps/blockchain/pallets/storage/tests/kzg_tests.rs` (T-104)
- [ ] T052 [P] [US4] Pallet test: 報酬0の断片が「忘却候補」になる in `apps/blockchain/pallets/storage/tests/kzg_tests.rs` (T-105)
- [ ] T053 [P] [US4] Integration test: E2E スコア閾値未満→報酬0→GC→復元失敗 (T-203)
- [ ] T054 [P] [US4] Integration test: E2E スコア回復→報酬再開→保持継続 (T-204)
- [ ] T077 [P] [US4] Integration test: フロントエンド「このコンテンツは利用できなくなりました」表示 (AS4-3 UI)

**⛔ IMPLEMENTATION BLOCKED**: 上記テストが全て作成され、意図的に失敗する状態になるまで実装に進まない

### Implementation for User Story 4

- [ ] T055 [US4] Implement score-based reward gating in `apps/blockchain/pallets/storage/src/rewards.rs` (FR-107)
- [ ] T056 [US4] Implement "forgetting candidate" marking in `apps/blockchain/pallets/storage/src/lib.rs` (FR-110)
- [ ] T057 [US4] Implement score-based GC logic in `apps/storage-node/src/gc.rs` (FR-203)
- [ ] T058 [US4] Implement 7-day grace period before GC in `apps/storage-node/src/gc.rs` (FR-204)
- [ ] T059 [US4] Add `ScoreThreshold` config parameter in `apps/blockchain/pallets/storage/src/lib.rs` (FR-111)
- [ ] T060 [US4] Display "forgetting candidate" warning in frontend in `apps/frontend/src/components/ScoreIndicator.tsx` (FR-304)

**Checkpoint**: US4完了 - 経済的忘却メカニズムが機能

---

## Phase 7: User Story 5 - スコアベースの報酬制御 (Priority: P3)

**Goal**: ScoreProviderインターフェース定義、デフォルト実装（全投稿報酬対象）

**Independent Test**: スコア閾値を設定し、閾値以上/未満の投稿で報酬が正しく切り替わることを確認

### Tests for User Story 5

**Acceptance Scenario Coverage:**

| AS# | Scenario | Test Task | spec.md Ref |
|-----|----------|-----------|-------------|
| AS5-1 | 閾値以上 → 報酬付与 | T043 | T-103 (US3) |
| AS5-2 | 閾値未満 → 報酬0 | T051 | T-104 (US4) |
| AS5-3 | システム未接続 → デフォルト | T061, T062 | T-205 |
| AS5-4 | 大きいデータ → 高い報酬 | T075 | — (US3) |

- [ ] T061 [P] [US5] Pallet test: ScoreProvider未接続時にデフォルトスコア使用 in `apps/blockchain/pallets/storage/tests/kzg_tests.rs`
- [ ] T062 [P] [US5] Integration test: E2E スコアシステム未接続→全投稿が報酬対象 (T-205)

**⛔ IMPLEMENTATION BLOCKED**: 上記テストが全て作成され、意図的に失敗する状態になるまで実装に進まない

### Implementation for User Story 5

- [ ] T063 [US5] Define `ScoreProvider` trait in `apps/blockchain/pallets/storage/src/lib.rs` (FR-106)
- [ ] T064 [US5] Implement default ScoreProvider (returns None → default score) (FR-112)
- [ ] T065 [US5] Add ScoreCache storage for external score caching in `apps/blockchain/pallets/storage/src/lib.rs`
- [ ] T066 [US5] Display score in frontend (when connected) in `apps/frontend/src/services/kzg-vss.ts` (FR-303)
- [ ] T067 [US5] Skip score display when ScoreProvider unavailable in frontend (FR-305)

**Checkpoint**: US5完了 - スコアインターフェース準備完了（後続実装に対応可能）

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Integration, documentation, performance optimization, success criteria verification

### Success Criteria Tests

- [ ] T068 [P] Add performance benchmarks for SC-001 (1MB <5s browser) in `packages/wasm-engine/benches/`
- [ ] T069 [P] Add performance benchmarks for SC-002 (verify <10ms on-chain) in `apps/blockchain/pallets/storage/benchmarking.rs`
- [ ] T078 [P] Add performance benchmark for SC-003 (100-node batch verification <1s) in `apps/blockchain/pallets/storage/benchmarking.rs`
- [ ] T079 [P] Integration test: SC-004 proof success rate measurement (>=99% for active nodes)
- [ ] T080 [P] Integration test: SC-005 GC timing accuracy (±10% of grace period)

### Cross-Cutting

- [ ] T070 [P] Integration test: E2E 投稿費用→90%報酬プール→10%バーン (T-206)
- [ ] T071 [P] Update CLAUDE.md with KZG-VSS module documentation
- [ ] T072 [P] Add hysteresis for score boundary changes (Edge Case)
- [ ] T073 Run quickstart.md validation (all commands succeed)
- [ ] T074 Remove sharks crate from `Cargo.toml` and delete `packages/wasm-engine/src/sss_core.rs`

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1 (Setup) ──────────────────────────────────────┐
          │                                            │
          ▼                                            ▼
Phase 2 (Foundational) ◄──── BLOCKS ALL USER STORIES
          │
          ├─────────────────────┬─────────────────────┐
          │                     │                     │
          ▼                     ▼                     │
      Phase 3              Phase 4                    │
        (US1)                (US2)                    │
         P1                   P1                      │
          │                     │                     │
          └──────────┬──────────┘                     │
                     │                                │
          ┌─────────┴─────────┐                      │
          │                   │                      │
          ▼                   ▼                      │
      Phase 5             Phase 6                    │
        (US3)               (US4)                    │
         P2                  P2                      │
          │                   │                      │
          └─────────┬─────────┘                      │
                    │                                │
                    ▼                                │
                Phase 7 ◄────────────────────────────┘
                  (US5)
                   P3
                    │
                    ▼
              Phase 8 (Polish)
```

### User Story Dependencies

| Story | Depends On | Can Start After |
|-------|-----------|-----------------|
| US1 (P1) | Foundational | Phase 2 complete |
| US2 (P1) | Foundational | Phase 2 complete |
| US3 (P2) | US1, US2 | Phase 3 & 4 complete |
| US4 (P2) | US3 | Phase 5 complete |
| US5 (P3) | Foundational | Phase 2 complete (but typically after US4) |

### Parallel Opportunities

**Within Phase 1 (Setup)**:
- T002, T003, T004, T005, T006 can all run in parallel

**Within Phase 2 (Foundational)**:
- T008, T010 can run in parallel

**US1 & US2 (P1) can run in parallel after Foundational**:
- Different teams can work on Wasm Engine (US1) and Storage Pallet verification (US2) simultaneously

**Within each User Story**:
- All [P] tasks can run in parallel
- Tests should be written first (TDD)

---

## Parallel Example: User Story 1

```bash
# Worker 1: Wasm Engine Tests
T012, T013, T014, T015, T016  # all [P] - run in parallel

# Worker 2: Pallet Tests
T017                           # can run while Worker 1 runs

# Worker 3: Integration Test Setup
T018                           # prepare E2E test infrastructure

# After tests written and failing:
# Implementation (sequential within story)
T019 → T020 → T021 → T022 → T023 → T024 → T025 → T026
```

---

## Summary

| Phase | Tasks | Story | Priority | Est. Effort |
|-------|-------|-------|----------|-------------|
| 1. Setup | T001-T006 | — | — | 1 day |
| 2. Foundational | T007-T011 | — | — | 2 days |
| 3. US1 | T012-T026 | 投稿の暗号学的断片化 | P1 | 5 days |
| 4. US2 | T027-T042 | 保持証明の提出と検証 | P1 | 5 days |
| 5. US3 | T043-T050, T075-T076 | 保持報酬の分配 | P2 | 3 days |
| 6. US4 | T051-T060, T077 | スコア閾値による自然な忘却 | P2 | 3 days |
| 7. US5 | T061-T067 | スコアベースの報酬制御 | P3 | 2 days |
| 8. Polish | T068-T074, T078-T080 | — | — | 3 days |
| **Total** | **80 tasks** | **5 stories** | — | **~24 days** |

**Test Tasks**: 37 tests (46% of total) — ensures comprehensive coverage

**MVP Scope**: Phase 1-4 (Setup + Foundational + US1 + US2) = ~13 days

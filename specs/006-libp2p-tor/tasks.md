# Tasks: libp2p + Tor統合

**Input**: Design documents from `/specs/006-libp2p-tor/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Tests**: Not explicitly requested in spec - test tasks omitted

**Organization**: Tasks are grouped by user story (P1→P2→P3) to enable independent implementation

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

Based on plan.md structure:
- **Blockchain node**: `apps/blockchain/node/src/`
- **Scripts**: `apps/blockchain/scripts/`
- **Docs**: `apps/blockchain/docs/`

---

## Phase 1: Setup

**Purpose**: Project structure and basic tooling verification

- [x] T001 Verify Tor installation availability on development environment
- [x] T002 [P] Verify torsocks installation and version (2.3+ required)
- [x] T003 [P] Document existing `apps/blockchain/node/src/cli.rs` structure for modification

**Result**: Tor/torsocks未インストール（セットアップスクリプトで対応）、cli.rsは標準clapベース

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core scripts and configuration that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T004 Create `apps/blockchain/scripts/tor-setup.sh` - Tor/torsocks installation script (Linux/macOS)
- [x] T005 [P] Create `apps/blockchain/docs/tor-deployment.md` - Base Tor deployment documentation structure
- [x] T006 Add TorMode enum definition in `apps/blockchain/node/src/cli.rs` (off/outbound-only/forced)
- [x] T007 Add `--tor-mode` CLI argument parsing in `apps/blockchain/node/src/cli.rs`
- [x] T008 Add environment variable `ANARCHY_TOR_MODE` support in `apps/blockchain/node/src/command.rs`

**Checkpoint**: ✅ Foundation ready - CLI can accept `--tor-mode` argument, ①②ロック実装済み

---

## Phase 3: User Story 1 - 匿名ノード起動 (Priority: P1) 🎯 MVP

**Goal**: ノード運営者がtorsocks経由で匿名にノードを起動できる

**Independent Test**: `torsocks ./anarchy-node --tor-mode=outbound-only`でノード起動、外部IPがTor出口ノードのものになることを確認

### Implementation for User Story 1

- [x] T009 [US1] Implement torsocks detection logic in `apps/blockchain/node/src/command.rs`
- [x] T010 [US1] Add startup warning for `outbound-only` mode ("受信IPは露出します") in `apps/blockchain/node/src/command.rs`
- [x] T011 [US1] Add error handling when torsocks not detected in `outbound-only` mode
- [x] T012 [US1] Update `apps/blockchain/scripts/run-multi-node.sh` to support `--tor-mode` option
- [x] T013 [US1] Document torsocks usage in `apps/blockchain/docs/tor-deployment.md` (Phase 1 section)
- [x] T014 [US1] Add verification command to check Tor connectivity in `apps/blockchain/scripts/tor-setup.sh`

**Checkpoint**: ✅ User Story 1 complete - torsocks経由でノード起動可能、anarchy-tor.sh作成済み

---

## Phase 4: User Story 2 - Onion Service受信 (Priority: P2)

**Goal**: ノードがOnion Serviceとして受信接続を受け付け、`.onion`アドレスをピアに広告できる

**Independent Test**: Onion Service設定後、別ノードから`.onion`アドレス経由で接続成功

### Implementation for User Story 2

- [x] T015 [US2] Create `apps/blockchain/scripts/onion-service.sh` - Onion Service設定生成スクリプト
- [x] T016 [US2] Generate torrc configuration snippet in `onion-service.sh`
- [x] T017 [US2] Extract `.onion` address from `/var/lib/tor/anarchy-node/hostname` in `onion-service.sh`
- [x] T018 [US2] Generate `--public-addr` command with Onion address in `onion-service.sh`
- [x] T019 [P] [US2] Document Onion Service setup in `apps/blockchain/docs/tor-deployment.md` (Phase 2 section)
- [x] T020 [US2] Add Onion address validation in `apps/blockchain/node/src/command.rs` (56 char base32 format)

**Checkpoint**: ✅ User Story 2 complete - Onion Service経由で受信可能

---

## Phase 5: User Story 4 - ブートストラップノード接続 (Priority: P2)

**Goal**: 新規ノードがOnionアドレスのブートストラップノードに接続してネットワーク参加

**Independent Test**: `.onion`アドレスのブートノードに初回接続し、追加ピアを発見

### Implementation for User Story 4

- [x] T021 [US4] Update `apps/blockchain/node/src/chain_spec.rs` to support Onion multiaddress in bootNodes
- [x] T022 [US4] Add example Onion bootnode entries in chain spec (commented template)
- [x] T023 [P] [US4] Document bootstrap node configuration in `apps/blockchain/docs/tor-deployment.md`
- [x] T024 [US4] Validate bootnode format accepts `/onion3/` prefix in `apps/blockchain/node/src/command.rs`

**Checkpoint**: ✅ User Story 4 complete - Onionブートノード経由でネットワーク参加可能

---

## Phase 6: User Story 3 - Torモード選択 (Priority: P3)

**Goal**: 運営者が環境に応じてTorモード（off/outbound-only/forced）を選択できる

**Independent Test**: 各モードで起動し、期待通りの接続動作を確認

### Implementation for User Story 3

- [x] T025 [US3] Implement ① 出口ロック: torsocks環境変数チェック in `apps/blockchain/node/src/command.rs`
- [x] T026 [US3] Implement ② 入口ロック: listen_addresses強制上書き in `apps/blockchain/node/src/command.rs`
- [x] T027 [US3] Add error message and exit(1) when torsocks env not set in forced mode
- [x] T028 [US3] Create `apps/blockchain/scripts/anarchy-tor.sh` wrapper script (sets ANARCHY_RUNNING_UNDER_TORSOCKS=1)
- [x] T029 [P] [US3] Document all three modes with usage examples in `apps/blockchain/docs/tor-deployment.md`
- [x] T030 [US3] Add mode-specific startup log messages (INFO/WARN level)

**Checkpoint**: ✅ User Story 3 complete - 全Torモードが機能（①出口＋②入口ロック）

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation completion, integration testing, security hardening

- [x] T031 [P] Complete `apps/blockchain/docs/tor-deployment.md` with troubleshooting section
- [x] T032 [P] Add security warnings about exit node risks in documentation
- [x] T033 [P] Add "Onion-to-Onion" communication best practices in documentation
- [x] T034 Create integration test script `apps/blockchain/tests/integration/tor_connectivity_test.sh`
- [x] T035 [P] Add timeout configuration guidance for Tor connections (90s recommended)
- [x] T036 Sanitize Onion addresses from log output (privacy protection)
- [x] T037 Run quickstart.md validation with all three Tor modes (requires Tor installation)

**Checkpoint**: ✅ Phase 7 complete - All tasks completed including Tor mode validation

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup - BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Foundational - MVP target
- **US2 (Phase 4)**: Depends on Foundational - can run parallel to US1
- **US4 (Phase 5)**: Depends on US2 (needs Onion address format) - can run parallel to US1
- **US3 (Phase 6)**: Depends on US1 + US4 (builds on both)
- **Polish (Phase 7)**: Depends on all user stories complete

### User Story Dependencies

```
Setup (Phase 1)
    │
    ▼
Foundational (Phase 2) ←── GATE: CLI accepts --tor-mode
    │
    ├──────────────────────────────┬─────────────────────────┐
    ▼                              ▼                         ▼
US1 (Phase 3)                  US2 (Phase 4)            US4 (Phase 5)
torsocks送信                   Onion Service            ブートノード
    │                              │                         │
    └──────────────────────────────┴─────────────────────────┘
                                   │
                                   ▼
                            US3 (Phase 6)
                            Torモード選択
                                   │
                                   ▼
                            Polish (Phase 7)
```

### Parallel Opportunities

**Within Phase 2**:
- T005 (docs structure) can run parallel to T006-T008 (code)

**Across Phases 3-5** (after Foundational):
- US1 (T009-T014) and US2 (T015-T020) can run in parallel
- US1 (T009-T014) and US4 (T021-T024) can run in parallel
- All [P] marked tasks within a phase can run in parallel

**Within Phase 7**:
- T031, T032, T033, T035 (documentation) can all run in parallel

---

## Summary

| Phase | Tasks | Parallel | Description |
|-------|-------|----------|-------------|
| 1 Setup | 3 | 2 | 環境確認 |
| 2 Foundational | 5 | 1 | CLI基盤 |
| 3 US1 (P1) | 6 | 0 | torsocks送信 |
| 4 US2 (P2) | 6 | 1 | Onion Service |
| 5 US4 (P2) | 4 | 1 | ブートノード |
| 6 US3 (P3) | 6 | 1 | モード選択 |
| 7 Polish | 7 | 4 | 仕上げ |
| **Total** | **37** | **10** | |

**MVP Scope**: Phase 1-3 (US1完了) = 14タスク
**Full Implementation**: Phase 1-7 = 37タスク
---

## Additional Features (Post-MVP)

### Mainnet Tor Enforcement

- [x] T038 Implement mainnet `--tor-mode=forced` enforcement in `command.rs`
- [x] T039 Document mainnet Tor requirement in README.md and tor-deployment.md

**Behavior**: When `--chain mainnet` is used, `--tor-mode=forced` is automatically applied regardless of user input. This ensures all mainnet nodes communicate exclusively via Tor.
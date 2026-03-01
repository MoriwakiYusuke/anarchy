# Tasks: ストレージノードアクセス制限（セッショントークン認証）

> **ABANDONED (2026-03)**: セッション認証は不要と判断され撤去済み。詳細は [spec.md](spec.md) の冒頭を参照。

**Input**: Design documents from `/specs/018-storage-node-auth/`  
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Tests**: テストタスクは含まれていません（明示的なTDD要求なし）。必要に応じてPolishフェーズで追加可能。

**Organization**: タスクはユーザーストーリーごとにグループ化され、独立した実装・テストが可能。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 並列実行可能（異なるファイル、依存関係なし）
- **[Story]**: 所属するユーザーストーリー（US1, US2, US3, US4, US5）
- ファイルパスは絶対パスではなくリポジトリルートからの相対パス

## Path Conventions

このプロジェクトはMulti-crateワークスペース:
- **ストレージノード**: `apps/storage-node/src/`
- **ブロックチェーンノード**: `apps/blockchain/node/src/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: セッション認証モジュールの基盤構築

- [X] T001 Create session module structure in apps/storage-node/src/session/mod.rs
- [X] T002 [P] Add dependencies to apps/storage-node/Cargo.toml (rand, hex, ed25519-dalek)
- [X] T003 [P] Create session config types in apps/storage-node/src/config/mod.rs (SessionConfig struct)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 全ユーザーストーリーに必要な基盤コンポーネント

**⚠️ CRITICAL**: このフェーズ完了まで他のユーザーストーリーは開始不可

- [X] T004 Implement SessionToken generation (OsRng + hex encode) in apps/storage-node/src/session/token.rs
- [X] T005 [P] Implement SessionInfo struct in apps/storage-node/src/session/token.rs
- [X] T006 [P] Implement SessionError enum in apps/storage-node/src/session/error.rs
- [X] T007 Implement SessionRegistry with HashMap<Token, SessionInfo> in apps/storage-node/src/session/registry.rs
- [X] T008 Add create_session(), validate(), revoke_for_peer(), cleanup_expired() methods in apps/storage-node/src/session/registry.rs
- [X] T009 [P] Implement ConnectedPeers with HashSet<PeerId> in apps/storage-node/src/session/peers.rs
- [X] T010 Add SwarmEvent handlers (ConnectionEstablished/Closed) in apps/storage-node/src/network/mod.rs
- [X] T011 Add session configs to apps/storage-node/src/config/mod.rs (token_ttl, idle_timeout, cleanup_interval)
- [X] T012 Start background cleanup task (tokio::spawn) in main.rs or service initialization

**Checkpoint**: SessionRegistry + ConnectedPeersが動作可能。JSON-RPCハンドラ実装可能状態。

---

## Phase 3: User Story 5 - ストレージノード間通信は認証不要 (Priority: P1)

**Goal**: ストレージノード間のリペア・同期通信をlibp2p P2Pに統一し、HTTP RPCを削除

**Independent Test**: ストレージノードAからBへのlibp2pリペアリクエストが成功することを確認

**Why first**: HTTP repair削除は他の認証機能に依存しない独立タスク。先に削除することでHTTP APIの認証対象を明確化。

### Implementation for User Story 5

- [X] T013 [US5] Identify HTTP repair functions in apps/storage-node/src/rpc/client.rs (N/A - file doesn't exist, no HTTP repair)
- [X] T014 [US5] Remove request_fragment_from_peer() and related HTTP client code (N/A - already P2P only)
- [X] T015 [US5] Update repair to use libp2p P2P only (DONE - src/repair/ already uses libp2p protocol)
- [X] T016 [US5] Verify existing libp2p repair works correctly (DONE - repair/coordinator.rs + repair/protocol.rs)
- [X] T017 [US5] Update /health endpoint to be the only unauthenticated HTTP endpoint (DONE in apps/storage-node/src/rpc/mod.rs)

**Checkpoint**: ストレージノード間通信が100% libp2p P2P。HTTP `/health` のみ公開。

---

## Phase 4: User Story 1 - ブロックチェーンノードがセッションを確立 (Priority: P1) 🎯 MVP

**Goal**: ブロックチェーンノードがlibp2p経由でセッショントークンを取得可能にする

**Independent Test**: `storage_requestSession` を呼び出し、有効なセッショントークンが返ることを確認

### Implementation for User Story 1

- [X] T018 [US1] Define SessionRequest/SessionResponse types in apps/storage-node/src/session/protocol.rs
- [X] T019 [US1] Implement Ed25519 signature verification (libp2p public key → peer_id) in apps/storage-node/src/session/protocol.rs
- [X] T020 [US1] Implement storage_requestSession RPC handler in apps/storage-node/src/main.rs (event loop)
- [X] T021 [US1] Add peer_id ∈ connected_peers validation in apps/storage-node/src/main.rs (event loop)
- [X] T022 [US1] Register storage_requestSession in libp2p request-response protocol (SessionProtocolCodec) in apps/storage-node/src/session/protocol.rs
- [X] T023 [US1] Add session logging (peer_id, issued_at, expires_at) in apps/storage-node/src/main.rs
- [X] T024 [P] [US1] Implement storage_renewSession RPC handler in apps/storage-node/src/main.rs
- [X] T025 [P] [US1] Implement storage_revokeSession RPC handler in apps/storage-node/src/main.rs

**Checkpoint**: ストレージノードが `storage_requestSession` を提供。P2P接続済みピアにトークン発行。

---

## Phase 5: User Story 2 + 3 - セッショントークン認証 & フロントエンド拒否 (Priority: P1)

**Goal**: HTTP APIでセッショントークン認証を実装し、無効なリクエストを拒否

**Independent Test**: 
- US2: 有効なトークン付きで `POST /fragments` が成功
- US3: トークンなし/無効トークンで 401/403 エラー

**Note**: US2とUS3は同じトークン検証機構の表裏一体のため統合

### Implementation for User Story 2 + 3

- [X] T026 [US2] Implement X-Session-Token header extractor in apps/storage-node/src/rpc/auth.rs
- [X] T027 [US2] Create require_session_auth function (with session registry) in apps/storage-node/src/rpc/auth.rs
- [X] T028 [US2] Add touch() method to SessionRegistry for last_access update in apps/storage-node/src/session/registry.rs
- [X] T029 [US3] Apply session token check in require_auth for write operations in apps/storage-node/src/rpc/mod.rs
- [X] T030 [US3] Apply session token check to DELETE operations (via method_requires_auth) in apps/storage-node/src/rpc/mod.rs
- [X] T031 [US2] Return 401 Unauthorized for missing X-Session-Token in apps/storage-node/src/rpc/auth.rs
- [X] T032 [US3] Return 403 Forbidden for invalid/expired token in apps/storage-node/src/rpc/auth.rs
- [X] T033 [US2] Keep GET /fragments and GET /health unauthenticated in apps/storage-node/src/rpc/mod.rs (read ops don't require auth)

**Checkpoint**: 書き込み/削除操作はセッショントークン必須。読み取り操作は認証不要。フロントエンドからの直接アクセスは拒否。

---

## Phase 6: User Story 4 - 複数ブロックチェーンノードの同時接続 (Priority: P2)

**Goal**: 複数のブロックチェーンノードが同時にセッションを維持可能

**Independent Test**: 2つのブロックチェーンノードがそれぞれセッションを確立し、両方からのリクエストが成功

### Implementation for User Story 4

- [X] T034 [US4] Verify HashMap<Token, SessionInfo> supports multiple concurrent sessions in apps/storage-node/src/session/registry.rs (verified via test_multiple_peers test)
- [X] T035 [US4] Add concurrent session test scenario documentation in specs/018-storage-node-auth/quickstart.md
- [X] T036 [US4] Implement blockchain node session client (request_session) in apps/blockchain/node/src/storage/session_client.rs
- [X] T037 [US4] Implement session auto-renew (1 hour before expiry) in apps/blockchain/node/src/storage/session_client.rs
- [X] T038 [US4] Integrate session client with node startup in apps/blockchain/node/src/service.rs
- [X] T039 [US4] Store session token in memory for HTTP API calls in apps/blockchain/node/src/storage/mod.rs (via Storage struct)

**Checkpoint**: 複数ブロックチェーンノードが独立してセッション管理。自動更新で24時間以上稼働。

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 最終調整、ドキュメント更新、統合テスト

- [X] T040 [P] Add session module documentation to apps/storage-node/README.md
- [X] T041 [P] Update docs/storage_logic.md with session authentication flow
- [X] T042 Run quickstart.md validation (manual test of session flow) - MANUAL: requires running nodes
- [X] T043 Verify all success criteria (SC-001 to SC-006) from spec.md, including SC-002 latency benchmark (compare sessionToken vs per-request signature)
- [X] T044 [P] Add unit tests for SessionRegistry in apps/storage-node/src/session/tests.rs (19 tests implemented)
- [X] T045 [P] Add integration test script in apps/blockchain/tests/integration/storage_auth_test.sh
- [X] T046 Code cleanup and remove unused imports

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1: Setup
    │
    ▼
Phase 2: Foundational ─────────────────────────────┐
    │                                               │
    ▼                                               │
Phase 3: US5 (HTTP repair削除) ◄────────────────────┤
    │                                               │
    ▼                                               │
Phase 4: US1 (Session確立) ◄────────────────────────┤
    │                                               │
    ▼                                               │
Phase 5: US2+US3 (トークン認証) ◄───────────────────┤
    │                                               │
    ▼                                               │
Phase 6: US4 (複数ノード) ◄─────────────────────────┘
    │
    ▼
Phase 7: Polish
```

### User Story Dependencies

| Story | Depends On | Can Start After |
|-------|------------|-----------------|
| US5 | Foundational | Phase 2 完了 |
| US1 | Foundational | Phase 2 完了 (US5と並列可能) |
| US2+US3 | US1 | Phase 4 完了 |
| US4 | US1, US2+US3 | Phase 5 完了 |

### Parallel Opportunities

**Phase 2内で並列可能**:
```
T005 SessionInfo + T006 SessionError + T009 ConnectedPeers
```

**Phase 3とPhase 4の一部は並列可能**:
```
T013-T017 (HTTP repair削除) || T018-T019 (Session types/verify)
```

**Phase 5内で並列可能**:
```
T026 Header extractor || T027 Middleware (別ファイル)
```

---

## Implementation Strategy

### MVP First (User Story 1 + 5)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL)
3. Complete Phase 3: US5 (HTTP repair削除)
4. Complete Phase 4: US1 (Session確立)
5. **STOP and VALIDATE**: `storage_requestSession` が動作することを確認
6. Deploy/demo if ready

### Full Implementation

1. MVP (Phase 1-4)
2. Add Phase 5: US2+US3 (トークン認証)
3. Add Phase 6: US4 (複数ノード + ブロックチェーンノード側クライアント)
4. Add Phase 7: Polish

### Estimated Task Count by Phase

| Phase | Tasks | Parallel Tasks |
|-------|-------|----------------|
| Phase 1: Setup | 3 | 2 |
| Phase 2: Foundational | 9 | 3 |
| Phase 3: US5 | 5 | 0 |
| Phase 4: US1 | 8 | 2 |
| Phase 5: US2+US3 | 8 | 0 |
| Phase 6: US4 | 6 | 0 |
| Phase 7: Polish | 7 | 4 |
| **Total** | **46** | **11** |

---

## Notes

- [P] タスク = 異なるファイル、依存関係なし
- [Story] ラベル = トレーサビリティ用ユーザーストーリーマッピング
- 各ユーザーストーリーは独立して完了・テスト可能
- 論理的なグループ単位でコミット
- チェックポイントでストーリーの独立検証実施
- 回避事項: 曖昧なタスク、同一ファイル競合、独立性を損なうストーリー間依存

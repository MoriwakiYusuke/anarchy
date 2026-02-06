# Tasks: Identity Pallet

**Input**: Design documents from `/specs/001-identity-pallet/`  
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Tests**: Included (Constitution principle VI. Test-First Development)

**Organization**: Tasks grouped by user story for independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- File paths relative to `apps/blockchain/`

---

## Phase 1: Setup (Pallet Scaffolding)

**Purpose**: Create Identity Pallet project structure

- [X] T001 Create pallet directory structure at pallets/identity/src/
- [X] T002 Create pallets/identity/Cargo.toml with dependencies (parity-scale-codec, scale-info, frame-support, frame-system, sp-runtime, sp-core)
- [X] T003 [P] Add pallet to workspace members in Cargo.toml

---

## Phase 2: Foundational (Core Types & Storage)

**Purpose**: Define types, storage, and configuration that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 Define Config trait with RuntimeEvent, MaxPasskeys, MaxPublicKeyLength, MaxDeviceNameLength in pallets/identity/src/lib.rs
- [X] T005 Define PasskeyId type alias ([u8; 32]) in pallets/identity/src/lib.rs
- [X] T006 [P] Define Passkey struct with id, public_key, registered_at, last_used_at, device_name in pallets/identity/src/lib.rs
- [X] T007 [P] Define Identity struct with created_at, passkeys (BoundedVec) in pallets/identity/src/lib.rs
- [X] T008 Implement derive_passkey_id() helper function using blake2_256 in pallets/identity/src/lib.rs
- [X] T009 Define Identities StorageMap (u64 → Identity) in pallets/identity/src/lib.rs
- [X] T010 [P] Define NextIdentityId StorageValue in pallets/identity/src/lib.rs
- [X] T011 [P] Define PasskeyOwner StorageMap (PasskeyId → u64) in pallets/identity/src/lib.rs
- [X] T012 Define Error enum (IdentityNotFound, PasskeyAlreadyRegistered, PasskeyNotFound, TooManyPasskeys, CannotRemoveLastPasskey, EmptyPublicKey, PublicKeyTooLong, Unauthorized) in pallets/identity/src/lib.rs
- [X] T013 Define Event enum (IdentityCreated, PasskeyAdded, PasskeyRemoved) in pallets/identity/src/lib.rs
- [X] T014 Create test mock runtime in pallets/identity/src/tests.rs

**Checkpoint**: Core types and storage ready - user story implementation can begin

---

## Phase 3: User Story 1 - 新規ユーザーがIdentityを作成する (Priority: P1) 🎯 MVP

**Goal**: WebAuthnパスキーでIdentityを作成し、オンチェーンに公開鍵を登録

**Independent Test**: `register_identity` extrinsic を呼び出し、Identities StorageにIdentityが作成されていることを確認

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T015 [P] [US1] Test register_identity_works: 正常にIdentity作成、イベント発行を検証 in pallets/identity/src/tests.rs
- [X] T016 [P] [US1] Test register_identity_empty_pubkey_fails: 空の公開鍵でエラー in pallets/identity/src/tests.rs
- [X] T017 [P] [US1] Test register_identity_pubkey_too_long_fails: 256バイト超過でエラー in pallets/identity/src/tests.rs
- [X] T018 [P] [US1] Test register_identity_duplicate_passkey_fails: 重複公開鍵でPasskeyAlreadyRegisteredエラー in pallets/identity/src/tests.rs

### Implementation for User Story 1

- [X] T019 [US1] Implement validate_public_key() helper function (empty check, length check) in pallets/identity/src/lib.rs
- [X] T020 [US1] Implement register_identity extrinsic in pallets/identity/src/lib.rs:
  - public_key, device_name パラメータ受け取り
  - 公開鍵バリデーション
  - PasskeyId導出
  - PasskeyOwner重複チェック
  - Identity構造体作成
  - Storage更新 (Identities, NextIdentityId, PasskeyOwner)
  - IdentityCreatedイベント発行
- [X] T021 [US1] Run US1 tests and verify all pass with `cargo test -p pallet-identity register_identity`

**Checkpoint**: User Story 1 complete - Identity作成機能が動作

---

## Phase 4: User Story 2 - 既存ユーザーが新しいデバイスを追加する (Priority: P2)

**Goal**: 既存IdentityにPasskey（デバイス）を追加登録

**Independent Test**: 既存Identityに `add_passkey` を呼び出し、passkeys配列に2つ目のPasskeyが追加されていることを確認

### Tests for User Story 2

- [X] T022 [P] [US2] Test add_passkey_works: 正常にPasskey追加、イベント発行を検証 in pallets/identity/src/tests.rs
- [X] T023 [P] [US2] Test add_passkey_identity_not_found: 存在しないIdentityでエラー in pallets/identity/src/tests.rs
- [X] T024 [P] [US2] Test add_passkey_duplicate_fails: 既に登録済み公開鍵でエラー in pallets/identity/src/tests.rs
- [X] T025 [P] [US2] Test add_passkey_max_limit: MaxPasskeys(10)超過でTooManyPasskeysエラー in pallets/identity/src/tests.rs

### Implementation for User Story 2

- [X] T026 [US2] Implement add_passkey extrinsic in pallets/identity/src/lib.rs:
  - identity_id, public_key, device_name パラメータ受け取り
  - Identity存在チェック
  - 公開鍵バリデーション
  - PasskeyOwner重複チェック
  - MaxPasskeys上限チェック
  - Passkey構造体作成・追加
  - Storage更新
  - PasskeyAddedイベント発行
- [X] T027 [US2] Run US2 tests and verify all pass with `cargo test -p pallet-identity add_passkey`

**Checkpoint**: User Story 2 complete - マルチデバイス対応が動作

---

## Phase 5: User Story 3 - ユーザーがデバイスを削除する (Priority: P3)

**Goal**: Identityから不要なPasskey（デバイス）を削除

**Independent Test**: 2台登録済みの状態から `remove_passkey` を呼び出し、passkeys配列から1つ削除されていることを確認

### Tests for User Story 3

- [X] T028 [P] [US3] Test remove_passkey_works: 正常にPasskey削除、イベント発行を検証 in pallets/identity/src/tests.rs
- [X] T029 [P] [US3] Test remove_passkey_not_found: 存在しないPasskeyでエラー in pallets/identity/src/tests.rs
- [X] T030 [P] [US3] Test remove_last_passkey_fails: 最後の1つ削除でCannotRemoveLastPasskeyエラー in pallets/identity/src/tests.rs

### Implementation for User Story 3

- [X] T031 [US3] Implement remove_passkey extrinsic in pallets/identity/src/lib.rs:
  - identity_id, passkey_id パラメータ受け取り
  - Identity存在チェック
  - Passkey存在チェック
  - 最後のPasskey削除防止チェック
  - passkeys配列から削除
  - PasskeyOwner削除
  - Storage更新
  - PasskeyRemovedイベント発行
- [X] T032 [US3] Run US3 tests and verify all pass with `cargo test -p pallet-identity remove_passkey`

**Checkpoint**: User Story 3 complete - デバイス削除機能が動作

---

## Phase 6: Polish & Integration

**Purpose**: Runtime統合、ドキュメント更新、全体テスト

- [X] T033 Add pallet-identity dependency to runtime/Cargo.toml
- [X] T034 Implement Config for Runtime in runtime/src/lib.rs (MaxPasskeys=10, MaxPublicKeyLength=256, MaxDeviceNameLength=64)
- [X] T035 Add Identity pallet to construct_runtime! macro in runtime/src/lib.rs
- [X] T036 Run full test suite with `cargo test -p pallet-identity`
- [X] T037 Build runtime with `cargo build --release -p anarchy-runtime`
- [X] T038 [P] Update docs/TODO.md with Identity Pallet completion status
- [X] T039 Run quickstart.md validation: ノード起動、PAPIでIdentity作成確認

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1: Setup
    ↓
Phase 2: Foundational (BLOCKS all user stories)
    ↓
┌───────────────┬───────────────┬───────────────┐
│ Phase 3: US1  │ Phase 4: US2  │ Phase 5: US3  │
│ (P1 - MVP)    │ (P2)          │ (P3)          │
└───────────────┴───────────────┴───────────────┘
         ↓ (after desired stories complete)
              Phase 6: Polish
```

### User Story Dependencies

| User Story | Depends On | Can Start After |
|------------|------------|-----------------|
| US1 (Identity作成) | Phase 2 | Phase 2 完了時 |
| US2 (デバイス追加) | Phase 2 + US1機能 | Phase 3 完了時を推奨 |
| US3 (デバイス削除) | Phase 2 + US1機能 | Phase 3 完了時を推奨 |

### Parallel Opportunities

**Phase 2 (Foundational)**:
```bash
# 並列実行可能グループA: 構造体定義
T006 & T007  # Passkey, Identity structs

# 並列実行可能グループB: Storage定義
T010 & T011  # NextIdentityId, PasskeyOwner
```

**Phase 3-5 (User Stories)**:
```bash
# 各User Story内のテストは並列実行可能
# US1: T015 & T016 & T017 & T018
# US2: T022 & T023 & T024 & T025
# US3: T028 & T029 & T030
```

---

## Implementation Strategy

### MVP First (推奨)

1. **Phase 1-2**: Setup + Foundational（必須）
2. **Phase 3 (US1)**: Identity作成 → **MVP完了** ✅
3. 実運用フィードバック収集
4. **Phase 4 (US2)**: マルチデバイス対応追加
5. **Phase 5 (US3)**: デバイス削除追加
6. **Phase 6**: 統合・ドキュメント

### Sequential (単独開発者向け)

```
T001 → T002 → T003 → T004 → ... → T039
```

---

## Summary

| Category | Count |
|----------|-------|
| Total Tasks | 39 |
| Setup Phase | 3 |
| Foundational Phase | 11 |
| User Story 1 | 7 tasks (4 tests + 3 impl) |
| User Story 2 | 6 tasks (4 tests + 2 impl) |
| User Story 3 | 5 tasks (3 tests + 2 impl) |
| Polish Phase | 7 |
| Parallel Opportunities | 19 tasks marked [P] |

**MVP Scope**: Phase 1 + Phase 2 + Phase 3 (US1) = **21 tasks**

**Format Validation**: ✅ All 39 tasks follow checklist format (checkbox, ID, labels, file paths)

# Tasks: WebAuthn署名検証

**Input**: Design documents from `/specs/002-webauthn-verification/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Tests**: Included per Constitution Principle VI (Test-First Development)

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Blockchain pallets**: `apps/blockchain/pallets/`
- **Identity Pallet**: `apps/blockchain/pallets/identity/src/`
- **Post Pallet**: `apps/blockchain/pallets/post/src/`

---

## Phase 1: Setup

**Purpose**: Add required dependencies for WebAuthn verification

- [X] T001 Add p256, ecdsa, sha2 dependencies in apps/blockchain/pallets/identity/Cargo.toml
- [X] T002 [P] Add base64 (no_std) dependency in apps/blockchain/pallets/identity/Cargo.toml
- [X] T003 [P] Update workspace Cargo.toml with new crate versions
- [X] T004 Update std features list in apps/blockchain/pallets/identity/Cargo.toml

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core types and COSE parser that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T005 Create WebAuthn type definitions in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T006 [P] Create COSE error types in apps/blockchain/pallets/identity/src/cose.rs
- [X] T007 [P] Create WebAuthn error types in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T008 Implement COSE public key parser (parse_cose_key) in apps/blockchain/pallets/identity/src/cose.rs
- [X] T009 Implement P-256 point validation (validate_public_key) in apps/blockchain/pallets/identity/src/cose.rs
- [X] T010 Add module declarations (mod cose; mod webauthn;) in apps/blockchain/pallets/identity/src/lib.rs
- [X] T011 Write COSE parser unit tests in apps/blockchain/pallets/identity/src/cose.rs

**Checkpoint**: Foundation ready - COSE parsing functional, user story implementation can begin

---

## Phase 3: User Story 1 - 投稿時のWebAuthn署名検証 (Priority: P1) 🎯 MVP

**Goal**: Users can create posts with WebAuthn signature verification, ensuring WYSIWYS

**Independent Test**: Create a post with valid/invalid WebAuthn signatures and verify acceptance/rejection

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T012 [P] [US1] Unit test for signature normalization (DER/raw) in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T013 [P] [US1] Unit test for verify_signature function in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T014 [P] [US1] Unit test for WYSIWYS challenge verification in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T015 [US1] Integration test for create_post_with_webauthn in apps/blockchain/pallets/post/src/tests.rs

### Implementation for User Story 1

- [X] T016 [US1] Implement normalize_signature (DER→raw conversion) in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T017 [US1] Implement verify_signature (ECDSA P-256) in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T018 [US1] Implement verify_wysiwys_challenge in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T019 [US1] Add WebAuthnSignatureData struct in apps/blockchain/pallets/post/src/lib.rs
- [X] T020 [US1] Implement create_post_with_webauthn extrinsic in apps/blockchain/pallets/post/src/lib.rs
- [X] T021 [US1] Add PostCreatedWithWebAuthn event in apps/blockchain/pallets/post/src/lib.rs
- [X] T022 [US1] Add WebAuthn-related errors to Post Pallet in apps/blockchain/pallets/post/src/lib.rs
- [X] T023 [US1] Wire Identity Pallet's webauthn module to Post Pallet in apps/blockchain/pallets/post/Cargo.toml

**Checkpoint**: User Story 1 complete - posts can be created with WebAuthn signature verification

---

## Phase 4: User Story 2 - COSE公開鍵の解析と保存 (Priority: P2)

**Goal**: Users can register Identity with COSE-format WebAuthn public keys

**Independent Test**: Register identity with valid/invalid COSE keys and verify parsing/rejection

### Tests for User Story 2

- [ ] T024 [P] [US2] Unit test for ES256 COSE key parsing in apps/blockchain/pallets/identity/src/cose.rs
- [ ] T025 [P] [US2] Unit test for unsupported algorithm rejection in apps/blockchain/pallets/identity/src/cose.rs
- [ ] T026 [US2] Integration test for register_identity_with_webauthn in apps/blockchain/pallets/identity/src/tests.rs

### Implementation for User Story 2

- [ ] T027 [US2] Add new errors (InvalidCoseKey, UnsupportedAlgorithm, etc.) in apps/blockchain/pallets/identity/src/lib.rs
- [ ] T028 [US2] Add IdentityCreatedWithWebAuthn event in apps/blockchain/pallets/identity/src/lib.rs
- [ ] T029 [US2] Implement register_identity_with_webauthn extrinsic in apps/blockchain/pallets/identity/src/lib.rs
- [ ] T030 [US2] Extend add_passkey to validate COSE key in apps/blockchain/pallets/identity/src/lib.rs

**Checkpoint**: User Story 2 complete - identities can be registered with WebAuthn COSE keys

---

## Phase 5: User Story 3 - authenticatorDataとclientDataJSONの検証 (Priority: P3)

**Goal**: Enhanced security validation of WebAuthn data to prevent replay attacks and origin spoofing

**Independent Test**: Submit various authenticatorData/clientDataJSON patterns and verify validation

### Tests for User Story 3

- [X] T031 [P] [US3] Unit test for parse_authenticator_data in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T032 [P] [US3] Unit test for parse_client_data_json in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T033 [P] [US3] Unit test for rpIdHash validation in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T034 [US3] Unit test for userPresent flag validation in apps/blockchain/pallets/identity/src/webauthn.rs

### Implementation for User Story 3

- [X] T035 [US3] Implement parse_authenticator_data in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T036 [US3] Implement parse_client_data_json in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T037 [US3] Implement rpIdHash validation in apps/blockchain/pallets/identity/src/webauthn.rs
- [X] T038 [US3] Implement userPresent flag check in apps/blockchain/pallets/identity/src/webauthn.rs
- [ ] T039 [US3] Add RpId config constant in apps/blockchain/pallets/identity/src/lib.rs
- [ ] T040 [US3] Integrate enhanced validation into verify_signature in apps/blockchain/pallets/identity/src/webauthn.rs

**Checkpoint**: User Story 3 complete - full security validation enabled

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, cleanup, and validation

- [ ] T041 [P] Update Identity Pallet documentation comments in apps/blockchain/pallets/identity/src/lib.rs
- [ ] T042 [P] Update Post Pallet documentation comments in apps/blockchain/pallets/post/src/lib.rs
- [ ] T043 Add weight benchmarks for WebAuthn extrinsics in apps/blockchain/pallets/identity/
- [ ] T044 Run cargo test --workspace to validate all tests pass
- [ ] T045 Run quickstart.md validation steps

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - start immediately
- **Foundational (Phase 2)**: Depends on Setup - BLOCKS all user stories
- **User Stories (Phase 3-5)**: All depend on Foundational phase completion
  - Can proceed in parallel if staffed
  - Or sequentially in priority order (P1 → P2 → P3)
- **Polish (Phase 6)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational - Independent of US1
- **User Story 3 (P3)**: Can start after Foundational - Integrates with US1's verify_signature

### Within Each User Story

- Tests MUST be written and FAIL before implementation (Constitution VI)
- Type definitions before logic
- Core functions before extrinsics
- Story complete before moving to next priority

### Parallel Opportunities

- Setup tasks T002, T003 can run in parallel with T001
- Foundational tasks T006, T007 can run in parallel with T005
- All test tasks marked [P] within a story can run in parallel
- Different user stories can be worked on in parallel by different team members once Foundational is complete

---

## Parallel Example: Phase 2 Foundational

```bash
# Can run in parallel (different files):
Task T005: "Create WebAuthn type definitions in webauthn.rs"
Task T006: "Create COSE error types in cose.rs"
Task T007: "Create WebAuthn error types in webauthn.rs"
```

## Parallel Example: User Story 1 Tests

```bash
# Can run in parallel (different test functions):
Task T012: "Unit test for signature normalization"
Task T013: "Unit test for verify_signature"
Task T014: "Unit test for WYSIWYS challenge"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test WebAuthn signature verification independently
5. Deploy/demo if ready - users can now post with WebAuthn signatures

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. User Story 1 → MVP! Posts with WebAuthn verification
3. User Story 2 → Enhanced identity registration
4. User Story 3 → Full security validation
5. Each story adds security without breaking previous functionality

---

## Notes

- [P] tasks = different files, no dependencies on each other
- [Story] label maps task to specific user story (US1, US2, US3)
- Verify tests fail before implementing (Test-First per Constitution VI)
- Commit after each task or logical group
- All new modules require `#![cfg_attr(not(feature = "std"), no_std)]`
- Use `sp_std::vec::Vec` instead of `std::vec::Vec` for no_std compatibility

## Summary

| Metric | Value |
|--------|-------|
| Total Tasks | 45 |
| Phase 1 (Setup) | 4 |
| Phase 2 (Foundational) | 7 |
| Phase 3 (US1/P1) | 12 |
| Phase 4 (US2/P2) | 7 |
| Phase 5 (US3/P3) | 10 |
| Phase 6 (Polish) | 5 |
| Parallel Opportunities | 15 tasks marked [P] |
| MVP Scope | Setup + Foundational + US1 (23 tasks) |

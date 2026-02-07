# Tasks: フロントエンドWebAuthn統合

**Input**: Design documents from `/specs/003-frontend-webauthn/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Unit tests are included (TDD approach for utility functions, hooks).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Web app**: `apps/frontend/src/`
- Tests: `apps/frontend/src/__tests__/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization, dependencies, and test framework setup

- [X] T001 Install new dependencies: `pnpm add cbor-x @noble/hashes` in apps/frontend/
- [X] T002 [P] Install dev dependencies: `pnpm add -D vitest @testing-library/react @vitejs/plugin-react jsdom` in apps/frontend/
- [X] T003 [P] Create Vitest configuration in apps/frontend/vitest.config.ts
- [X] T004 [P] Create test setup file with WebAuthn API mocks in apps/frontend/src/__tests__/setup.ts

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core utilities that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Utility Functions

- [X] T005 [P] Create Base64URL encoding/decoding utilities in apps/frontend/src/utils/webauthn.ts
- [X] T006 [P] Implement COSE public key extraction from attestationObject in apps/frontend/src/utils/cose.ts
- [X] T007 [P] Implement WYSIWYS challenge generation (prefix + SHA-256 hash + suffix) in apps/frontend/src/utils/webauthn.ts
- [X] T008 [P] Implement derivePasskeyId (Blake2-256 hash of COSE key) in apps/frontend/src/utils/webauthn.ts

### Unit Tests for Utilities

- [X] T009 [P] Write tests for Base64URL utilities in apps/frontend/src/__tests__/webauthn.test.ts
- [X] T010 [P] Write tests for COSE key extraction in apps/frontend/src/__tests__/cose.test.ts
- [X] T011 [P] Write tests for challenge generation in apps/frontend/src/__tests__/webauthn.test.ts

### Feature Detection Hook

- [X] T012 Implement useWebAuthnSupport hook in apps/frontend/src/hooks/useWebAuthnSupport.ts
- [X] T013 [P] Write tests for useWebAuthnSupport in apps/frontend/src/__tests__/useWebAuthnSupport.test.ts

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - パスキー登録フロー (Priority: P1) 🎯 MVP

**Goal**: 新規ユーザーがパスキーでIdentityを作成できるようにする

**Independent Test**: ユーザーがパスキー登録ボタンをクリックし、生体認証/PINを完了すると、ブロックチェーン上にIdentityが作成され、UIに成功メッセージが表示される

### Tests for User Story 1

- [X] T014 [P] [US1] Write tests for useWebAuthnRegistration hook in apps/frontend/src/__tests__/useWebAuthnRegistration.test.ts

### Implementation for User Story 1

- [X] T015 [US1] Implement useWebAuthnRegistration hook in apps/frontend/src/hooks/useWebAuthnRegistration.ts
  - WebAuthn credentials.create() call
  - COSE public key extraction via cose.ts
  - register_identity extrinsic call via PAPI
  - Status management (idle → authenticating → extracting → submitting → confirming → success/error)

- [X] T016 [US1] Create PasskeyRegister component in apps/frontend/src/components/PasskeyRegister.tsx
  - 「パスキーで登録」ボタン
  - ローディング状態表示
  - 成功メッセージ（Identity ID表示）
  - エラーメッセージとリトライ機能

- [X] T017 [US1] Create WebAuthnGate component in apps/frontend/src/components/WebAuthnGate.tsx
  - WebAuthn非対応ブラウザ検出
  - Platform authenticator不在の警告
  - 子コンポーネントのゲート表示

- [X] T018 [US1] Add PasskeyRegister styles in apps/frontend/src/components/PasskeyRegister.module.css

**Checkpoint**: User Story 1 完了 - 新規ユーザーがパスキー登録可能

---

## Phase 4: User Story 2 - WebAuthn署名付き投稿 (Priority: P2)

**Goal**: 登録済みユーザーがパスキーで署名付き投稿を行う（WYSIWYS保証）

**Independent Test**: 投稿内容を入力し「署名して投稿」をクリック、パスキー認証を完了すると、WebAuthn署名付きの投稿がブロックチェーンに記録される

### Tests for User Story 2

- [X] T019 [P] [US2] Write tests for useWebAuthnSigning hook in apps/frontend/src/__tests__/useWebAuthnSigning.test.ts

### Implementation for User Story 2

- [X] T020 [US2] Implement useWebAuthnSigning hook in apps/frontend/src/hooks/useWebAuthnSigning.ts
  - WYSIWYS challenge generation via webauthn.ts
  - WebAuthn credentials.get() call
  - create_post_with_webauthn extrinsic call via PAPI
  - Status management (idle → hashing → authenticating → submitting → confirming → success/error)
  - estimateCost function integration with usePostCost

- [X] T021 [US2] Create PasskeySignPost component in apps/frontend/src/components/PasskeySignPost.tsx
  - 投稿フォーム（コンテンツ入力）
  - 「署名して投稿」ボタン
  - バイト数・コスト表示（既存のusePostCost統合）
  - ローディング状態表示
  - 成功・エラーメッセージ

- [X] T022 [US2] Add PasskeySignPost styles in apps/frontend/src/components/PasskeySignPost.module.css

- [X] T023 [US2] Update Timeline component to show WebAuthn-signed posts in apps/frontend/src/components/Timeline.tsx
  - WebAuthn署名付き投稿の表示（アイコン/バッジ）

**Checkpoint**: User Stories 1 AND 2 完了 - 登録・署名投稿フローが動作

---

## Phase 5: User Story 3 - マルチデバイス対応（パスキー追加） (Priority: P3)

**Goal**: 登録済みユーザーが別デバイスのパスキーを追加し、複数デバイスからアクセス可能にする

**Independent Test**: 既存Identity保持者が新しいデバイスでパスキー追加を行い、そのデバイスからも投稿できることを確認

### Tests for User Story 3

- [X] T024 [P] [US3] Write tests for addPasskey function in apps/frontend/src/__tests__/useWebAuthn.test.ts

### Implementation for User Story 3

- [X] T025 [US3] Implement useWebAuthn統合フック in apps/frontend/src/hooks/useWebAuthn.ts
  - useWebAuthnSupport統合
  - useWebAuthnRegistration統合
  - useWebAuthnSigning統合
  - addPasskey function（既存Identityへのパスキー追加）
  - loadIdentityById function

- [X] T026 [US3] Create WebAuthnContext in apps/frontend/src/contexts/WebAuthnContext.tsx
  - グローバル状態管理
  - LocalStorage永続化（lastIdentityId, credentialIds）
  - Provider component

- [X] T027 [US3] Create settings/device management UI in apps/frontend/src/components/DeviceSettings.tsx
  - 登録済みパスキー一覧表示
  - 「デバイスを追加」ボタン
  - パスキー削除（将来対応）

- [X] T028 [US3] Add DeviceSettings styles in apps/frontend/src/components/DeviceSettings.module.css

**Checkpoint**: All user stories 完了 - マルチデバイス対応

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Integration, testing, and final polish

### Main Page Integration

- [X] T029 Update main page to use WebAuthn components in apps/frontend/src/app/page.tsx
  - WebAuthnGateでラップ
  - 未登録時: PasskeyRegister表示
  - 登録済み: PasskeySignPost表示
  - WebAuthnContextプロバイダー設定

- [X] T030 [P] Update layout with WebAuthnProvider in apps/frontend/src/app/layout.tsx
  - Note: WebAuthnProvider integrated in page.tsx instead

### E2E Testing (Playwright)

- [X] T031 [P] Create Playwright configuration in apps/frontend/playwright.config.ts
- [X] T032 [P] Create E2E test for registration flow in apps/frontend/e2e/registration.spec.ts
  - Virtual Authenticator setup
  - 登録フロー全体のテスト

- [X] T033 [P] Create E2E test for signing flow in apps/frontend/e2e/signing.spec.ts
  - WYSIWYS署名付き投稿のテスト

### Documentation

- [X] T034 [P] Update README with WebAuthn setup instructions in apps/frontend/README.md
- [X] T035 Run quickstart.md validation and update if needed

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can proceed in priority order (P1 → P2 → P3)
  - US2 benefits from US1 completion (Identity needed for signing)
  - US3 integrates US1 and US2 hooks
- **Polish (Phase 6)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Depends on US1 (requires Identity for signing)
- **User Story 3 (P3)**: Integrates US1 and US2, can start after both

### Within Each User Story

- Tests SHOULD be written and FAIL before implementation (TDD)
- Hooks before components
- Core implementation before styling
- Story complete before moving to next priority

### Parallel Opportunities

```
Phase 1 (Setup):
  T001 ─┬─ T002 ─┬─ T003
        │        └─ T004
        └────────────────→ Phase 2

Phase 2 (Foundational):
  T005 ─┬─ T009
  T006 ─┼─ T010
  T007 ─┤
  T008 ─┘
        └──→ T012 → T013 ──→ Phase 3

Phase 3+ (User Stories):
  US1: T014 → T015 → T016 → T017 → T018
  US2: T019 → T020 → T021 → T022 → T023  (after US1)
  US3: T024 → T025 → T026 → T027 → T028  (after US2)

Phase 6 (Polish):
  T029 → T030
  T031 ─┬─ T032
        └─ T033
  T034
  T035
```

---

## Summary

| Phase | Task Count | Purpose |
|-------|------------|---------|
| Phase 1: Setup | 4 | Dependencies & test framework |
| Phase 2: Foundational | 9 | Utilities & feature detection |
| Phase 3: US1 (P1) | 5 | パスキー登録 |
| Phase 4: US2 (P2) | 5 | 署名付き投稿 |
| Phase 5: US3 (P3) | 5 | マルチデバイス |
| Phase 6: Polish | 7 | Integration & E2E |
| **Total** | **35** | |

**MVP Scope**: Phase 1 + Phase 2 + Phase 3 (US1) = 18 tasks
**Full Implementation**: All 35 tasks

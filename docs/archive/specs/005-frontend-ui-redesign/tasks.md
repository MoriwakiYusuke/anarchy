# Tasks: Frontend UI Redesign

**Input**: Design documents from `/specs/005-frontend-ui-redesign/`  
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/ ✓

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

Based on plan.md structure:
- **Frontend**: `apps/frontend/src/`
- **Tests**: `apps/frontend/tests/` (to be created)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and directory structure

- [x] T001 Create i18n directory structure at apps/frontend/src/i18n/
- [x] T002 Create lib/matrix directory at apps/frontend/src/lib/matrix/
- [x] T003 [P] Create tests directory structure at apps/frontend/tests/
- [x] T004 [P] Add Matrix color variables to apps/frontend/src/app/globals.css

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core types and utilities that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T005 Create i18n type definitions in apps/frontend/src/i18n/types.ts (Locale, TranslationKey, TranslationMap)
- [x] T006 [P] Create matrix type definitions in apps/frontend/src/lib/matrix/types.ts (MatrixConfig, MatrixColumn)
- [x] T007 [P] Create matrix config constants in apps/frontend/src/lib/matrix/config.ts (DEFAULT_MATRIX_CONFIG)
- [x] T008 Implement useReducedMotion hook in apps/frontend/src/hooks/useReducedMotion.ts

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - 多言語対応 (Priority: P1) 🎯 MVP

**Goal**: 日本語・中国語・英語を母語とするユーザーが自分の言語でAnarchyを使用できる

**Independent Test**: 言語切り替え機能のみで、UIテキストが正しく切り替わることを確認

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T009 [P] [US1] Unit test for useLocale hook in apps/frontend/tests/hooks/useLocale.test.ts
- [x] T010 [P] [US1] Unit test for LanguageSwitcher in apps/frontend/tests/components/LanguageSwitcher.test.tsx

### Implementation for User Story 1

- [x] T011 [P] [US1] Create English translations in apps/frontend/src/i18n/translations/en.json
- [x] T012 [P] [US1] Create Japanese translations in apps/frontend/src/i18n/translations/ja.json
- [x] T013 [P] [US1] Create Chinese translations in apps/frontend/src/i18n/translations/zh.json
- [x] T014 [US1] Implement LocaleContext provider in apps/frontend/src/i18n/context.tsx
- [x] T015 [US1] Create i18n exports barrel file in apps/frontend/src/i18n/index.ts
- [x] T016 [US1] Implement useLocale hook in apps/frontend/src/hooks/useLocale.ts
- [x] T017 [US1] Implement LanguageSwitcher component in apps/frontend/src/components/LanguageSwitcher.tsx
- [x] T018 [P] [US1] Create LanguageSwitcher styles in apps/frontend/src/components/LanguageSwitcher.module.css
- [x] T019 [US1] Integrate LocaleProvider into apps/frontend/src/app/layout.tsx
- [x] T020 [US1] Internationalize WalletConnect component in apps/frontend/src/components/WalletConnect.tsx
- [x] T021 [US1] Internationalize PostForm component in apps/frontend/src/components/PostForm.tsx
- [x] T022 [US1] Internationalize Timeline component in apps/frontend/src/components/Timeline.tsx

**Checkpoint**: User Story 1 complete - 言語切替が機能し、全UI要素が翻訳される

---

## Phase 4: User Story 2 - cMatrix背景体験 (Priority: P2)

**Goal**: cMatrixスタイルの文字落下アニメーションをBlood Glitchテーマで背景に表示

**Independent Test**: 背景コンポーネントのみ実装し、アニメーションが動作することを確認

### Tests for User Story 2

- [x] T023 [P] [US2] Unit test for MatrixBackground in apps/frontend/tests/components/MatrixBackground.test.tsx
- [x] T024 [P] [US2] Unit test for matrix engine in apps/frontend/tests/lib/matrix.test.ts

### Implementation for User Story 2

- [x] T025 [US2] Implement matrix animation engine in apps/frontend/src/lib/matrix/index.ts
- [x] T026 [US2] Implement MatrixBackground component in apps/frontend/src/components/MatrixBackground.tsx
- [x] T027 [P] [US2] Create MatrixBackground styles in apps/frontend/src/components/MatrixBackground.module.css
- [x] T028 [US2] Integrate MatrixBackground into apps/frontend/src/app/layout.tsx
- [x] T029 [US2] Verify content readability with background animation active

**Checkpoint**: User Story 2 complete - 背景にMatrix風アニメーションが表示され、Blood Glitch効果が発生

---

## Phase 5: User Story 3 - 視覚アクセシビリティ対応 (Priority: P3)

**Goal**: 動きに敏感なユーザーが背景アニメーションを無効化できる

**Independent Test**: prefers-reduced-motion設定でアニメーションが停止することを確認

### Tests for User Story 3

- [x] T030 [P] [US3] Unit test for useReducedMotion in apps/frontend/tests/hooks/useReducedMotion.test.ts

### Implementation for User Story 3

- [x] T031 [US3] Connect useReducedMotion hook to MatrixBackground component
- [x] T032 [US3] Verify WCAG 2.1 AA contrast compliance between content and background

**Checkpoint**: User Story 3 complete - アクセシビリティ設定が尊重される

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final quality improvements

- [x] T033 [P] Update quickstart.md with final test scenarios in specs/005-frontend-ui-redesign/quickstart.md
- [x] T034 [P] Add feature documentation to docs/architecture.md
- [x] T035 Run full test suite and fix any failures
- [x] T036 Performance audit: verify 60fps animation and 500ms language switch
- [x] T037 Mobile device testing and optimization

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1 (Setup)
    │
    ▼
Phase 2 (Foundational) ◄── BLOCKING
    │
    ├──────────────────────────────────┐
    │                                  │
    ▼                                  ▼
Phase 3 (US1: i18n)        Phase 4 (US2: Matrix)
    │                           │         │
    │                           │         │
    └───────────────────────────┴─────────┘
                    │
                    ▼
            Phase 5 (US3: A11y)
                    │
                    ▼
            Phase 6 (Polish)
```

### User Story Dependencies

| Story | Depends On | Can Parallel With |
|-------|------------|-------------------|
| US1 (i18n) | Phase 2 | US2 |
| US2 (Matrix) | Phase 2, T008 (useReducedMotion) | US1 |
| US3 (A11y) | US2 (MatrixBackground) | - |

### Within Each User Story

1. Tests FIRST → implement → verify tests pass
2. Types/Models → Hooks/Services → Components → Integration

### Parallel Opportunities Per Phase

**Phase 1**: T003, T004 can run in parallel  
**Phase 2**: T006, T007 can run in parallel after T005  
**Phase 3**: T009-T010 (tests), T011-T013 (translations), T018 can all run in parallel  
**Phase 4**: T023, T024, T027 can run in parallel  
**Phase 5**: T030 can run immediately after Phase 2  
**Phase 6**: T033, T034 can run in parallel

---

## Parallel Example: User Story 1

```bash
# Parallel batch 1: Tests (write first, should fail)
T009 & T010 (parallel)

# Parallel batch 2: Translations
T011 & T012 & T013 (parallel)

# Sequential: Core implementation
T014 → T015 → T016 → T017

# Parallel batch 3: Styles
T018 (can run parallel with T017)

# Sequential: Integration
T019 → T020 → T021 → T022
```

---

## Implementation Strategy

### MVP Scope: User Story 1 Only

User Story 1（多言語対応）を完了すれば、国際ユーザーに価値を提供できる最小の機能セットとなる。

### Incremental Delivery

1. **Increment 1 (MVP)**: US1 - 3言語切替機能
2. **Increment 2**: US2 - cMatrix背景アニメーション
3. **Increment 3**: US3 - アクセシビリティ対応 + Polish

### Total Task Count

| Phase | Count |
|-------|-------|
| Setup | 4 |
| Foundational | 4 |
| User Story 1 | 14 |
| User Story 2 | 7 |
| User Story 3 | 3 |
| Polish | 5 |
| **Total** | **37** |

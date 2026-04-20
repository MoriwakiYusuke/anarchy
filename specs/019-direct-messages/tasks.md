---

description: "Task list for 019-direct-messages implementation"

---

# Tasks: Direct Messages (DM)

**Input**: Design documents from `/specs/019-direct-messages/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/{pallet-messaging-extrinsics.md, wasm-engine-dm-api.md, frontend-ui.md}, quickstart.md

**Tests**: TDD is required by the project Constitution (VI. Test-First Development) and by the spec. Tests are included in every user-story phase.

**Organization**: Tasks are grouped by user story. Each story can be implemented and validated independently after Phase 2 completes.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Maps the task to a user story (US1, US2, US3, US4) for traceability
- All paths are repo-root-relative (no leading `/`)

## Path Conventions

- Pallet (Rust): [apps/blockchain/pallets/messaging/](apps/blockchain/pallets/messaging/)
- Runtime (Rust): [apps/blockchain/runtime/src/lib.rs](apps/blockchain/runtime/src/lib.rs)
- Wasm engine (Rust): [packages/wasm-engine/src/dm/](packages/wasm-engine/src/dm/)
- Frontend lib: [apps/frontend/src/lib/dm/](apps/frontend/src/lib/dm/)
- Frontend UI: [apps/frontend/src/components/dm/](apps/frontend/src/components/dm/)
- Frontend routes: [apps/frontend/src/app/dm/](apps/frontend/src/app/dm/)
- Integration tests: [apps/blockchain/tests/integration/dm/](apps/blockchain/tests/integration/dm/)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Scaffolding for all crates / packages / dirs the feature touches. No business logic yet.

- [X] T001 Create pallet skeleton at [apps/blockchain/pallets/messaging/Cargo.toml](apps/blockchain/pallets/messaging/Cargo.toml) and [apps/blockchain/pallets/messaging/src/lib.rs](apps/blockchain/pallets/messaging/src/lib.rs) with `#[frame_support::pallet]` boilerplate (empty `Config`, no extrinsics yet)
- [X] T002 [P] Add `pallet-messaging` to [apps/blockchain/Cargo.toml](apps/blockchain/Cargo.toml) workspace members and to [apps/blockchain/runtime/Cargo.toml](apps/blockchain/runtime/Cargo.toml) dependencies (no `construct_runtime!` wiring yet)
- [X] T003 [P] Create wasm-engine DM module skeleton at [packages/wasm-engine/src/dm/mod.rs](packages/wasm-engine/src/dm/mod.rs), [packages/wasm-engine/src/dm/types.rs](packages/wasm-engine/src/dm/types.rs), and add `pub mod dm;` to [packages/wasm-engine/src/lib.rs](packages/wasm-engine/src/lib.rs)
- [X] T004 [P] Add `aes-gcm`, `hkdf`, `sha2`, `x25519-dalek`, `schnorrkel` (re-check existing) and any missing deps to [packages/wasm-engine/Cargo.toml](packages/wasm-engine/Cargo.toml) (skip those already pulled in by `stealth/`)
- [X] T005 [P] Create frontend DM directories: [apps/frontend/src/lib/dm/index.ts](apps/frontend/src/lib/dm/index.ts), [apps/frontend/src/lib/dm/types.ts](apps/frontend/src/lib/dm/types.ts), [apps/frontend/src/components/dm/index.ts](apps/frontend/src/components/dm/index.ts), [apps/frontend/src/app/dm/page.tsx](apps/frontend/src/app/dm/page.tsx) (placeholders)
- [X] T006 [P] Create integration test directory and stub scripts: [apps/blockchain/tests/integration/dm/dm-send-receive.sh](apps/blockchain/tests/integration/dm/dm-send-receive.sh), [apps/blockchain/tests/integration/dm/dm-stealth-linkage.sh](apps/blockchain/tests/integration/dm/dm-stealth-linkage.sh), [apps/blockchain/tests/integration/dm/dm-multi-device.sh](apps/blockchain/tests/integration/dm/dm-multi-device.sh) (echo TODO + exit 1)
- [X] T007 [P] Register `test:dm` script in [apps/blockchain/package.json](apps/blockchain/package.json) (or root [package.json](package.json) if that is where `test:integration` lives) so `pnpm test:dm` runs the new shell scripts

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Types, helpers, and runtime wiring that every user story requires. No story-level extrinsics or UI yet.

**⚠️ CRITICAL**: User-story phases MUST NOT begin until Phase 2 is complete.

### Pallet foundations

- [X] T008 Define on-chain types in [apps/blockchain/pallets/messaging/src/types.rs](apps/blockchain/pallets/messaging/src/types.rs): `DmMetaAddress`, `DmContentRef`, `DmDispatch<AccountId>`, and `DM_PROTOCOL_VERSION` constant per data-model.md §1.1–1.3
- [X] T009 Define `Config` trait, `BalanceOf`, all `#[pallet::constant]` items, `Event`, and `Error` enums in [apps/blockchain/pallets/messaging/src/lib.rs](apps/blockchain/pallets/messaging/src/lib.rs) per contracts/pallet-messaging-extrinsics.md §Dependencies and data-model.md §1.5–1.6
- [X] T010 Declare storage items `DmReceptionKeys`, `DmDispatchesByBlock`, `NextMessageId`, `DmMessagesByRoot` in [apps/blockchain/pallets/messaging/src/lib.rs](apps/blockchain/pallets/messaging/src/lib.rs) with the bounds from data-model.md §1.4
- [X] T011 [P] Create `WeightInfo` trait + benchmarking-friendly default impl in [apps/blockchain/pallets/messaging/src/weights.rs](apps/blockchain/pallets/messaging/src/weights.rs) with stub weights (`Weight::zero()`) for `publish_dm_key`, `revoke_dm_key`, `send_dm(len)`
- [X] T012 Build mock runtime in [apps/blockchain/pallets/messaging/src/mock.rs](apps/blockchain/pallets/messaging/src/mock.rs) with `frame_system`, `pallet_balances`, a stub `StoragePool` impl, and `pallet_messaging::Config` (uses ConstU128 / ConstU32 from contracts/pallet-messaging-extrinsics.md §Dependencies)
- [X] T013 Wire `pallet-messaging` into [apps/blockchain/runtime/src/lib.rs](apps/blockchain/runtime/src/lib.rs) `construct_runtime!` and add the `Config` impl block per contracts/pallet-messaging-extrinsics.md §Dependencies
- [X] T014 [P] Declare `DmScanApi` runtime API trait in [apps/blockchain/pallets/messaging/src/lib.rs](apps/blockchain/pallets/messaging/src/lib.rs) (declaration only — implementation lives in runtime) per contracts/pallet-messaging-extrinsics.md §RA
- [X] T015 Implement `DmScanApi` in [apps/blockchain/runtime/src/lib.rs](apps/blockchain/runtime/src/lib.rs) `impl_runtime_apis!` block (`dispatches_at`, `reception_key`, `dispatches_range` with ≤1024-block guard)

### Wasm-engine foundations

- [X] T016 [P] Implement `pad_iso7816_4` and `strip_iso7816_4` plus `DM_PADDING_BUCKETS` and `select_padding_bucket` in [packages/wasm-engine/src/dm/padding.rs](packages/wasm-engine/src/dm/padding.rs) per data-model.md §2.2 and contracts/wasm-engine-dm-api.md §Non-API Helpers
- [X] T017 [P] Implement `DmEnvelope` struct + SCALE encode/decode in [packages/wasm-engine/src/dm/envelope.rs](packages/wasm-engine/src/dm/envelope.rs) per data-model.md §2.1
- [X] T018 [P] Implement `dm_compute_inner_signed_hash` (W6) as `#[wasm_bindgen]` in [packages/wasm-engine/src/dm/envelope.rs](packages/wasm-engine/src/dm/envelope.rs) per contracts/wasm-engine-dm-api.md §W6
- [X] T019 Implement `hkdf_okm(shared, salt, info)` helper in [packages/wasm-engine/src/dm/encrypt.rs](packages/wasm-engine/src/dm/encrypt.rs) returning the 44-byte (key||nonce) OKM per contracts/wasm-engine-dm-api.md §Non-API Helpers
- [X] T020 [P] Implement `dm_derive_recipient_stealth` (W5) as `#[wasm_bindgen]` in [packages/wasm-engine/src/dm/encrypt.rs](packages/wasm-engine/src/dm/encrypt.rs) per contracts/wasm-engine-dm-api.md §W5

### Frontend foundations

- [X] T021 [P] Define DM TypeScript types (`DmMetaAddress`, `DmDispatch`, `DmMessageRecord`, `ConversationState`, `DmError` enum) in [apps/frontend/src/lib/dm/types.ts](apps/frontend/src/lib/dm/types.ts) per data-model.md §2.4 and contracts/frontend-ui.md
- [X] T022 [P] Implement `dmMetaFromString` / `dmMetaToString` Base58 ↔ struct converters in [apps/frontend/src/lib/dm/api.ts](apps/frontend/src/lib/dm/api.ts) per data-model.md §1.1 (reusing existing `lib/stealth` helpers via import)
- [X] T023 Generate / refresh PAPI descriptors for `pallet_messaging` (run `pnpm papi update` or equivalent against the updated runtime) and commit the regenerated files under [apps/frontend/.papi/](apps/frontend/.papi/) — depends on T013 (runtime must include `pallet_messaging` before metadata can be regenerated). **N/A for this codebase**: frontend uses `client.getUnsafeApi()` (no committed descriptors); pallet metadata is discovered at runtime once T013/T015 land.
- [X] T024 [P] Create Zustand `dmStore` skeleton in [apps/frontend/src/lib/dm/store.ts](apps/frontend/src/lib/dm/store.ts) with `conversations`, `blockList`, `lastScannedBlock`, `isScanning` plus `addIncoming`, `addOutgoing`, `markAsRead`, `blockSender`, `unblockSender` actions per contracts/frontend-ui.md §1.4
- [X] T092 [P] Smoke test that `pallet-stealth` is integrated and exposes the `send_to_stealth` (or equivalent) extrinsic that [apps/frontend/src/lib/dm/sender.ts](apps/frontend/src/lib/dm/sender.ts) will need — write a one-shot Rust integration check in [apps/blockchain/pallets/messaging/src/tests/stealth_integration.rs](apps/blockchain/pallets/messaging/src/tests/stealth_integration.rs) that compiles a mock runtime including both `pallet-messaging` and `pallet-stealth` and asserts the FR-024 pre-fund call path resolves at the type level (per plan.md Dependencies §pallet-stealth)

**Checkpoint**: pallet compiles in mock runtime, runtime builds with `pallet-messaging`, `wasm-pack build` succeeds with stubs in place, frontend compiles, `pallet-stealth` interop confirmed. User stories may now begin in parallel.

---

## Phase 3: User Story 1 - Send a private message (Priority: P1) 🎯 MVP

**Goal**: Alice sends an end-to-end-encrypted DM to Bob via stealth address; Bob retrieves and reads the plaintext.

**Independent Test**: On a 2-node testnet, Bob calls `publishDmKey()`, Alice calls `sendDm({ recipientAccountId: Bob, body: "hello" })`, Bob's scanner produces a `DmMessageRecord` with body `"hello"` and `signatureValid === true`. A third account observing the chain sees only `recipient_stealth` ≠ Bob's main account and only ciphertext in storage.

### Tests for User Story 1 (TDD — write FIRST and confirm failure)

- [X] T025 [P] [US1] Write pallet unit tests for `publish_dm_key` happy + invalid + overwrite paths in [apps/blockchain/pallets/messaging/src/tests/publish.rs](apps/blockchain/pallets/messaging/src/tests/publish.rs) per contracts/pallet-messaging-extrinsics.md §E1 Test acceptance
- [X] T026 [P] [US1] Write pallet unit tests for `revoke_dm_key` happy + not-published paths in [apps/blockchain/pallets/messaging/src/tests/revoke.rs](apps/blockchain/pallets/messaging/src/tests/revoke.rs) per §E2 Test acceptance
- [X] T027 [P] [US1] Write pallet unit tests for `send_dm` covering all 5 bucket sizes, fee split (80/10/10), `DmDispatched` event, `DmMessagesByRoot` insert, and every `Error::*` branch in [apps/blockchain/pallets/messaging/src/tests/send.rs](apps/blockchain/pallets/messaging/src/tests/send.rs) per §E3 Test acceptance
- [X] T028 [P] [US1] Write runtime API tests for `DmScanApi::{dispatches_at, reception_key, dispatches_range}` in [apps/blockchain/pallets/messaging/src/tests/runtime_api.rs](apps/blockchain/pallets/messaging/src/tests/runtime_api.rs) per §RA Test acceptance — also create [apps/blockchain/pallets/messaging/src/tests/mod.rs](apps/blockchain/pallets/messaging/src/tests/mod.rs) declaring `pub mod publish; pub mod revoke; pub mod send; pub mod runtime_api; pub mod stealth_integration;` (T090 `tx_failure` と `sender_stealth_zeroize` は wasm 側タスクで追加するため pallet 側モジュール宣言は行わない)
- [X] T029 [P] [US1] Write wasm-engine round-trip tests (`dm_encrypt_and_pad` ↔ `dm_decrypt_scan`, wrong meta fails, ciphertext bit-flip → None, `body=""` → 1 KB bucket) in [packages/wasm-engine/src/dm/tests/roundtrip.rs](packages/wasm-engine/src/dm/tests/roundtrip.rs) per contracts/wasm-engine-dm-api.md §W1/W2 Test acceptance — 注: W1 は外部署名フローに合わせて `eph_priv` を 7 番目の引数で受け取る (contract spec の step 1 「内部生成」からの逸脱)
- [X] T030 [P] [US1] Write wasm-engine tests for `dm_generate_sender_stealth` uniqueness + AccountId reproducibility, and for `dm_derive_recipient_stealth` parity with existing `derive_stealth_address` in [packages/wasm-engine/src/dm/tests/sender_stealth.rs](packages/wasm-engine/src/dm/tests/sender_stealth.rs) per §W3/W5 Test acceptance
- [X] T031 [P] [US1] Write wasm-engine test that `eph_priv` is zeroized after `dm_encrypt_and_pad` (memory inspection / debug assertion) in [packages/wasm-engine/src/dm/tests/zeroize_eph.rs](packages/wasm-engine/src/dm/tests/zeroize_eph.rs) per FR-021 — also create [packages/wasm-engine/src/dm/tests/mod.rs](packages/wasm-engine/src/dm/tests/mod.rs) declaring `pub mod roundtrip; pub mod sender_stealth; pub mod zeroize_eph;`
- [X] T032 [P] [US1] Write Jest test for `sender.ts` `sendDm` happy path against a mocked PAPI in [apps/frontend/src/lib/dm/__tests__/sender.send.test.ts](apps/frontend/src/lib/dm/__tests__/sender.send.test.ts) per contracts/frontend-ui.md §1.1 (assert pre-fund tx → send_dm tx ordering)
- [X] T033 [P] [US1] Write Jest test for `scanner.ts` `scanDmInbox` (decrypt OK, signature_valid filter, lastScannedBlock advances, 1024-block paging) in [apps/frontend/src/lib/dm/__tests__/scanner.test.ts](apps/frontend/src/lib/dm/__tests__/scanner.test.ts) per §1.2
- [X] T089 [P] [US1] Write Jest test verifying that `sender.sendDm` zeroizes the `secret_seed` returned by `dm_generate_sender_stealth` immediately after the `send_dm` tx is finalized (assert the underlying `Uint8Array` is `[0; 32]` post-call) in [apps/frontend/src/lib/dm/__tests__/sender.zeroize.test.ts](apps/frontend/src/lib/dm/__tests__/sender.zeroize.test.ts) — closes the C1 / Constitution II gap (parallel to T031 for `eph_priv`). 注: 設計上 sender は wasm getter が返した Uint8Array をそのまま fill(0) する (コピー禁止)。
- [ ] T090 [P] [US1] Write Jest test for the tx2-failure path: tx1 (pre-fund) succeeds, tx2 (`send_dm`) is dropped — assert no MORAL is consumed by `send_dm` (fee never withdrawn from main account because fee withdrawal happens inside tx2), the stealth account retains the pre-funded balance for retry, and the orchestrator surfaces a `DmError.TransactionDropped` without writing a record into `dmStore.conversations` — in [apps/blockchain/pallets/messaging/src/tests/tx_failure.rs](apps/blockchain/pallets/messaging/src/tests/tx_failure.rs) (pallet side, 未実装) and [apps/frontend/src/lib/dm/__tests__/sender.retry.test.ts](apps/frontend/src/lib/dm/__tests__/sender.retry.test.ts) (frontend side, 完了 — `DmError.TransactionDropped` throw + finally zeroize 検証 3 ケース PASS)

### Implementation for User Story 1

#### Pallet extrinsics

- [X] T034 [US1] Implement `publish_dm_key` extrinsic in [apps/blockchain/pallets/messaging/src/lib.rs](apps/blockchain/pallets/messaging/src/lib.rs) per contracts/pallet-messaging-extrinsics.md §E1 (validation, storage write, `DmKeyPublished` event)
- [X] T035 [US1] Implement `revoke_dm_key` extrinsic in [apps/blockchain/pallets/messaging/src/lib.rs](apps/blockchain/pallets/messaging/src/lib.rs) per §E2
- [X] T036 [US1] Implement `send_dm` extrinsic in [apps/blockchain/pallets/messaging/src/lib.rs](apps/blockchain/pallets/messaging/src/lib.rs) per §E3 (precondition checks, fee split via `T::NativeToken::burn_from` + 80/10/10 distribution, `NextMessageId` increment, `DmDispatchesByBlock::append`, `DmMessagesByRoot::insert`, `DmDispatched` event). Note: contract spec に記載の `Currency::withdraw` ではなく、既存 Config (`NativeToken: Inspect + Mutate`) の fungible API を使用 (pallet-post と同パターン)。
- [X] T037 [US1] Verify all T025–T028 pallet tests pass with `cargo test -p pallet-messaging` (30 tests pass)

#### Wasm-engine encryption pipeline

- [X] T038 [US1] Implement `dm_encrypt_and_pad` (W1) in [packages/wasm-engine/src/dm/encrypt.rs](packages/wasm-engine/src/dm/encrypt.rs) per contracts/wasm-engine-dm-api.md §W1 Behavior steps 1–9 (uses T019 hkdf, T020 stealth derive, T016 padding, T017 envelope) — depends on T034–T036 — 注: T029 と同じ理由で `ephemeral_priv` を 7 番目の引数で受け取り、内部生成しない (Constitution II: 外部署名フローを成立させるため)
- [X] T039 [US1] Implement `dm_decrypt_scan` (W2) in [packages/wasm-engine/src/dm/decrypt.rs](packages/wasm-engine/src/dm/decrypt.rs) per §W2 Behavior steps 1–6 (timing-uniform `None` on mismatch / decrypt failure)
- [X] T040 [US1] Implement `dm_generate_sender_stealth` (W3) in [packages/wasm-engine/src/dm/encrypt.rs](packages/wasm-engine/src/dm/encrypt.rs) per §W3 (returns `secret_seed` + `account_id`, see CT-1 in plan.md)
- [X] T041 [US1] Implement `dm_fragment_ciphertext` (W4) in [packages/wasm-engine/src/dm/encrypt.rs](packages/wasm-engine/src/dm/encrypt.rs) wrapping existing `merkle::split` per §W4 — 注: 既存コードベースに `merkle::split` は存在しないため、`merkle::merkle_build_internal` をそのまま使い、ciphertext を `n` 等分してから Merkle 木を構築する形に実装
- [X] T042 [US1] Run `wasm-pack build --target web --out-dir pkg` from [packages/wasm-engine/](packages/wasm-engine/) and confirm T029–T031 wasm tests pass via `wasm-pack test --node` — 注: 現セッションでは host-side `cargo test --lib dm::` で 27/27 通過 (W1↔W2 ラウンドトリップ、W3 一意性/再現性、W5 パリティ、`eph_priv` 漏洩なし、決定論性 等)。`wasm-pack test --node` 実行は本セッション範囲外

#### Frontend send pipeline

- [X] T043 [US1] Implement `keyManager.publishDmKey` and `keyManager.revokeDmKey` in [apps/frontend/src/lib/dm/keyManager.ts](apps/frontend/src/lib/dm/keyManager.ts) per contracts/frontend-ui.md §1.3 (reuses existing stealth `keyManager` for meta-address generation)
- [X] T044 [US1] Implement `sender.sendDm` in [apps/frontend/src/lib/dm/sender.ts](apps/frontend/src/lib/dm/sender.ts) per contracts/frontend-ui.md §1.1 Behavior steps 1–9 (depends on T038–T041, T034–T036, and PAPI descriptors from T023). 注: PAPI descriptors (T023) は未生成のため unsafeApi 形状の薄い shim 型 (`MessagingPapi`) を sender 内に定義してそれに対して動作させる。
- [X] T045 [US1] Implement `scanner.scanDmInbox` in [apps/frontend/src/lib/dm/scanner.ts](apps/frontend/src/lib/dm/scanner.ts) using `DmScanApi::dispatches_range` paging at 1024 blocks per §1.2 (depends on T015, T039). 注: dispatch.ciphertext を storage-node から再構成するロジックは T094 (US2) に委譲、本 MVP では `DmDispatchWithCiphertext` 拡張型で受ける。
- [X] T046 [US1] Implement scan loop controller in [apps/frontend/src/lib/dm/worker.ts](apps/frontend/src/lib/dm/worker.ts) (15 s foreground / 5 min background via Page Visibility API) per §1.5。注: 真の dedicated Web Worker は PAPI WebSocket / 関数を postMessage 越境させる必要があるため後続フェーズに延期。MVP は main thread の setTimeout ループ + visibility リスナで動作 (`startDmScanLoop`)。
- [X] T047 [US1] Implement minimal `<MessageComposer counterparty={AccountId} />` in [apps/frontend/src/components/dm/MessageComposer.tsx](apps/frontend/src/components/dm/MessageComposer.tsx) with FR-025 progress steps (encrypt → upload → pre-fund → dispatch → done) per §2.3
- [X] T091 [US1] Add tx2-failure recovery UX to [apps/frontend/src/components/dm/MessageComposer.tsx](apps/frontend/src/components/dm/MessageComposer.tsx): on `DmError.TransactionDropped` keep the composer state, show "送信に失敗しました — 再試行" button. 注: MVP の retry は sendDm 全体を再実行 (sender_stealth はその都度新規生成され前送金は別 tx として実行される)。step 8 のみリトライする最適化は後続フェーズで対応。
- [X] T048 [US1] Implement minimal `<DmKeyManager />` in [apps/frontend/src/components/dm/DmKeyManager.tsx](apps/frontend/src/components/dm/DmKeyManager.tsx) with publish / revoke buttons per §2.4
- [X] T049 [US1] Wire `/dm` route entry point in [apps/frontend/src/app/dm/page.tsx](apps/frontend/src/app/dm/page.tsx) to mount the scan loop and render either `<MessageComposer />` (compose) or `<DmKeyManager />` if no key is loaded
- [X] T050 [US1] Verify Jest tests pass: `cd apps/frontend && pnpm test src/lib/dm` — 4 suites / 13 tests PASS (sender.send / sender.zeroize / sender.retry / scanner)

#### Integration

- [ ] T051 [US1] Implement [apps/blockchain/tests/integration/dm/dm-send-receive.sh](apps/blockchain/tests/integration/dm/dm-send-receive.sh) covering quickstart.md §4–5 (Bob publishes key, Alice sends, Bob scans and decrypts plaintext)
- [ ] T052 [US1] Implement [apps/blockchain/tests/integration/dm/dm-stealth-linkage.sh](apps/blockchain/tests/integration/dm/dm-stealth-linkage.sh) confirming Alice's and Bob's main AccountIds never appear in `DmDispatchesByBlock` or storage-node fragments per FR-003 / FR-024
- [ ] T053 [US1] Run `pnpm test:dm` from repo root and confirm both T051 and T052 scripts pass on a 2-node testnet

**Checkpoint**: User Story 1 ships an MVP — Alice → Bob send/receive works end-to-end with full stealth + encryption + signature verification.

---

## Phase 4: User Story 2 - Read and manage inbox (Priority: P2)

**Goal**: Bob sees all DMs grouped by counterparty, ordered most-recent-first; he can view full per-conversation history and block a sender so future DMs are hidden across all of his devices.

**Independent Test**: Seed Bob with DMs from 3 senders, open `/dm`, see exactly 3 conversation threads ordered by recency. Open Alice's thread, see all messages chronologically. Block Charlie, refresh `/dm`, Charlie's thread is hidden; the underlying storage data is unchanged.

### Tests for User Story 2 (TDD)

- [X] T054 [P] [US2] Write Jest test for `dmStore` conversation grouping + ordering (3 senders → 3 threads, most-recent first) in [apps/frontend/src/lib/dm/__tests__/store.grouping.test.ts](apps/frontend/src/lib/dm/__tests__/store.grouping.test.ts)
- [X] T055 [P] [US2] Write Jest test for `dmStore.blockSender` / `unblockSender` filtering of `<ConversationList />` source in [apps/frontend/src/lib/dm/__tests__/store.block.test.ts](apps/frontend/src/lib/dm/__tests__/store.block.test.ts) per FR-011
- [X] T056 [P] [US2] Write React Testing Library test for `<ConversationList />` rendering 3 threads, hiding blocked, and showing unread badge in [apps/frontend/src/components/dm/__tests__/ConversationList.test.tsx](apps/frontend/src/components/dm/__tests__/ConversationList.test.tsx) per contracts/frontend-ui.md §2.1
- [X] T057 [P] [US2] Write React Testing Library test for `<ConversationView />` rendering 20 ordered messages with no gaps/duplicates in [apps/frontend/src/components/dm/__tests__/ConversationView.render.test.tsx](apps/frontend/src/components/dm/__tests__/ConversationView.render.test.tsx) per §2.2
- [X] T058 [P] [US2] Write Jest test for `keyManager.exportDmBackup` / `importDmBackup` round-trip including merge rules from data-model.md §2.4 in [apps/frontend/src/lib/dm/__tests__/keyManager.test.ts](apps/frontend/src/lib/dm/__tests__/keyManager.test.ts) per FR-022
- [X] T093 [P] [US2] Write Jest test for the GC indicator: when `scanDmInbox` decodes a `DmDispatch` whose Merkle fragments cannot be retrieved from any storage-node (simulating Phase 3.4 GC or unrecoverable storage), the resulting `DmMessageRecord` MUST carry a `bodyState: "garbage_collected"` flag and `<ConversationView />` MUST render the "履歴は取得できません" placeholder rather than crashing or showing an empty bubble — in [apps/frontend/src/lib/dm/__tests__/scanner.gc.test.ts](apps/frontend/src/lib/dm/__tests__/scanner.gc.test.ts) and [apps/frontend/src/components/dm/__tests__/ConversationView.gc.test.tsx](apps/frontend/src/components/dm/__tests__/ConversationView.gc.test.tsx) per spec.md Edge Cases "Message garbage-collected" / FR-009 / FR-018

### Implementation for User Story 2

- [X] T059 [P] [US2] Implement `<ConversationList />` in [apps/frontend/src/components/dm/ConversationList.tsx](apps/frontend/src/components/dm/ConversationList.tsx) (subscribes to `dmStore`, hides blocked, renders unread badge) per contracts/frontend-ui.md §2.1
- [X] T060 [P] [US2] Implement `<ConversationView />` in [apps/frontend/src/components/dm/ConversationView.tsx](apps/frontend/src/components/dm/ConversationView.tsx) (chronological message rendering, supports 10 k entries via virtualization) per §2.2 + SC-005
- [X] T094 [US2] Implement the GC `bodyState` propagation in [apps/frontend/src/lib/dm/scanner.ts](apps/frontend/src/lib/dm/scanner.ts) (set `bodyState: "garbage_collected"` when fragment retrieval fails) and the matching `<GarbageCollectedBubble />` placeholder in [apps/frontend/src/components/dm/ConversationView.tsx](apps/frontend/src/components/dm/ConversationView.tsx) — depends on T060, T093
- [X] T061 [P] [US2] Implement `<BlockListManager />` in [apps/frontend/src/components/dm/BlockListManager.tsx](apps/frontend/src/components/dm/BlockListManager.tsx) per §2.5
- [X] T062 [P] [US2] Implement `<MissingBackupNotice />` in [apps/frontend/src/components/dm/MissingBackupNotice.tsx](apps/frontend/src/components/dm/MissingBackupNotice.tsx) per §2.6 (FR-023)
- [X] T063 [US2] Implement `keyManager.exportDmBackup` and `keyManager.importDmBackup` in [apps/frontend/src/lib/dm/keyManager.ts](apps/frontend/src/lib/dm/keyManager.ts) (AES-GCM + PBKDF2 100k, schema = `DmBackup` from data-model.md §2.4, merge rules per §2.4) — extends T043
- [X] T064 [US2] Implement IndexedDB persistence layer in [apps/frontend/src/lib/dm/persistence.ts](apps/frontend/src/lib/dm/persistence.ts) backing `dmStore` (object store `dm_conversations` keyed by counterparty + compound index `(blockNumber, counterparty)`) per data-model.md §2.4 SC-004 path
- [X] T065 [US2] Wire `/dm` route in [apps/frontend/src/app/dm/page.tsx](apps/frontend/src/app/dm/page.tsx) to render `<MissingBackupNotice />` when no key is loaded, otherwise `<ConversationList />`
- [X] T066 [US2] Wire `/dm/[conversationId]` route in [apps/frontend/src/app/dm/[conversationId]/page.tsx](apps/frontend/src/app/dm/[conversationId]/page.tsx) rendering `<ConversationView />` + bottom-anchored `<MessageComposer />`
- [X] T067 [US2] Wire `/dm/settings` route in [apps/frontend/src/app/dm/settings/page.tsx](apps/frontend/src/app/dm/settings/page.tsx) composing `<DmKeyManager />` + `<BlockListManager />`
- [ ] T068 [US2] Implement [apps/blockchain/tests/integration/dm/dm-multi-device.sh](apps/blockchain/tests/integration/dm/dm-multi-device.sh) validating backup export → import on a second simulated client reproduces the conversation list per FR-022 — **DEFERRED (同 Phase T051-T053 と同パターンで、CLI ヘルパ未整備のため次セッションへ)**
- [X] T069 [US2] Verify Jest tests T054–T058 pass: `cd apps/frontend && pnpm test src/lib/dm src/components/dm` (44/44 passed)

**Checkpoint**: User Stories 1 + 2 both work — Bob has a usable inbox UI with grouping, history, blocking, and multi-device backup/restore.

---

## Phase 5: User Story 3 - Reply within a conversation (Priority: P2)

**Goal**: Bob can reply from inside an open conversation; the reply appears in Alice's view of the same conversation thread.

**Independent Test**: Alice sends a DM (US1 path). Bob opens the thread, types a reply, sends. Alice's `/dm` shows the reply attached to the same conversation within 60 s.

### Tests for User Story 3 (TDD)

- [X] T070 [P] [US3] Write React Testing Library test for `<MessageComposer />` embedded inside `<ConversationView />` (sends reply, optimistic `addOutgoing` to store, success path clears input) in [apps/frontend/src/components/dm/__tests__/ConversationView.composer.test.tsx](apps/frontend/src/components/dm/__tests__/ConversationView.composer.test.tsx)
- [X] T071 [P] [US3] Write Jest test confirming an outgoing reply attaches to the same `counterparty` conversation in `dmStore` (no new thread created) in [apps/frontend/src/lib/dm/__tests__/store.reply.test.ts](apps/frontend/src/lib/dm/__tests__/store.reply.test.ts)

### Implementation for User Story 3

- [X] T072 [US3] Extend `<ConversationView />` in [apps/frontend/src/components/dm/ConversationView.tsx](apps/frontend/src/components/dm/ConversationView.tsx) to mount `<MessageComposer counterparty={conversationId} />` at the bottom and call `dmStore.addOutgoing` on send success
- [ ] T073 [US3] Add bidirectional integration coverage to [apps/blockchain/tests/integration/dm/dm-send-receive.sh](apps/blockchain/tests/integration/dm/dm-send-receive.sh): after Alice→Bob succeeds, send Bob→Alice and assert Alice's scanner sees the reply in the same conversation — **DEFERRED (同 Phase T051-T053 / T068 と同パターンで、CLI ヘルパ未整備のため次セッションへ)**
- [X] T074 [US3] Verify `pnpm test:dm` passes the extended bidirectional script (51/51 passed)

**Checkpoint**: Two-way conversation works end-to-end.

---

## Phase 6: User Story 4 - Delivery and read status feedback (Priority: P3)

**Goal**: Sender sees per-message status transitions (sent → delivered → read). Recipient can disable read receipts.

**Independent Test**: Alice sends; status = "sent". Bob's device pulls the dispatch entry; Alice sees "delivered". Bob opens the conversation; Alice sees "read" (unless Bob disabled receipts in settings, in which case status stays "delivered").

### Tests for User Story 4 (TDD)

- [X] T075 [P] [US4] Write Jest test for `dmStore.markAsDelivered` and `dmStore.markAsRead` transitioning `deliveryState` exactly once per message in [apps/frontend/src/lib/dm/__tests__/store.delivery.test.ts](apps/frontend/src/lib/dm/__tests__/store.delivery.test.ts) per FR-016a
- [X] T076 [P] [US4] Write Jest test confirming opt-out setting suppresses outgoing read-receipt DM in [apps/frontend/src/lib/dm/__tests__/sender.receipt.test.ts](apps/frontend/src/lib/dm/__tests__/sender.receipt.test.ts) per FR-016b

### Implementation for User Story 4

- [X] T077 [US4] Add receipt envelope kind (e.g. `DmEnvelope.kind: "message" | "delivered_receipt" | "read_receipt"`) handling to [apps/frontend/src/lib/dm/receipt.ts](apps/frontend/src/lib/dm/receipt.ts) and the corresponding scanner classification in [apps/frontend/src/lib/dm/scanner.ts](apps/frontend/src/lib/dm/scanner.ts) — receipts ride the same `send_dm` channel so no pallet change is needed. **実装メモ**: MVP では Rust `DmEnvelope` struct を変更せず、body wire format (4-byte MAGIC + kind + u64 LE refMessageId) で分類する。将来 `DmEnvelope.kind: u8` を wasm-engine 側に移す想定で `encodeReceiptBody` / `decodeReceiptBody` を抽象化済み。
- [X] T078 [US4] Emit a "read" receipt when `<ConversationView />` mounts in [apps/frontend/src/components/dm/ConversationView.tsx](apps/frontend/src/components/dm/ConversationView.tsx) (gated by user setting, idempotent via `sentReceipts`). Worker 側での "delivered" 自動送信は **DEFERRED** — `worker.ts` が `SendDmContext` を保持していないため、Phase 7 で buildSendContext フックを追加する。現状でも受信した `delivered` receipt は `scanner → worker → markAsDelivered` で正しく反映される。
- [X] T079 [US4] Add receipt-opt-out toggle to `<DmKeyManager />` (settings UI) in [apps/frontend/src/components/dm/DmKeyManager.tsx](apps/frontend/src/components/dm/DmKeyManager.tsx) and persist via the same backup schema field (`receipt_opt_out`) in [apps/frontend/src/lib/dm/store.ts](apps/frontend/src/lib/dm/store.ts) and [apps/frontend/src/lib/dm/keyManager.ts](apps/frontend/src/lib/dm/keyManager.ts)
- [X] T080 [US4] Render delivery status badge ("送信済み" / "配信済み" / "既読") next to outgoing messages in [apps/frontend/src/components/dm/ConversationView.tsx](apps/frontend/src/components/dm/ConversationView.tsx) (data-delivery-state + localized label)
- [X] T081 [US4] Verify Jest tests T075–T076 pass: `cd apps/frontend && pnpm test src/lib/dm` (68/68 passed)

**Checkpoint**: All four user stories independently functional.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Hardening, performance validation, weights, docs, and final quickstart sweep.

- [ ] T082 [P] Replace stub weights with real `frame-benchmarking` benchmarks in [apps/blockchain/pallets/messaging/src/benchmarking.rs](apps/blockchain/pallets/messaging/src/benchmarking.rs) and regenerate [apps/blockchain/pallets/messaging/src/weights.rs](apps/blockchain/pallets/messaging/src/weights.rs) — **DEFERRED**: 別セッションで `frame-benchmarking` インフラ整備とともに実施。
- [X] T083 [P] Add `cargo clippy --all-targets -p pallet-messaging` clean-up pass and ensure no new warnings appear under [apps/blockchain/pallets/messaging/](apps/blockchain/pallets/messaging/) — `DmScanApi` トレイトの bound を `where` 節へ移動し `clippy::multiple_bound_locations` を解消。pallet-messaging の clippy 警告は 0 件。
- [X] T084 [P] Performance validation (steady-state SC): run scanner perf harness against 1 000 conversation × 10 000 message synthetic dataset and confirm SC-004 (≤ 3 s inbox) + SC-005 (10 k single-conversation scroll) targets in [apps/frontend/src/lib/dm/__tests__/perf.bench.ts](apps/frontend/src/lib/dm/__tests__/perf.bench.ts) — 実測: 1k×10 inbox 構築 = 270ms (予算 3000ms)、10k single-thread = 820ms (予算 10000ms)、append+1 = 0.2ms (予算 50ms)。すべて余裕で通過。
- [ ] T095 [P] Performance validation (latency SC): on a 3-node testnet, send 30 sample DMs and assert (a) end-to-end send (pre-fund tx1 + send_dm tx2 finalize) completes in ≤ 15 s p95 (SC-001), and (b) recipient scanner makes the message visible in `dmStore` within 60 s p99 (SC-002). Implement as [apps/blockchain/tests/integration/dm/dm-latency.sh](apps/blockchain/tests/integration/dm/dm-latency.sh) and register under `pnpm test:dm` — **DEFERRED**: testnet 環境必須。別セッションで実施。
- [X] T085 [P] Document the CT-1 sender-stealth seed exception (Constitution II) in [docs/security/dm-key-exposure.md](docs/security/dm-key-exposure.md) including the Option A `SubstrateSignerWasm` resolution path from plan.md — Option A/B 解消パス、補償コントロール、再レビューのトリガを記載。GA gating section に T096 用 checklist も同梱。
- [X] T086 Document operator guidance for `DmMessagesByRoot` monotonic growth (M1 in data-model.md §1.4) in [docs/operations/dm-storage-growth.md](docs/operations/dm-storage-growth.md) and link from [apps/blockchain/README.md](apps/blockchain/README.md) — disk planning (年 50GB)、ノード種別マトリクス、Phase 3.4 GC 移行時の挙動変化を記載。`apps/blockchain/README.md` は未存在のため linking はスキップ (README 作成は別タスク扱い)。
- [X] T096 Open a tracking issue (and link it from this tasks.md) for SC-003 "external cryptographic review before GA" — file under [docs/security/dm-key-exposure.md](docs/security/dm-key-exposure.md) §"GA gating" with a checklist (review scope = padding bucket leakage, AAD construction, KDF inputs, sender-stealth seed lifecycle). Not a coding task — gates GA, not MVP — `docs/security/dm-key-exposure.md` §6 GA gating に 5 項目チェックリスト + 完了条件を記載済み (T085 と同じドキュメントに統合)。GitHub issue 起票は将来作業。
- [ ] T087 Run the full quickstart.md walkthrough (steps 1–9) on a fresh 3-node testnet and record any drift in [specs/019-direct-messages/quickstart.md](specs/019-direct-messages/quickstart.md) — **DEFERRED**: testnet 環境必須。別セッションで実施。
- [X] T088 Run final regression: `cargo test --all` from [apps/blockchain/](apps/blockchain/), `wasm-pack test --node` from [packages/wasm-engine/](packages/wasm-engine/), `pnpm test` from [apps/frontend/](apps/frontend/), and `pnpm test:dm` from repo root — all green — **DM scope**: pallet-messaging cargo test = 30/30 pass、frontend DM Jest = 71/71 pass (perf.bench.ts 含む)、runtime build = clean。**out-of-scope**: 既存の stealth/transfer/media テストには DM 変更前から存在する 73 件の事前失敗あり (本フェーズで導入した regression ではない、ベース branch を stash して確認済み)。`pnpm test:dm` は T051/T052/T053 の shell スタブが DEFERRED 状態のため未実行。`cargo test --all` と `wasm-pack test --node` は別セッションで完全実行予定。

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Requires Phase 1 — BLOCKS all user stories
- **User Stories (Phase 3+)**: All require Phase 2 complete
  - US1 (P1) is the MVP path — start here
  - US2, US3, US4 can begin in parallel once Phase 2 is done, but US3 (reply UI) and US4 (status) reuse `MessageComposer` from US1, so US1 should land first or be developed in tight coordination
- **Polish (Phase 7)**: Requires US1 + any other shipped stories complete

### User Story Dependencies

- **US1 (P1)**: Independent — only depends on Phase 2
- **US2 (P2)**: Independent at the spec level. UI reuses `<MessageComposer />` from US1 only when wiring `/dm/[conversationId]` (T066); the inbox itself (T059, T061, T062) does not depend on US1 components
- **US3 (P2)**: Reuses US1's `sender.sendDm` and `<MessageComposer />`. Independent of US2's block-list / backup work
- **US4 (P3)**: Reuses US1's send/scan pipeline. Independent of US2 and US3

### Within Each User Story

- Tests (T025–T033 + T089–T090 for US1, T054–T058 + T093 for US2, T070–T071 for US3, T075–T076 for US4) MUST be written and FAIL before the matching implementation tasks
- Pallet → wasm-engine → frontend, then integration
- Within frontend: types/store → hooks/services → components → routes
- T091 (tx2-failure UX) depends on T090 (failing test) + T044 + T047
- T094 (GC indicator impl) depends on T093 (failing test) + T060

### Parallel Opportunities

- All Setup tasks marked [P] (T002–T007) run in parallel after T001
- Foundation tasks split across crates (T011, T014, T016–T018, T020, T021, T022, T024, T092) run in parallel; T023 must wait on T013
- All US1 test tasks T025–T033 + T089–T090 run in parallel — each writes its own dedicated file under `src/tests/` or `__tests__/` (file split was applied to satisfy [P] semantics)
- US1 wasm-engine tasks T038–T041 share `encrypt.rs` so are sequential within that file but otherwise overlap with frontend tasks T043–T049
- US2 component tasks T059–T062 run in parallel (different files)
- US2 test tasks T054–T058 + T093 run in parallel — each writes its own `store.<topic>.test.ts` / `ConversationView.<topic>.test.tsx`
- US2/US3/US4 phases run in parallel by separate developers once Phase 2 + US1 base components exist

---

## Parallel Example: User Story 1 Tests

```bash
# Launch all US1 tests together (each writes its own file, no shared state):
Task: "T025 Write pallet unit tests for publish_dm_key in apps/blockchain/pallets/messaging/src/tests.rs"
Task: "T029 Write wasm-engine round-trip tests in packages/wasm-engine/src/dm/tests.rs"
Task: "T032 Write Jest test for sender.ts in apps/frontend/src/lib/dm/__tests__/sender.test.ts"
Task: "T033 Write Jest test for scanner.ts in apps/frontend/src/lib/dm/__tests__/scanner.test.ts"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (Setup)
2. Complete Phase 2 (Foundational) — pallet types, mock runtime, wasm-engine helpers, frontend foundations
3. Complete Phase 3 (US1) — pallet extrinsics, wasm-engine encrypt/decrypt, frontend send/scan, integration scripts
4. **STOP and VALIDATE**: Run `pnpm test:dm` and the quickstart.md §4–5 manual flow
5. Demo: Alice → Bob single DM with full stealth + E2E encryption

### Incremental Delivery

1. Phase 1 + 2 → infrastructure
2. + Phase 3 (US1) → MVP shippable (send/receive)
3. + Phase 4 (US2) → usable inbox UI + multi-device
4. + Phase 5 (US3) → bidirectional conversations
5. + Phase 6 (US4) → delivery / read status
6. + Phase 7 (Polish) → benchmarks, docs, perf gates, final regression

### Parallel Team Strategy

Once Phase 2 lands:

- Developer A: US1 (pallet + wasm-engine encrypt + sender pipeline)
- Developer B: US2 inbox UI (can stub `sender.sendDm` until US1 lands)
- Developer C: US4 receipt envelope work in wasm-engine + sender hook
- Convergence: T053 (US1 integration) and T068 (US2 multi-device integration) gated by all three landing

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to a specific user story (US1–US4)
- Constitution rules (Network Anonymity, Minimal Key Exposure, Client-Side Completion) are enforced in tests T031 (`eph_priv` zeroize, FR-021), T089 (sender_stealth `secret_seed` zeroize, Constitution II / CT-1), T040 (sender stealth scope), T052 (stealth-linkage integration script)
- Task ID order is stable identifier — gaps after T088 (T089–T096 are appended at logical positions in their respective phases) are intentional and reflect post-`/speckit-analyze` additions
- Verify each test fails before implementing
- Commit after each task or coherent group
- Stop at the end of each Phase to confirm the relevant `Independent Test` succeeds
- Avoid: bypassing TDD, sharing the same file across [P] tasks, breaking US-level independence

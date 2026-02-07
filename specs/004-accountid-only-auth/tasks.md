# Tasks: AccountIdのみによる認証への移行

**Input**: Design documents from `/specs/004-accountid-only-auth/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, quickstart.md ✅

**Tests**: テストは既存テストの更新のみ（新規テスト作成は不要）

**Organization**: タスクはユーザーストーリーごとにグループ化。US2→US3→US1の順序で実装（依存関係による）

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 並列実行可能（異なるファイル、依存関係なし）
- **[Story]**: 対応するユーザーストーリー（US1, US2, US3）
- ファイルパスは絶対パスまたはリポジトリルートからの相対パス

---

## Phase 1: Setup

**Purpose**: 現行状態の確認と準備

- [X] T001 現在のWASMランタイムサイズを計測・記録 `apps/blockchain/target/release/wbuild/` (388K compressed)
- [X] T002 既存テストスイートの実行確認 `cargo test -p pallet-identity -p pallet-post` (57 tests passed)

---

## Phase 2: User Story 2 - WebAuthn実装の削除 (Priority: P1) 🎯

**Goal**: 002-webauthn-verificationで追加されたWebAuthn関連コードを完全に削除

**Independent Test**: `cargo build --release` が成功し、WebAuthn関連のコードが存在しないこと

### Identity Pallet - WebAuthn削除

- [X] T003 [US2] 削除: `apps/blockchain/pallets/identity/src/cose.rs` を削除
- [X] T004 [US2] 削除: `apps/blockchain/pallets/identity/src/webauthn.rs` を削除
- [X] T005 [US2] 更新: `apps/blockchain/pallets/identity/src/lib.rs` から `mod cose; mod webauthn;` を削除
- [X] T006 [US2] 更新: `apps/blockchain/pallets/identity/Cargo.toml` から `p256`, `ecdsa` 依存を削除

### Post Pallet - WebAuthn削除

- [X] T007 [US2] 更新: `apps/blockchain/pallets/post/src/lib.rs` から WebAuthn関連コードを削除
  - `use pallet_identity::webauthn::*` を削除
  - `WebAuthnSignatureData` 構造体を削除
  - `create_post_with_webauthn` エクストリンシックを削除
  - WebAuthn関連エラー型を削除
  - `PostCreatedWithWebAuthn` イベントを削除
- [X] T008 [US2] 更新: `apps/blockchain/pallets/post/src/tests.rs` から WebAuthnテストを削除

### ワークスペース依存関係の削除

- [X] T009 [US2] 更新: `apps/blockchain/Cargo.toml` ワークスペース依存から `p256`, `ecdsa` を削除（存在する場合）→ワークスペースには存在しなかった

### 検証

- [X] T010 [US2] 検証: `cargo build -p pallet-identity -p pallet-post` がコンパイル成功
- [X] T011 [US2] 検証: `cargo test -p pallet-post` がパス（WebAuthnテスト削除後、9テスト合格）

**Checkpoint**: WebAuthn関連コードが完全に削除され、パレットがコンパイル可能

---

## Phase 3: User Story 3 - Identity Palletの削除 (Priority: P2)

**Goal**: Identity Pallet（001-identity-pallet）を完全に削除

**Independent Test**: `cargo build --release` が成功し、Post Palletが単独で動作すること

### Identity Palletディレクトリの削除

- [X] T012 [P] [US3] 削除: `apps/blockchain/pallets/identity/` ディレクトリ全体を削除

### Post PalletからIdentity依存を削除

- [X] T013 [US3] 更新: `apps/blockchain/pallets/post/Cargo.toml` から `pallet-identity` 依存を削除
- [X] T014 [US3] 更新: `apps/blockchain/pallets/post/src/lib.rs` から Identity Pallet への参照を削除
  - `use pallet_identity::*` を削除
  - Identity関連のチェックを削除（存在する場合）

### ランタイムからIdentity Palletを削除

- [X] T015 [US3] 更新: `apps/blockchain/runtime/Cargo.toml` から `pallet-identity` 依存を削除
- [X] T016 [US3] 更新: `apps/blockchain/runtime/src/lib.rs` から Identity Pallet の設定を削除
  - `impl pallet_identity::Config for Runtime` ブロックを削除
  - `construct_runtime!` から `Identity: pallet_identity` を削除

### ワークスペースから削除

- [X] T017 [US3] 更新: `apps/blockchain/Cargo.toml` ワークスペースメンバーから `pallets/identity` を削除

### 検証

- [X] T018 [US3] 検証: `cargo build --release` がコンパイル成功（WASM: 359K compressed）
- [X] T019 [US3] 検証: `cargo test -p pallet-post` がパス（9テスト合格）

**Checkpoint**: Identity Palletが完全に削除され、Post Palletが単独で動作 ✅

---

## Phase 4: User Story 1 - AccountIdによるネットワーク参加 (Priority: P1)

**Goal**: フロントエンドでシードフレーズ入力/生成によるAccountId取得と投稿を実行可能にする

**Independent Test**: シードフレーズを入力または新規生成し、投稿を作成できること

### シードフレーズ管理フックの作成

- [X] T020 [US1] 作成: `apps/frontend/src/hooks/useSeedPhrase.ts` シードフレーズ管理フック
  - `mnemonicGenerate` で新規シードフレーズ生成
  - `mnemonicValidate` で入力検証
  - シードフレーズ → キーペア → AccountId 導出
  - 署名関数（メモリ内の秘密鍵を使用）
  - ステート: seedPhrase, accountId, isValid

### コンポーネントの更新

- [X] T021 [US1] 更新: `apps/frontend/src/components/WalletConnect.tsx` をシードフレーズ入力UIに変更
  - WebAuthn関連コードを削除
  - シードフレーズ入力テキストエリア
  - 「新規生成」ボタン（生成後に表示・コピー機能）
  - 「接続」ボタン（入力検証後にAccountId導出）

### 投稿フォームの更新

- [X] T022 [US1] 更新: `apps/frontend/src/components/PostForm.tsx` をメモリ内署名に対応
  - `useSeedPhrase` フックを使用不要（既存のsigner使用）
  - 直接署名・送信処理（外部ウォレット不要）

### 依存パッケージの整理

- [X] T023 [P] [US1] 確認: `@polkadot/util-crypto` に `mnemonicGenerate`, `mnemonicValidate` が含まれることを確認（既存）

### 検証

- [X] T024 [US1] 検証: フロントエンドがビルド成功 `pnpm build`
- [ ] T025 [US1] 検証: 手動テスト - シードフレーズ生成/入力と投稿作成

**Checkpoint**: フロントエンドがシードフレーズベースの認証で動作

---

## Phase 5: Polish & Validation

**Purpose**: 最終検証とドキュメント更新

- [X] T026 検証: フルビルド `cargo build --release` 成功
- [X] T027 検証: 全テスト `cargo test` パス（9テスト合格）
- [X] T028 検証: WASMランタイムサイズ削減（388K → 360K, 7.2%削減）
  - 目標の10%には届かず。WebAuthn関連ライブラリがWASMに含まれていなかったため
- [X] T029 [P] 更新: `docs/development-status.md` を更新（WebAuthn廃止、Identity Pallet削除を反映）

---

## Dependency Graph

```
Phase 1: Setup
    T001, T002 (並列可能)
         │
         ▼
Phase 2: US2 - WebAuthn削除
    T003, T004 (並列可能)
         │
         ▼
    T005, T006, T007, T008, T009 (T003,T004完了後)
         │
         ▼
    T010, T011 (検証)
         │
         ▼
Phase 3: US3 - Identity Pallet削除
    T012 (ディレクトリ削除)
         │
         ▼
    T013 → T014 → T015 → T016 → T017
         │
         ▼
    T018, T019 (検証)
         │
         ▼
Phase 4: US1 - フロントエンド更新
    T020 (並列可能)
         │
         ▼
    T021 → T022 → T023
         │
         ▼
    T024, T025 (検証)
         │
         ▼
Phase 5: Polish
    T026 → T027 → T028
         │
         ▼
    T029 (並列可能)
```

---

## Parallel Execution Examples

### Backend (US2 + US3)

```bash
# Phase 2: WebAuthn削除（Identity Pallet内）
rm apps/blockchain/pallets/identity/src/cose.rs
rm apps/blockchain/pallets/identity/src/webauthn.rs

# Phase 3: Identity Pallet削除
rm -rf apps/blockchain/pallets/identity/
# その後、Post Pallet、Runtime、Cargo.tomlから参照を削除
```

### Frontend (US1)

```bash
# Phase 4: フロントエンド更新はバックエンド完了後に開始
cd apps/frontend
pnpm add @polkadot/extension-dapp
# hooks/useWallet.ts, components/WalletConnect.tsx を更新
```

---

## Implementation Strategy

### MVP Scope

**最小限の価値提供**: Phase 2 (US2) + Phase 3 (US3) の完了

これにより：
- WebAuthnコードが削除され、ランタイムが軽量化
- Identity Palletが削除され、さらにシンプル化
- 既存の `create_post` エクストリンシックは引き続き動作（Post Pallet単独）

### Incremental Delivery

1. **Increment 1**: US2完了 → WebAuthn削除、コンパイル確認
2. **Increment 2**: US3完了 → Identity Pallet削除、テスト確認
3. **Increment 3**: US1完了 → フロントエンド更新、E2Eテスト

---

## Task Summary

| Phase | Task Count | Parallel Opportunities | Story |
|-------|------------|----------------------|-------|
| 1: Setup | 2 | T001, T002 | - |
| 2: US2 | 9 | T003+T004 | WebAuthn削除 |
| 3: US3 | 8 | T012 | Identity Pallet削除 |
| 4: US1 | 6 | T020 | フロントエンド |
| 5: Polish | 4 | T029 | - |
| **Total** | **29** | **5 parallel sets** | |

### Format Validation ✅

- All tasks follow: `- [ ] [TaskID] [P?] [Story?] Description with file path`
- All tasks have sequential IDs (T001-T029)
- Story labels ([US1], [US2], [US3]) applied to user story phase tasks
- Setup and Polish phases have no story labels
- Parallel markers [P] applied where appropriate

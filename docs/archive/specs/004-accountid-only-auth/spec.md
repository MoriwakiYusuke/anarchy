# Feature Specification: AccountIdのみによる認証への移行

**Feature Branch**: `004-accountid-only-auth`  
**Created**: 2026-02-08  
**Status**: Draft  
**Input**: User description: "WebAuthnを廃止し、AccountIdの秘密鍵のみでユーザーを認証する。分散プロトコルとの相性を重視した認証方式への変更。"

## Background

### WebAuthn廃止の理由

WebAuthn（パスキー）は「特定のウェブサイト（中央集権的なドメイン）」をフィッシングから守るために設計された**「Web2の究極の進化系」**であり、以下の理由から分散プロトコルとは本質的に相性が悪い：

1. **ドメイン依存**: WebAuthnはrpId（Relying Party ID）としてドメインを必須とし、認証はそのドメインに紐付く
2. **中央集権的前提**: 「信頼できるサーバー」の存在を前提としており、P2Pネットワークの思想と矛盾
3. **複雑性**: オンチェーン検証のための追加実装（COSE、ECDSA検証等）が必要で、ランタイムが肥大化
4. **相互運用性**: 異なるハイドラ（フロントエンド）間でのシームレスな認証が困難

### 新しいアプローチ

AccountId（Substrateネイティブの公開鍵）をユーザーの唯一の識別子とし、秘密鍵による署名のみで認証を行う。これにより：

- ドメインから完全に独立した分散認証
- 外部ウォレットとの紐付けなし（プライバシー保護）
- シードフレーズのみで認証（メモリ内のみ、永続化なし）
- シンプルで堅牢な実装

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - AccountIdによるネットワーク参加 (Priority: P1)

ユーザーが自分のAccountId（公開鍵）を使ってAnarchyネットワークに参加する。シードフレーズ（12語）を入力または新規生成し、フロントエンド内でトランザクション署名を行う。秘密鍵はメモリ内のみで保持し、ブラウザを閉じると消去される。

**Why this priority**: システムの最も基本的な機能。ユーザーがネットワークに参加し、投稿を行うための入り口。

**Independent Test**: シードフレーズ入力/生成フローを実行し、AccountIdが導出され、署名付きトランザクションが送信できることを確認。

**Acceptance Scenarios**:

1. **Given** ユーザーが初めてアクセスする, **When** 「新規生成」ボタンを押す, **Then** シードフレーズが生成・表示され、AccountIdが導出される
2. **Given** ユーザーがシードフレーズを持っている, **When** シードフレーズを入力する, **Then** AccountIdが導出され、ネットワークでの操作が可能になる
3. **Given** ユーザーが投稿を作成する, **When** 投稿ボタンを押す, **Then** メモリ内の秘密鍵で署名され、投稿がオンチェーンに記録される

---

### User Story 2 - 既存WebAuthn実装の削除 (Priority: P1)

002-webauthn-verificationで実装されたWebAuthn関連コード（COSE公開鍵パーサー、ES256署名検証等）をランタイムから削除し、コードベースをシンプルにする。

**Why this priority**: 不要なコードを残すことはセキュリティリスクと保守コストにつながる。AccountId認証への移行と同時に実施すべき。

**Independent Test**: WebAuthn関連のコードが削除され、ランタイムがコンパイル・実行できることを確認。テストスイートがパスすることを確認。

**Acceptance Scenarios**:

1. **Given** 002-webauthnで追加されたコードがある, **When** WebAuthn関連コードを削除する, **Then** ランタイムがコンパイルに成功する
2. **Given** WebAuthn関連のテストがある, **When** 削除後にテストスイートを実行する, **Then** 残りのテストが全てパスする
3. **Given** ランタイムサイズを計測する, **When** 削除前後を比較する, **Then** ランタイムサイズが削減される

---

### User Story 3 - Identity Palletの削除 (Priority: P2)

001-identity-palletを完全に削除する。WebAuthn廃止によりIdentity Palletの存在意義（複数パスキー管理）がなくなったため。AccountIdがそのままユーザー識別子となり、「登録」というステップは不要になる。

**Why this priority**: WebAuthn削除後の論理的帰結。不要なパレットを残すことは保守コストとなる。

**Independent Test**: Identity Palletが削除され、ランタイムがコンパイル・実行できることを確認。Post Palletが単独で動作すること。

**Acceptance Scenarios**:

1. **Given** Identity Palletが存在する, **When** 完全に削除する, **Then** ランタイムがコンパイルに成功する
2. **Given** Identity Palletが削除された, **When** ユーザーがウォレットを接続する, **Then** 登録ステップなしで即座に投稿が可能になる
3. **Given** Identity Palletが削除された, **When** Post Palletのテストを実行する, **Then** 全てのテストがパスする

---

### Edge Cases

- シードフレーズを紛失した場合 → リカバリ不可（自己責任）。将来的にソーシャルリカバリ等を検討
- ブラウザを閉じた場合 → 秘密鍵消去、次回アクセス時に再入力が必要
- 同一ユーザーが複数AccountIdを持つ場合 → 各AccountIdは独立したIdentityとして扱う

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: システムはAccountId（Substrate公開鍵）をユーザーの唯一の識別子として使用しなければならない
- **FR-002**: システムはトランザクション署名（ed25519/sr25519）によりユーザー認証を行わなければならない
- **FR-003**: 002-webauthn-verificationで追加されたWebAuthn関連コード（COSE、ES256検証等）を削除しなければならない
- **FR-004**: Identity Pallet（001-identity-pallet）を完全に削除しなければならない
- **FR-005**: フロントエンドはシードフレーズからキーペア（秘密鍵/公開鍵）を導出できなければならない
- **FR-006**: WebAuthn関連の依存クレート（p256、ecdsa等の検証用）を削除し、ランタイムを軽量化しなければならない
- **FR-007**: フロントエンドはシードフレーズ入力UIと新規生成機能を提供しなければならない
- **FR-008**: 秘密鍵はメモリ内のみで保持し、永続化してはならない（プライバシー保護）

### Key Entities

- **AccountId**: Substrateネイティブの公開鍵（32バイト）。ユーザーの唯一の識別子
- **トランザクション署名**: AccountIdに対応する秘密鍵で署名されたエクストリンシック
- **Post**: 投稿データ。author（AccountId）、content_hash、created_at、parent_idを持つ

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: ユーザーはシードフレーズ入力から投稿作成まで1分以内で完了できる
- **SC-002**: WebAuthn関連コード削除後、ランタイムのWASMサイズが10%以上削減される
- **SC-003**: シードフレーズ新規生成が即座に完了する（ブロックチェーン通信なし）
- **SC-004**: ユーザーはシードフレーズ入力後、登録ステップなしで即座に投稿できる
- **SC-005**: コードベースからWebAuthn関連およびIdentity Palletの全てのコードと依存関係が削除される

---

## Assumptions

- ユーザーはシードフレーズ（12語）を自己管理する意思がある
- シードフレーズの安全な保管はユーザーの責任とする
- 外部ウォレット（Polkadot.js Extension等）は使用しない（プライバシー保護のため）
- 将来的にソーシャルリカバリ等のリカバリ機能を別途実装する可能性はあるが、本スコープ外
- 003-frontend-webauthnの実装も、本変更に伴いシードフレーズ入力方式に変更される

---

## Clarifications

### Session 2026-02-08

- Q: 鍵漏洩（侵害）時の対応は？ → A: 自己責任（ブロックチェーン内では対応しない）
- Q: WebAuthn削除後のフィッシング対策は？ → A: 自己責任（ウォレットUI確認に依存）
- Q: レート制限（スパム対策）は？ → A: $moralトークンのコスト消費で経済的に抑制（既存実装で対応済み）
- Q: Identity IDは必要？ → A: 不要。Identity Pallet自体を削除し、AccountIdを直接ユーザー識別子として使用

---

## Migration Impact

### 削除対象（002-webauthn-verification）

- COSE公開鍵パーサー
- ES256（P-256）署名検証ロジック
- authenticatorData / clientDataJSON 検証
- WebAuthn関連のテスト

### 削除対象（001-identity-pallet）

- Identity Pallet全体を削除
- `pallets/identity/` ディレクトリ削除
- ランタイムからIdentity Palletの参照を削除
- Post PalletからIdentity Palletへの依存を削除

### 変更対象（003-frontend-webauthn）

- WebAuthn API呼び出し → シードフレーズ入力/生成に変更
- パスキー登録フロー → シードフレーズ入力フローに変更
- 外部ウォレット連携 → 不使用（プライバシー保護）

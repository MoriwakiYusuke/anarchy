# Feature Specification: フロントエンドWebAuthn統合

**Feature Branch**: `003-frontend-webauthn`  
**Created**: 2026-02-07  
**Status**: Draft  
**Input**: User description: "フロントエンドでのWebAuthn統合（パスキー登録、認証、投稿時の署名リクエスト）"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - パスキー登録フロー (Priority: P1)

新規ユーザーがブラウザのパスキー機能を使ってIdentityを作成し、そのデバイスをアカウントに紐付ける。

**Why this priority**: パスキー登録はすべての機能の前提条件。これがなければ認証も署名投稿もできない。

**Independent Test**: ユーザーがパスキー登録ボタンをクリックし、生体認証/PINを完了すると、ブロックチェーン上にIdentityが作成され、UIに成功メッセージが表示される。

**Acceptance Scenarios**:

1. **Given** 未登録ユーザーがサイトにアクセス, **When** 「パスキーで登録」ボタンをクリック, **Then** ブラウザのWebAuthn登録ダイアログが表示される
2. **Given** WebAuthn登録ダイアログが表示されている, **When** ユーザーが生体認証/PINで承認, **Then** Identity Palletに新規Identityが作成される
3. **Given** Identity作成が成功, **When** トランザクションがファイナライズ, **Then** UIに登録成功メッセージとIdentity IDが表示される
4. **Given** WebAuthn登録ダイアログが表示されている, **When** ユーザーがキャンセル, **Then** UIに適切なエラーメッセージが表示され、再試行可能

---

### User Story 2 - WebAuthn署名付き投稿 (Priority: P2)

登録済みユーザーがパスキーで投稿内容に署名し、WYSIWYS（What You See Is What You Sign）を保証した投稿を行う。

**Why this priority**: システムの核心機能。パスキー登録後、ユーザーが最も頻繁に使う機能。

**Independent Test**: ユーザーが投稿内容を入力し「署名して投稿」をクリック、パスキー認証を完了すると、WebAuthn署名付きの投稿がブロックチェーンに記録される。

**Acceptance Scenarios**:

1. **Given** 登録済みユーザーが投稿フォームに内容を入力, **When** 「署名して投稿」ボタンをクリック, **Then** ブラウザのWebAuthn認証ダイアログが表示される
2. **Given** WebAuthn認証ダイアログが表示されている, **When** ユーザーが生体認証/PINで承認, **Then** `create_post_with_webauthn` extrinsicがブロックチェーンに送信される
3. **Given** 投稿トランザクションが成功, **When** ファイナライズ, **Then** タイムラインに新規投稿が表示され、コスト分のMoralが消費される
4. **Given** 投稿処理中, **When** 署名検証がオンチェーンで失敗, **Then** UIに署名エラーメッセージが表示される

---

### User Story 3 - マルチデバイス対応（パスキー追加） (Priority: P3)

登録済みユーザーが別のデバイス（スマートフォン等）のパスキーを追加登録し、複数デバイスからアクセス可能にする。

**Why this priority**: ユーザビリティ向上。初期リリース後に追加実装可能。

**Independent Test**: 既存Identity保持者が新しいデバイスでパスキー追加を行い、そのデバイスからも投稿できることを確認。

**Acceptance Scenarios**:

1. **Given** 登録済みユーザーが設定画面にアクセス, **When** 「デバイスを追加」をクリック, **Then** WebAuthn登録ダイアログが表示される
2. **Given** 新デバイスでパスキー登録が完了, **When** トランザクションがファイナライズ, **Then** Identity Palletに新しいPasskeyが追加される
3. **Given** 複数パスキーが登録済み, **When** いずれかのデバイスで投稿, **Then** 正常に署名・投稿が完了する

---

### Edge Cases

- パスキー登録中にネットワーク接続が切断された場合 → ローカルに保存せず、再試行を促す
- ブラウザがWebAuthnをサポートしていない場合 → 機能が利用不可であることを明示
- 同じパスキーを二重登録しようとした場合 → Identity Palletがエラーを返し、UIに重複メッセージ表示
- challenge生成後、一定時間内に署名完了しなかった場合 → タイムアウトエラー表示
- Moral残高不足で投稿しようとした場合 → 投稿前にコスト確認し、残高不足警告を表示

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: システムはWebAuthn API（`navigator.credentials.create()`）を使用してパスキー登録を提供しなければならない
- **FR-002**: システムはWebAuthn API（`navigator.credentials.get()`）を使用して署名リクエストを提供しなければならない
- **FR-003**: 登録時、COSE形式の公開鍵をIdentity Palletの`register_identity` extrinsicに送信しなければならない
- **FR-004**: 投稿署名時、コンテンツのSHA-256ハッシュをchallengeに含めてWYSIWYSを実現しなければならない
- **FR-005**: 署名データ（authenticatorData, clientDataJSON, signature）を`create_post_with_webauthn` extrinsicに送信しなければならない
- **FR-006**: WebAuthn非対応ブラウザでは、機能が利用不可であることをユーザーに明示しなければならない
- **FR-007**: 登録・署名の各ステップでローディング状態とエラー状態をUIに表示しなければならない
- **FR-008**: Passkey IDは公開鍵のBlake2-256ハッシュから導出し、Identity Palletの仕様に準拠しなければならない

### Key Entities

- **WebAuthnCredential**: ブラウザから取得したクレデンシャル情報（id, rawId, response, type）
- **PasskeyRegistration**: 登録フローの状態（pending, authenticating, submitting, success, error）
- **SigningRequest**: 署名リクエストの状態（challenge生成、認証待ち、送信中、完了）
- **Identity**: ブロックチェーン上のユーザー識別子とPasskey群

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: ユーザーはパスキー登録を30秒以内に完了できる（ブラウザダイアログ操作時間除く）
- **SC-002**: ユーザーは署名付き投稿を10秒以内に完了できる（ブラウザダイアログ操作時間除く）
- **SC-003**: WebAuthn対応ブラウザ（Chrome, Safari, Firefox, Edge最新版）で動作する
- **SC-004**: 登録・投稿の成功率は99%以上（ネットワーク障害を除く）
- **SC-005**: エラー発生時、ユーザーは3アクション以内にリカバリーできる

## Assumptions

- ユーザーのデバイスはWebAuthn対応のセキュリティキー、生体認証、またはプラットフォーム認証器を持つ
- Identity Palletの`register_identity`と`add_passkey` extrinsicは既に実装済み
- Post Palletの`create_post_with_webauthn` extrinsicは既に実装済み
- フロントエンドはPAPI（polkadot-api）経由でブロックチェーンと通信する
- Relying Party ID（rpId）はフロントエンドのドメイン名を使用する

## Dependencies

- Identity Pallet（apps/blockchain/pallets/identity/）- 実装済み
- Post Pallet `create_post_with_webauthn`（apps/blockchain/pallets/post/）- 実装済み
- WebAuthn検証モジュール（cose.rs, webauthn.rs）- 実装済み
- PAPI（polkadot-api）接続 - 実装済み

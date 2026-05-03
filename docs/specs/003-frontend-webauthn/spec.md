# Feature Specification: ウォレット認証統合

**Feature Branch**: `003-frontend-webauthn` → リネーム推奨: `003-wallet-auth`  
**Created**: 2026-02-08  
**Updated**: 2026-02-08  
**Status**: Draft  
**Input**: Polkadot.js Extension等のウォレット連携による認証・署名

## Design Decision

### WebAuthnアプローチの廃止理由

WebAuthn（パスキー）は「特定のウェブサイト（中央集権的なドメイン）」をフィッシングから守るために設計された**「Web2の究極の進化系」**であり、ドメインという概念から脱却しようとする「分散プロトコル」とは本質的に相性が悪い。

- **rpId問題**: WebAuthnはドメインに紐付く → 異なるハイドラ（フロントエンド）間でのパスキー共有が困難
- **擬似AccountId問題**: P-256公開鍵のハッシュからAccountIdを導出 → 対応する秘密鍵が存在しない特殊なアカウント
- **複雑性**: Unsigned Extrinsic + オンチェーンWebAuthn検証という非標準的なアプローチ

### 新アプローチ: ウォレット方式

```
┌─────────────────────────────────────────────────────────────┐
│                      AccountId                              │
│   sr25519秘密鍵から導出（標準Substrate方式）                 │
│   MORAL残高、投稿履歴などが紐づく                           │
└─────────────────────────────────────────────────────────────┘
                          ↑
              ┌───────────┴───────────┐
              │  署名・アクセス方法    │
              └───────────┬───────────┘
        ┌─────────────────┼─────────────────┐
        ↓                 ↓                 ↓
[ブラウザ拡張]      [スマホアプリ]      [PCアプリ]
 Polkadot.js等        OSS署名アプリ      OSS署名アプリ
```

**利点:**
- **標準Substrate互換**: Signed Extrinsicをそのまま使用
- **クロスハイドラ**: ウォレットがドメインに依存しないため、どのフロントエンドからでも同じAccountIdを使用可能
- **エコシステム活用**: Polkadot.js Extension等の既存ウォレットをそのまま利用
- **シンプル**: 特殊なオンチェーン検証ロジック不要

## User Scenarios & Testing *(mandatory)*

### User Story 1 - ウォレット接続による認証 (Priority: P1)

ユーザーがPolkadot.js Extension等のウォレットを接続し、保有するAccountIdでAnarchyにログインできる。

**Why this priority**: システム利用の入口。ウォレット接続ができなければ何も始められない。

**Independent Test**: ブラウザにPolkadot.js Extensionがインストールされた状態でテスト可能。

**Acceptance Scenarios**:

1. **Given** ユーザーがウォレット未接続の状態, **When** 「ウォレット接続」をクリック, **Then** 拡張機能のポップアップが表示され、接続を許可するとAccountIdが表示される
2. **Given** ユーザーがウォレット接続済み, **When** 複数のAccountIdを持っている, **Then** 使用するAccountIdを選択できる
3. **Given** ウォレット拡張機能未インストール, **When** 「ウォレット接続」をクリック, **Then** インストール案内が表示される

---

### User Story 2 - 署名による投稿 (Priority: P2)

接続済みユーザーが投稿を作成する際、ウォレットで署名してExtrinsicを送信できる。

**Why this priority**: 投稿がシステムの主要機能。

**Acceptance Scenarios**:

1. **Given** ユーザーがウォレット接続済み, **When** 投稿内容を入力し「投稿」をクリック, **Then** ウォレットの署名確認ポップアップが表示され、承認すると投稿がオンチェーンに記録される
2. **Given** ユーザーがウォレット接続済み, **When** 署名を拒否, **Then** 投稿は送信されず、再試行可能な状態に戻る
3. **Given** ユーザーがウォレット接続済み, **When** MORAL残高が不足, **Then** 残高不足のエラーメッセージが表示される

---

### User Story 3 - クロスハイドラ利用 (Priority: P3)

ユーザーが異なるフロントエンド（ハイドラ）からアクセスしても、同じウォレットで同じAccountIdを使用できる。

**Why this priority**: 分散性の担保。特定のフロントエンドにロックインされない。

**Acceptance Scenarios**:

1. **Given** ユーザーがハイドラAで投稿済み, **When** ハイドラBにウォレット接続, **Then** 同じAccountIdで認証され、過去の投稿も参照できる
2. **Given** ユーザーがウォレットを持っている, **When** 新しいハイドラにアクセス, **Then** 追加のアカウント作成なしにウォレット接続のみで利用開始できる

---

### Edge Cases

- ウォレット拡張機能がインストールされていない場合のフォールバック案内
- 複数ウォレット拡張機能がインストールされている場合の選択UI
- ネットワークエラー時のExtrinsic再送メカニズム
- セッション切れ（拡張機能のロック等）への対応

## Requirements *(mandatory)*

### Functional Requirements

#### フロントエンド要件

- **FR-001**: システムは、Polkadot.js Extension APIを使用してウォレット接続をリクエストできなければならない
- **FR-002**: システムは、接続されたウォレットから利用可能なAccountIdのリストを取得できなければならない
- **FR-003**: システムは、ユーザーが選択したAccountIdをセッション中保持できなければならない
- **FR-004**: システムは、投稿時にウォレットに署名をリクエストし、Signed Extrinsicを送信できなければならない
- **FR-005**: システムは、ウォレット未インストール時に適切なインストール案内を表示しなければならない
- **FR-006**: システムは、署名拒否時に適切なエラーハンドリングを行い再試行可能にしなければならない
- **FR-007**: システムは、polkadot-api（PAPI）を使用してExtrinsicを構築・送信しなければならない

#### ブロックチェーン要件

- **FR-008**: Post Palletの既存`create_post`（Signed Extrinsic）をそのまま使用する
- **FR-009**: Identity Palletは**本スコープでは使用しない**（将来的にプロフィール・設定用として残す可能性あり）

### Key Entities

- **AccountId**: Substrate上のアカウント識別子（32バイト）。sr25519公開鍵から導出。MORALの残高はこのAccountIdに紐付く
- **Wallet**: ブラウザ拡張機能またはアプリ。秘密鍵を安全に保持し、署名リクエストに応答する
- **Session**: フロントエンドで保持する接続状態。選択中のAccountId、接続ウォレット情報等

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Polkadot.js Extensionインストール済みユーザーの95%以上が、30秒以内にウォレット接続を完了できる
- **SC-002**: 投稿フローの成功率が95%以上を達成する
- **SC-003**: 異なるハイドラ間で同一AccountIdでの認証が100%成功する
- **SC-004**: ウォレット未インストール時に100%のケースで適切なガイダンスが表示される

## Assumptions

- ユーザーはPolkadot.js Extension（または互換ウォレット）をインストール済み、またはインストール可能な環境にいる
- モバイルユーザー向けには将来的にOSSスマホアプリを提供（本スコープ外）
- 既存のSubstrate/Polkadotエコシステムのウォレット標準に準拠
- polkadot-api（PAPI）を使用してチェーンとの通信を行う

## Out of Scope

- **モバイルアプリ**: 将来的にOSSスマホアプリを検討（別スペック）
- **独自ウォレット開発**: 既存エコシステムを活用
- **WebAuthn/パスキー**: 廃止決定
- **Identity Pallet統合**: 本スコープでは不使用

## Comparison: WebAuthn vs Wallet

| 項目 | WebAuthn（廃止） | Wallet（採用） |
|------|-----------------|---------------|
| 認証 | パスキー（P-256） | sr25519秘密鍵 |
| 署名 | WebAuthn API | ウォレット署名 |
| Extrinsic | Unsigned + オンチェーン検証 | **Signed（標準）** |
| Identity Pallet | 必要（Passkey管理） | **不要** |
| クロスハイドラ | rpId問題あり | ✅ ドメイン非依存 |
| 実装複雑性 | 高（独自検証ロジック） | **低（標準API）** |
| モバイル | ブラウザネイティブ | アプリ必要 |

## Related Documents

- [Implementation Tasks](tasks.md) - 実装タスク一覧
- [001-identity-pallet](../001-identity-pallet/spec.md) - Identity Pallet仕様（参考、本スコープでは不使用）


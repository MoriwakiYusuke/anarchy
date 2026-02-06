# Feature Specification: Identity Pallet

**Feature Branch**: `001-identity-pallet`  
**Created**: 2026-02-07  
**Status**: Draft  
**Input**: User description: "Identity Palletを作成したい"

## Overview

WebAuthn公開鍵をオンチェーンで管理し、「秘密鍵をユーザーに扱わせない」を実現するSubstrateパレット。

Constitution原則 **II. Keyless UX**【NON-NEGOTIABLE】を実装する中核コンポーネント。

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 新規ユーザーがIdentityを作成する (Priority: P1)

新規ユーザーがスマートフォンやPCのパスキー（Touch ID、Face ID、Windows Hello等）を使って、Anarchyネットワーク上に自分のIdentityを登録する。シードフレーズや秘密鍵の管理は一切不要。

**Why this priority**: システムの最も基本的な機能。ユーザーがネットワークに参加するための入り口であり、これがなければ他の全ての機能が使えない。

**Independent Test**: フロントエンドからWebAuthn登録フローを実行し、オンチェーンにIdentityが作成されていることをクエリで確認できる。

**Acceptance Scenarios**:

1. **Given** ユーザーがIdentityを持っていない, **When** パスキーで登録を実行する, **Then** 一意のIdentity IDが発行され、公開鍵がオンチェーンに保存される
2. **Given** ユーザーが登録を完了した, **When** 同じパスキーで再度登録を試みる, **Then** 既に登録済みであることが通知される
3. **Given** ユーザーがIdentityを作成した, **When** 別のフロントエンド（ハイドラ）からIdentityを照会する, **Then** 同じIdentity情報が取得できる

---

### User Story 2 - 既存ユーザーが新しいデバイスを追加する (Priority: P2)

ユーザーがPCで作成したIdentityに、スマートフォンのパスキーを追加登録する。これにより複数デバイスから同一Identityでアクセス可能になる。

**Why this priority**: 端末紛失時のリカバリや利便性のために、マルチデバイス対応は必須。ただしP1なしには成り立たないため優先度2。

**Independent Test**: 既存Identityに2台目のデバイスを追加し、どちらのデバイスからも認証・署名が成功することを確認。

**Acceptance Scenarios**:

1. **Given** ユーザーがIdentityを持っている, **When** 既存デバイスで新しいデバイスの追加を承認する, **Then** 新しいパスキー公開鍵がIdentityに紐付けられる
2. **Given** ユーザーが2台のデバイスを登録済み, **When** どちらのデバイスからでも投稿を作成する, **Then** 同一のIdentityとして投稿が記録される
3. **Given** ユーザーが複数デバイスを持つ, **When** 登録済みデバイスの一覧を確認する, **Then** 全ての登録デバイス情報が表示される

---

### User Story 3 - ユーザーがデバイスを削除する (Priority: P3)

紛失や機種変更により不要になったデバイス（パスキー）をIdentityから削除する。

**Why this priority**: セキュリティリカバリ機能として重要だが、登録機能が優先。

**Independent Test**: 3台登録済みの状態から1台を削除し、削除されたデバイスでは認証不可、残り2台では認証可能であることを確認。

**Acceptance Scenarios**:

1. **Given** ユーザーが複数デバイスを登録済み, **When** 既存デバイスで特定のデバイスを削除する, **Then** 削除されたパスキーは無効化される
2. **Given** ユーザーが1台のみ登録している, **When** そのデバイスを削除しようとする, **Then** 最後のデバイスは削除できないエラーになる
3. **Given** デバイスが削除された, **When** 削除されたデバイスで署名を試みる, **Then** 認証が拒否される

---

### Edge Cases

- 同一のパスキー公開鍵を複数のIdentityに登録しようとした場合 → 拒否（1つの公開鍵は1つのIdentityにのみ紐付く）
- Identity作成時にネットワークが切断された場合 → フロントエンドでリトライ可能、オンチェーンは冪等性を保証
- 悪意あるフロントエンドが不正な公開鍵を登録しようとした場合 → WebAuthn署名検証により拒否（Phase 1.4で実装）

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: システムはユーザーに一意のIdentity IDを発行できなければならない
- **FR-002**: システムは1つのIdentityに対して複数のパスキー公開鍵（最大10個）を紐付けられなければならない
- **FR-003**: ユーザーは既存デバイスの承認により新しいデバイスを追加できなければならない
- **FR-004**: ユーザーは登録済みデバイスを削除できなければならない（ただし最後の1台は削除不可）
- **FR-005**: システムは同一の公開鍵が複数のIdentityに登録されることを防止しなければならない
- **FR-006**: システムはIdentityの作成日時を記録しなければならない
- **FR-007**: システムは各パスキーの登録日時と最終使用日時を記録しなければならない

### Key Entities

- **Identity**: ユーザーを一意に識別するエンティティ。Identity ID、作成日時、紐付けられたパスキーのリストを持つ
- **Passkey**: WebAuthn公開鍵情報。公開鍵データ、登録日時、最終使用日時、デバイス名（オプション）を持つ
- **PasskeyId**: 公開鍵のハッシュ値から導出される一意識別子。Identity横断で重複不可

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: ユーザーはパスキー登録から投稿作成まで3分以内で完了できる
- **SC-002**: 1つのIdentityに対して最大10デバイスを登録でき、全てのデバイスから認証が成功する
- **SC-003**: デバイス追加・削除操作の成功率が99%以上である
- **SC-004**: パスキー登録フローでシードフレーズや秘密鍵の入力が一切発生しない（Keyless UXの達成）
- **SC-005**: 悪意あるフロントエンドからの不正な公開鍵登録がブロックされる（WebAuthn検証連携後）

---

## Assumptions

- WebAuthn署名検証（Phase 1.4）は本パレットの後に実装されるため、初期実装では公開鍵の形式検証のみ行い、署名検証は後から追加する
- フロントエンドは navigator.credentials API（WebAuthn）を使用してパスキー操作を行う
- パスキーの公開鍵はCOSEフォーマット（ES256/P-256）で提供される
- Identity IDはu64型で、シーケンシャルに発行される

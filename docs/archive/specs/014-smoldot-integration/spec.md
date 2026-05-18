# Feature Specification: smoldot Light Client統合

**Feature Branch**: `014-smoldot-integration`  
**Created**: 2026-02-24  
**Status**: Draft  
**Input**: User description: "smoldot Light Client統合を行う。既存のフロントエンドの見た目は一切変えない事、またチェーン側のロジックも変更しない事"

## 概要

フロントエンドアプリケーションにsmoldotライトクライアントを統合し、ユーザーがフルノードに依存せずにブロックチェーンに接続できるようにする。現在のWebSocket RPC接続をsmoldotベースの接続にアップグレードし、より分散化された、検閲耐性のある接続方式を実現する。

### 制約事項

- **UI変更禁止**: 既存のフロントエンドの見た目は一切変更しない
  - **例外**: 同期状態表示のテキスト変更のみ許可（「同期中...」「接続エラー」等）

### 許可事項

- **ブロックチェーン側の変更許可**: 必要に応じてパレットやランタイムの変更が可能
- **後方互換性不要**: 既存のWebSocket RPC接続コードは削除必須
- **クリーンアップ必須**: smoldot統合に伴い不要となったコードは全て削除する

## User Scenarios & Testing *(mandatory)*

### User Story 1 - smoldotでのチェーン接続 (Priority: P1)

ユーザーがフロントエンドアプリケーションを開くと、smoldotライトクライアントが自動的に起動し、ブロックチェーンネットワークに接続する。接続が確立されると、ユーザーは通常通りアプリケーションを使用できる。

**Why this priority**: これがコア機能であり、他のすべての機能がこれに依存するため最優先

**Independent Test**: アプリケーションを起動し、フルノードなしでブロック番号が表示され、増加していくことを確認する

**Acceptance Scenarios**:

1. **Given** ユーザーがフロントエンドアプリケーションを開く, **When** smoldotが初期化される, **Then** ブロックチェーンに接続され、接続状態が表示される
2. **Given** ローカルフルノードが起動していない状態, **When** アプリケーションを開く, **Then** smoldot経由でネットワークに接続できる
3. **Given** smoldotで接続中, **When** 最新ブロック番号を取得する, **Then** 正しいブロック番号が表示される

---

### User Story 2 - 既存機能のシームレスな動作 (Priority: P1)

smoldot接続時でも、投稿作成、残高表示、Faucetからのトークン取得など、すべての既存機能が従来通り動作する。

**Why this priority**: UIを変更しない制約があるため、機能の後方互換性は必須

**Independent Test**: smoldot接続状態で投稿を作成し、オンチェーンに記録されることを確認する

**Acceptance Scenarios**:

1. **Given** smoldotで接続中, **When** 投稿を作成する, **Then** 投稿がオンチェーンに正常に記録される
2. **Given** smoldotで接続中, **When** アカウント残高を確認する, **Then** 正確な残高が表示される
3. **Given** smoldotで接続中, **When** Faucetでトークンを請求する, **Then** トークンが正常に受け取れる

---

### User Story 3 - 初期同期中のフィードバック (Priority: P2)

smoldotの初期同期中、ユーザーに同期状況をフィードバックする。同期が完了するまで、操作を適切に制御する。

**Why this priority**: ユーザー体験の向上には重要だが、機能自体は同期完了後は不要

**Independent Test**: アプリケーションを起動し、同期中はローディング状態が表示され、同期完了後に操作可能になることを確認

**Acceptance Scenarios**:

1. **Given** アプリケーションを起動した直後, **When** smoldotが同期中, **Then** 同期中であることを示す状態が表示される
2. **Given** smoldotが同期中, **When** 同期が完了する, **Then** 通常の接続状態に遷移する
3. **Given** 同期中, **When** ユーザーが操作を試みる, **Then** 同期完了後に実行されるか、待機を促すメッセージが表示される

---

### User Story 4 - レガシーコードのクリーンアップ (Priority: P2)

smoldot統合に伴い、既存のWebSocket RPC接続関連コードを完全に削除し、コードベースをクリーンに保つ。

**Why this priority**: 後方互換性が不要なため、レガシーコードの削除は保守性向上のために重要

**Independent Test**: WebSocket RPC関連のimportやコードが完全に削除されていることを確認

**Acceptance Scenarios**:

1. **Given** smoldot統合が完了, **When** コードベースを検索, **Then** `getWsProvider`のimportが存在しない
2. **Given** smoldot統合が完了, **When** ビルドを実行, **Then** 未使用コードの警告が発生しない

---

### Edge Cases

- smoldotのチェーンスペックが最新でない場合、どのように検出・更新するか?
  - **想定対応**: 起動時にチェーンスペックのバージョンを確認し、不一致時は警告を表示
- ブラウザがWeb Workerをサポートしていない場合はどうするか?
  - **想定対応**: 機能検出を行い、未対応の場合はエラーメッセージを表示（フォールバックなし）
- 同期に非常に長い時間がかかる場合のタイムアウト処理は?
  - **想定対応**: タイムアウト後にError状態に遷移、ブラウザリロードでリトライ
- ネットワーク切断・再接続時の挙動は?
  - **想定対応**: smoldotの自動再接続機能を活用

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: システムはsmoldotライトクライアントを使用してブロックチェーンに接続できなければならない
- **FR-002**: システムは既存のすべてのAPI呼び出し（残高取得、投稿作成、Faucet利用）をsmoldot経由で実行できなければならない
- **FR-003**: システムはsmoldotの初期同期状態を追跡し、適切なUI状態（既存のisConnected状態の拡張）を提供しなければならない
- **FR-004**: ~~システムはsmoldot接続失敗時にWebSocket RPC接続にフォールバックできなければならない~~ （削除：後方互換性不要のため、smoldotのみをサポート）
- **FR-005**: システムはAnarchyチェーン用のチェーンスペックをビルド時に静的に埋め込み、適切に管理しなければならない
- **FR-006**: システムはsmoldotをWeb Worker内で実行し、メインスレッドのブロッキングを回避しなければならない
- **FR-007**: システムは既存のpolkadot-api統合と互換性を維持しなければならない
- **FR-008**: システムは不要となったWebSocket RPC関連コードを完全に削除しなければならない

### Non-Functional Requirements

- **NFR-001**: smoldotの初期化は5秒以内に完了すること
- **NFR-002**: 初期同期は典型的なネットワーク条件下で60秒以内に完了すること
- **NFR-003**: smoldotのWasmバンドルサイズは追加で2MB以下に抑えること
- **NFR-004**: メインスレッドのブロッキングを避け、UIの応答性を維持すること

### Key Entities

- **ChainSpec**: Anarchyブロックチェーンのチェーン仕様（ジェネシスハッシュ、ブートノード情報等を含む）
- **LightClientProvider**: smoldotを利用したPAPI用プロバイダー（既存のWebSocketプロバイダーの代替）
- **ConnectionState**: 接続状態を管理するステートマシン（Initializing → Syncing → Connected / Error の4状態）

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: ユーザーがローカルフルノードなしでアプリケーションを使用でき、投稿作成が成功する
- **SC-002**: アプリケーションの初回起動から操作可能状態まで2分以内に到達する
- **SC-003**: 既存のすべてのE2Eテストがsmoldot接続モードでも合格する
- **SC-004**: レガシーコード（WebSocket RPC関連）が完全に削除されている
- **SC-005**: 追加バンドルサイズが2MB以下に収まる

## Assumptions

- polkadot-apiライブラリがsmoldotプロバイダー（`@polkadot-api/smoldot`）をサポートしている
- Anarchyチェーンのチェーンスペックはビルド時に取得可能（ブートノード情報を含む）
- ユーザーのブラウザはWeb Worker、WebAssemblyをサポートしている（未対応時はエラー表示）
  - **注**: smoldot 2.xはSharedArrayBufferを必要としない
- ブートノード情報はチェーンスペックJSONに含まれている
- 後方互換性は不要であり、WebSocket RPC接続は完全に削除される

## Out of Scope

- フロントエンドUIデザインの変更
- 新しいUI要素の追加（既存の接続状態表示を流用）
- smoldot専用の詳細な同期進捗表示（将来のエンハンスメント候補）
- オフライン対応機能

## クリーンアップ対象

- 既存のWebSocket RPCプロバイダー関連コード（`getWsProvider`等）
- フォールバック接続ロジック（smoldotのみに統一）
- 不要となった接続状態管理コード

## Clarifications

### Session 2026-02-24

- Q: smoldot接続失敗時の最終状態はどうすべきか？ → A: `Initializing → Syncing → Connected / Error` の4状態を採用
- Q: チェーンスペックはどのように管理するか？ → A: フロントエンドビルド時にチェーンスペックJSONを静的に埋め込む
- Q: Error状態からのリカバリー方法は？ → A: ブラウザリロードで対応（UI変更禁止のためリトライボタンは追加しない）
- Q: ブートノードはどのように設定するか？ → A: チェーンスペックJSON内にブートノードアドレスを含める
- Q: 同期中の表示方法は？ → A: 例外的に同期状態表示のUI変更を許可（既存の状態表示テキストを「同期中...」等に変更）

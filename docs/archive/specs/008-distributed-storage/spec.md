# Feature Specification: Storage MVP - Phase 1 (Storage Registry & P2P)

**Feature Branch**: `008-distributed-storage`  
**Created**: 2026-02-09  
**Status**: Draft  
**Input**: User description: "分散ストレージ Phase 1 - 断片登録とP2P送受信のみ。報酬・罰則なし。"

## 概要

分散ストレージシステムの最小実装（MVP）。データの保存場所（PeerID）をチェーンに登録し、libp2pで断片を送受信できる状態を目指す。

**Phase 1のスコープ**:
- ✅ 断片メタデータのオンチェーン登録（カタログ機能）
- ✅ ストレージノードの登録
- ✅ libp2pによる断片の送受信
- ❌ ~~PoST（Proof of Spacetime）~~ → Phase 2
- ❌ ~~報酬分配~~ → Phase 2
- ❌ ~~スラッシング~~ → Phase 3
- ❌ ~~自己修復プロトコル~~ → Phase 3

**関連ドキュメント**: [StorageStrategy.md](../../../architecture/storage-strategy.md)

## フェーズ構成

```
Phase 1: Storage Registry & P2P (今回)
├── 断片メタデータのチェーン登録
├── ストレージノード登録
└── libp2pでの断片送受信

Phase 2: Simple Proof & Rewards (次回)
├── 抜き打ちチェック (Proof of Storage)
├── 報酬プール管理
└── チャレンジ/レスポンス

Phase 3: Slashing & Repair (将来)
├── 応答失敗時のスラッシング
├── 自己修復プロトコル
└── k-of-n健全性監視
```

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 断片メタデータの登録 (Priority: P1)

投稿者がデータ断片のメタデータ（ID、サイズ、保持者PeerID）をチェーンに登録する。これにより「どのノードがどの断片を持っているか」がネットワーク全体で参照可能になる。

**Why this priority**: ストレージネットワーク構築の最初の一歩。断片の「索引」がなければ、誰がデータを持っているか分からない。

**Independent Test**: `register_fragment`エクストリンシックを実行し、チェーン上にFragmentMetadataが保存されることを確認。

**Acceptance Scenarios**:

1. **Given** 投稿者がFragment IDとサイズを指定する, **When** `register_fragment`を実行する, **Then** チェーン上にFragmentMetadataが保存される
2. **Given** 既に登録済みのFragment IDが指定された, **When** `register_fragment`を実行する, **Then** `FragmentAlreadyExists`エラーが返される
3. **Given** 断片サイズが上限を超えている, **When** `register_fragment`を実行する, **Then** `FragmentTooLarge`エラーが返される

---

### User Story 2 - ストレージノードの登録 (Priority: P1)

ストレージノード運営者が自分のノード（PeerID、提供容量）をチェーンに登録する。これにより他のユーザーが断片の保存先としてこのノードを選択できるようになる。

**Why this priority**: 断片を受け取るノードの存在が前提。ノード登録なしにはデータの配置先が決まらない。

**Independent Test**: `register_node`エクストリンシックを実行し、チェーン上にStorageNode情報が保存されることを確認。

**Acceptance Scenarios**:

1. **Given** 運営者がPeerIDと提供容量を指定する, **When** `register_node`を実行する, **Then** チェーン上にStorageNode情報が保存される
2. **Given** 既に登録済みのPeerIDが指定された, **When** `register_node`を実行する, **Then** `NodeAlreadyRegistered`エラーが返される
3. **Given** 登録済みノードの情報を更新したい, **When** `update_node`を実行する, **Then** 提供容量等の情報が更新される
4. **Given** ノード運営を終了したい, **When** `unregister_node`を実行する, **Then** ノード情報がチェーンから削除される

---

### User Story 3 - libp2pでの断片送受信 (Priority: P1)

クライアントがストレージノードに断片データをlibp2p経由で送信し、ノードがディスクに保存する。また、保存された断片をクライアントがリクエストして取得できる。

**Why this priority**: 実際のデータ移動の仕組み。チェーン上の「索引」と実際の「データ」を結びつける。

**Independent Test**: ストレージノードデーモンを起動し、テスト断片を送信→受信→取得のフローが完了することを確認。

**Acceptance Scenarios**:

1. **Given** ストレージノードが起動している, **When** クライアントがFragment IDを指定してデータを送信する, **Then** ノードが断片をディスクに保存し、保持表明をチェーンに記録する
2. **Given** ストレージノードが断片を保持している, **When** クライアントがFragment IDを指定してリクエストする, **Then** 断片データが返却される
3. **Given** 存在しないFragment IDが指定された, **When** クライアントがリクエストする, **Then** `FragmentNotFound`エラーが返される
4. **Given** ノードのディスク容量が上限に達した, **When** 新しい断片の保存リクエストが来る, **Then** `StorageCapacityExceeded`エラーが返される

---

### User Story 4 - ストレージノードのセットアップ (Priority: P2)

新規参加者がストレージノードデーモンをインストールし、ストレージ容量を設定して運営を開始できる。バリデーターノードとは独立して動作する。

**Why this priority**: ネットワークの分散化に必要。簡単にセットアップできることで参入障壁を下げる。

**Independent Test**: ストレージノードデーモンをインストール・設定し、チェーンに登録されてP2P接続が確立されることを確認。

**Acceptance Scenarios**:

1. **Given** ユーザーがストレージノードデーモンをインストールした, **When** 設定ファイルで容量制限とPeerIDを設定して起動する, **Then** libp2pネットワークに参加し待機状態になる
2. **Given** ストレージノードが起動中である, **When** チェーンに`register_node`が実行される, **Then** ノードが公式に登録され断片受け入れ可能になる
3. **Given** ストレージノードを停止したい, **When** シャットダウンコマンドを実行する, **Then** 進行中の転送を完了してから終了する

---

### Edge Cases

- **PeerID衝突**: 異なるオペレーターが同じPeerIDで登録しようとする → 最初の登録者のみ有効、後からの登録は拒否
- **ノード未登録での断片送信**: チェーンに登録されていないノードに断片を送信 → 転送は成功するが、保持表明はチェーンに記録されない（非推奨だが許容）
- **大容量断片**: 1MB超の断片を送信しようとする → 分割を促すエラーを返す
- **ネットワーク切断中の操作**: libp2p接続が切れた状態での送受信 → 再接続を試みてタイムアウト後にエラー
- **重複保持表明**: 同じノードが同じ断片の保持を二重に表明 → 後からの表明は無視（べき等）
- **Wallet Drain Attack**: 悪意ある第三者が大量の断片を送りつけ、ノードのウォレットから手数料を枯渇させる攻撃 → 以下の二重防御で対策:
  1. **登録済み断片のみ受け入れ**: チェーン上にFragmentMetadataが存在する断片のみPUT許可
  2. **レート制限**: 1分あたり最大10件のdeclare_holding送信（設定可能）

## Requirements *(mandatory)*

### Functional Requirements

#### Storage Pallet（オンチェーン）

- **FR-001**: System MUST `pallet-storage`を実装し、断片メタデータの登録を行う
- **FR-002**: System MUST 各断片に対してFragment IDをユーザー指定で受け付ける（Blake2ハッシュ形式）
- **FR-003**: System MUST 断片メタデータを保存する（Fragment ID、サイズ、作成者AccountId）
- **FR-004**: System MUST ストレージノードの登録・更新・登録解除を管理する
- **FR-005**: System MUST ノード情報を保存する（PeerID、オペレーターAccountId、提供容量）
- **FR-006**: System MUST 保持表明（`declare_holding`）を受け付け、断片とノードの紐付けを記録する
- **FR-007**: System MUST 保持取消（`revoke_holding`）を受け付け、紐付けを解除する
- **FR-008**: System MUST 断片の保持者一覧を参照可能にする

#### Storage Node Daemon（オフチェーン）

- **FR-101**: System MUST libp2p経由で断片データを受信し、ローカルディスクに保存する
- **FR-102**: System MUST libp2p経由で断片データのリクエストを受け付け、保存済み断片を返却する
- **FR-103**: System MUST 設定されたディスククォータ内でデータを管理する
- **FR-104**: System MUST 断片の保存成功時に、自動的にチェーンへ保持表明を送信する
- **FR-105**: System MUST 設定ファイルでPeerID、ディスクパス、容量上限を指定可能にする
- **FR-106**: System MUST シャットダウン時にgracefulに転送を完了する
- **FR-107**: System MUST チェーン上にFragmentMetadataが存在する断片のみPUT受け入れする（Wallet Drain Attack対策）
- **FR-108**: System MUST declare_holdingのレート制限を実装する（デフォルト: 1分あたり最大10件）
- **FR-109**: System MUST tracing crateでログ出力し、基本メトリクス（断片数、容量使用率）を提供する

### Key Entities *(include if feature involves data)*

- **Fragment（断片）**: クライアント側で生成されたデータの一片。Fragment ID（Blake2ハッシュ）で識別。
- **FragmentMetadata**: チェーン上に保存される断片のメタデータ。Fragment ID、サイズ（バイト）、作成者AccountId、作成ブロック番号を含む。
- **StorageNode**: ストレージノードの登録情報。PeerID、オペレーターAccountId、提供容量（バイト）を含む。
- **HoldingDeclaration**: 特定のノードが特定の断片を保持していることの表明。ノードPeerID、Fragment ID、表明ブロック番号を含む。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 断片登録からチェーン確定までの時間が平均6秒以内（1ブロック）
- **SC-002**: ストレージノード間の断片転送が1MB/秒以上の速度で完了する（ローカルネットワーク環境）
- **SC-003**: 新規ストレージノードのセットアップが30分以内に完了（ドキュメント参照のみで）
- **SC-004**: ストレージノードが10GB分の断片を保持しても、デーモンのメモリ使用量が200MB未満
- **SC-005**: 断片の保存・取得の成功率が99%以上（ネットワーク正常時）

### Testing Requirements

#### Pallet Tests (Rust)

- **T-001**: 断片登録が成功しFragmentMetadataが保存される
- **T-002**: 重複Fragment IDで登録するとエラーになる
- **T-003**: ストレージノード登録が成功しStorageNode情報が保存される
- **T-004**: 重複PeerIDで登録するとエラーになる
- **T-005**: ノード情報更新が正常に動作する
- **T-006**: ノード登録解除が正常に動作する
- **T-007**: 保持表明が正常に記録される
- **T-008**: 保持取消が正常に記録される
- **T-009**: 断片の保持者一覧が正しく返却される

#### Storage Node Daemon Tests

- **T-101**: libp2p経由で断片を受信・保存できる
- **T-102**: libp2p経由で断片を返却できる
- **T-103**: ディスククォータ制限が正しく動作する
- **T-104**: 存在しない断片のリクエストでエラーが返る
- **T-105**: 設定ファイルの読み込みが正常に動作する
- **T-106**: gracefulシャットダウンが正常に動作する

#### Integration Tests

- **T-201**: E2E: ノード登録 → 断片登録 → 断片送信 → 保持表明
- **T-202**: E2E: 断片取得リクエスト → 断片返却
- **T-203**: E2E: 2ノード間での断片転送

## Assumptions

- 断片サイズは最大1MBとする（それ以上は複数断片に分割）
- PeerIDはlibp2pの標準形式（`12D3KooW...`）を使用
- 保持表明は「そのノードが持っていると主張している」だけで、Phase 1では検証なし（性善説）
- チェーン接続にはSubxtを使用（pallet-faucetと同じパターン）
- **Peer Discovery**: クライアントはチェーン上のStorageNodes一覧から送信先ノードを選択する（Kademlia DHT不要）
- **Fragment Deletion**: Phase 1ではFragmentMetadataの削除機能なし。一度登録された断片メタデータは永続。`revoke_holding`は保持表明の解除のみ。
- **Auto-declare**: ストレージノードは断片保存成功時に自動的に`declare_holding`を送信する

## Out of Scope (Phase 2以降)

以下の機能はPhase 1では実装しない：

| 機能 | 実装予定フェーズ | 理由 |
|------|----------------|------|
| Proof of Storage（抜き打ちチェック） | Phase 2 | 報酬の前提条件 |
| 報酬分配 | Phase 2 | PoSと同時実装が効率的 |
| 報酬プール管理 | Phase 2 | 経済モデルの検討が必要 |
| スラッシング | Phase 3 | 自己修復と同時実装が効率的 |
| 自己修復プロトコル | Phase 3 | k-of-n設計の検討が必要 |
| 健全性監視 | Phase 3 | 自己修復の前提条件 |
| Tor Hidden Service対応 | Phase 2+ | 006-libp2p-torとの統合 |

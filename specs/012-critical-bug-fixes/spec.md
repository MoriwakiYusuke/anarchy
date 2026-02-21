# Feature Specification: Critical Bug Fixes (HIGH Priority 13 Issues)

**Feature Branch**: `012-critical-bug-fixes`  
**Created**: 2026-02-21  
**Status**: Draft  
**Input**: HIGH優先度13件の重大なバグ・セキュリティ脆弱性を修正

## Clarifications

### Session 2026-02-21

- Q: チャレンジを発行できる「validator」の定義は？ → A: 他の登録済みストレージノード（相互チャレンジモデル）
- Q: チャレンジの有効期限は何ブロック？ → A: 50ブロック（約5分@6秒/ブロック）
- Q: Gossip同時接続の最大数は？ → A: 128接続（標準的なP2Pネットワーク）
- Q: Gossipレジストリの最大エントリ数は？ → A: 10,000エントリ
- Q: RPC再接続パラメータは？ → A: 最大10回、初期1秒、最大60秒（バランス型）

## 概要

本仕様は、コードレビューで検出された13件のHIGH優先度issue（セキュリティ脆弱性・重大バグ）を修正するためのものである。対象コンポーネントは、ブロックチェーンパレット、ノードネットワーキング、Wasmエンジン、ストレージノード、フロントエンドに及ぶ。

### Issue一覧

| # | 指摘 | ファイル |
|---|------|---------|
| 1 | issue_challenge にスパム防止不十分（任意アカウントを challenged_node に指定可能） | pallets/storage/src/lib.rs |
| 2 | チャレンジ期限切れ処理が完全に未実装（on_finalize で処理なし → PendingChallenges 無限肥大化） | pallets/storage/src/lib.rs |
| 3 | 報酬の二重計上（ProofRecords.pending_reward と PendingRewards 両方に加算） | pallets/storage/src/lib.rs |
| 4 | register_kzg_fragment が直接 extrinsic として公開（create_post 経由なしに登録→報酬不正取得） | pallets/storage/src/lib.rs |
| 5 | TAU_G2_BYTES がパレットとノードで重複定義（不整合リスク、末尾ゼロ埋め疑惑） | kzg.rs / storage.rs |
| 6 | Gossip受信接続を無条件Accept（DoS脆弱性） | node/src/gossip/mod.rs |
| 7 | Gossipメッセージによるレジストリ肥大化制限なし | node/src/gossip/mod.rs |
| 8 | sss_split_byte 内の expect() でRNG失敗時Wasmパニック | wasm-engine/src/kzg/key_sss.rs |
| 9 | vss_prove がコミットメントとの整合性を検証していない（let _ = commitment） | wasm-engine/src/kzg/proof.rs |
| 10 | チャレンジモニターがメインループに統合されていない（機能が動作しない） | storage-node/src/main.rs |
| 11 | フェイルオーバー後に subxt クライアントが再接続されない | storage-node/src/chain/mod.rs |
| 12 | PostItem ごとに独立 Web Worker 生成（50投稿 = 50 Worker → クラッシュ） | frontend/src/components/PostItem.tsx |
| 13 | useScore がモック実装のまま出荷、useStorage.ts の責務過多(516行) | frontend/src/hooks/ |

## User Scenarios & Testing

### User Story 1 - ストレージノードオペレータがセキュアにチャレンジ応答する (Priority: P1)

ストレージノードオペレータとして、私のノードに対するチャレンジが正当なvalidatorからのみ発行され、不正なスパムチャレンジでリソースを消費されないようにしたい。また、チャレンジの期限切れが適切に処理されることで、ブロックチェーンの状態肥大化を防ぎたい。

**Why this priority**: セキュリティと経済的インセンティブの根幹。スパムチャレンジはDoS攻撃のベクトルとなり、期限切れ未処理はチェーン状態の無限肥大化を招く。

**Independent Test**: チャレンジ発行制限と期限切れガベージコレクションが正常動作することをパレットテストで検証可能。

**Acceptance Scenarios**:

1. **Given** 登録されていないアカウント, **When** issue_challengeを呼び出す, **Then** エラー「NotRegisteredStorageNode」が返される
2. **Given** 有効な登録済みストレージノード, **When** 存在しないノードに対してissue_challengeを呼び出す, **Then** エラー「ChallengedNodeNotRegistered」が返される
3. **Given** PendingChallengesに古いチャレンジが存在する, **When** 期限ブロックを超過してon_finalizeが実行される, **Then** 期限切れチャレンジが削除され、該当ノードのスコアが減算される
4. **Given** 正当なチャレンジ, **When** 期限内にproof提出, **Then** チャレンジが解消され報酬が付与される

---

### User Story 2 - 報酬システムの一貫性確保 (Priority: P1)

ストレージノードとして、正当なfragment保持証明に対して報酬が正確に1回だけ計上されることを保証したい。また、create_postを経由せずにfragmentを不正登録して報酬を得ることができないようにしたい。

**Why this priority**: 経済インセンティブの基盤。二重計上や不正登録は報酬プールの不正流出につながり、ネットワーク経済を破壊する。

**Independent Test**: 報酬計上ロジックとregister_kzg_fragmentのアクセス制御をパレットテストで検証可能。

**Acceptance Scenarios**:

1. **Given** validなKZG証明を提出, **When** prove_holding_kzgが成功, **Then** 報酬は PendingRewards にのみ1回計上される
2. **Given** 外部アカウント, **When** register_kzg_fragmentを直接呼び出す, **Then** エラー「NotPostPallet」または呼び出し不可
3. **Given** create_post経由でfragment登録, **When** 正常に処理される, **Then** KzgFragmentsに正しくcommitmentが登録される

---

### User Story 3 - ノードネットワーキングのDoS耐性向上 (Priority: P1)

ノードオペレータとして、Gossipネットワークがスパム接続や悪意あるメッセージによるDoS攻撃に耐性を持ち、レジストリが無限肥大化しないことを保証したい。

**Why this priority**: ネットワーク可用性の根幹。無条件Acceptとレジストリ肥大化はノードクラッシュを招き、ネットワーク全体の可用性を損なう。

**Independent Test**: Gossip接続制限と登録制限の動作をユニットテストで検証可能。

**Acceptance Scenarios**:

1. **Given** 新規接続要求, **When** 接続数が上限を超過, **Then** 接続が拒否される
2. **Given** 悪意ある大量メッセージ, **When** レジストリサイズが上限に達する, **Then** 新規登録が拒否されるか古いエントリが削除される
3. **Given** 正当な接続・メッセージ, **When** 通常運用, **Then** 正常に処理される

---

### User Story 4 - Wasmエンジンの堅牢性向上 (Priority: P2)

フロントエンドユーザーとして、Wasm暗号エンジンがRNG失敗などのエッジケースでもクラッシュせずにエラーを適切に処理し、vss証明がコミットメントと整合性を持つことを保証したい。

**Why this priority**: ユーザー体験とセキュリティ。Wasmパニックはアプリケーションクラッシュを招き、整合性検証不備は不正データの混入を許す。

**Independent Test**: sss_split_byteのエラーハンドリングとvss_proveの整合性検証をWasmテストで検証可能。

**Acceptance Scenarios**:

1. **Given** RNGが失敗, **When** sss_split_byteが呼び出される, **Then** パニックせずにResultエラーを返す
2. **Given** コミットメント不整合, **When** vss_proveが呼び出される, **Then** エラーが返される
3. **Given** 正常なデータ, **When** vss_proveが呼び出される, **Then** コミットメントと整合する証明が生成される

---

### User Story 5 - ストレージノードの信頼性向上 (Priority: P2)

ストレージノードオペレータとして、チャレンジモニターが正常に動作し、RPC接続断絶後に自動的に再接続されることで、ノードのダウンタイムを最小化したい。

**Why this priority**: 運用信頼性。チャレンジ応答失敗はスコア減算と報酬損失につながり、再接続失敗は長期ダウンタイムを招く。

**Independent Test**: チャレンジモニター統合と再接続ロジックを統合テストで検証可能。

**Acceptance Scenarios**:

1. **Given** ストレージノードが起動中, **When** 自ノード宛のチャレンジがチェーンに登録される, **Then** チャレンジモニターが検出しproof提出を開始する
2. **Given** RPCエンドポイントが一時的に断絶, **When** 再接続可能になる, **Then** subxtクライアントが自動的に再接続する
3. **Given** RPC断絶が長期化, **When** 再試行回数が上限に達する, **Then** エラーログを出力し適切にリカバリーを試行する

---

### User Story 6 - フロントエンドパフォーマンス最適化 (Priority: P2)

フロントエンドユーザーとして、多数の投稿が表示されてもブラウザがクラッシュせず、スムーズなスクロールと操作を維持したい。

**Why this priority**: ユーザー体験。50投稿で50 Web Workerはブラウザリソース枯渇とクラッシュを招く。

**Independent Test**: PostItemのWeb Worker共有化をフロントエンドテストで検証可能。

**Acceptance Scenarios**:

1. **Given** 100件の投稿を表示, **When** ページをスクロール, **Then** ブラウザがクラッシュせず、メモリ使用量が安定している
2. **Given** 複数のPostItem, **When** 暗号処理が必要, **Then** 共有されたWeb Worker（またはWorkerプール）が使用される
3. **Given** Worker内でエラー発生, **When** 処理が失敗, **Then** 適切にエラーがハンドリングされUIに表示される

---

### User Story 7 - フロントエンドコード品質向上 (Priority: P3)

開発者として、useScoreが実際のブロックチェーンデータを取得し、useStorage.tsが適切な責務に分割されていることで、保守性とテスト容易性を向上させたい。

**Why this priority**: 開発効率と長期保守性。モック実装の出荷はユーザーに不正確なデータを表示し、責務過多は技術的負債を蓄積する。

**Independent Test**: useScoreの実装とuseStorage分割をユニットテストで検証可能。

**Acceptance Scenarios**:

1. **Given** useScoreフックを使用, **When** ノードスコアを取得, **Then** ブロックチェーンから実際のスコアデータが取得される
2. **Given** useStorage.ts, **When** 分割後, **Then** 各ファイルが200行以下で単一責務を持つ
3. **Given** 分割後のフック群, **When** テスト実行, **Then** 各フックが単体でテスト可能

---

### User Story 8 - TAU_G2_BYTES定数の一元化 (Priority: P2)

開発者として、TAU_G2_BYTES定数が単一ソースで管理され、パレットとストレージノード間で不整合が生じないことを保証したい。

**Why this priority**: データ整合性。定数の不整合はKZG検証失敗を招き、正当なproofが拒否される可能性がある。

**Independent Test**: 定数の一元化と参照整合性をビルド時検証可能。

**Acceptance Scenarios**:

1. **Given** TAU_G2_BYTES定数, **When** ビルド, **Then** 単一の定義元からのみ参照される
2. **Given** 末尾ゼロ埋め疑惑の定数, **When** 検証, **Then** 正しいBLS12-381 G2ポイントであることが確認される

---

### Edge Cases

- チャレンジ期限切れ処理中にノードが再起動した場合の状態整合性
- 報酬計上中に残高オーバーフローが発生した場合の処理
- Gossip接続が上限に達した状態で正当な新規ノードが参入する場合
- RNG失敗が連続して発生した場合のリトライ戦略
- RPC再接続中に複数のチャレンジが同時発生した場合のキューイング
- Web Workerプールが枯渇した場合のフォールバック戦略

## Requirements

### Functional Requirements

#### Pallet Storage (Issue 1-4)

- **FR-001**: issue_challenge呼び出し元は登録済みストレージノード（他ノードへの相互チャレンジ）であることを検証MUST
- **FR-002**: issue_challengeの対象ノードが登録済みであることを検証MUST
- **FR-003**: on_finalizeで期限切れPendingChallengesを削除MUST
- **FR-004**: 期限切れチャレンジの対象ノードのスコアを減算MUST
- **FR-005**: prove_holding_kzg成功時、報酬はPendingRewardsにのみ計上MUST
- **FR-006**: ProofRecordsからpending_rewardフィールドを削除（または使用しない）MUST
- **FR-007**: register_kzg_fragmentは外部extrinsicとして公開してはならない MUST
- **FR-008**: register_kzg_fragmentはcreate_post内部からのみ呼び出し可能MUST

#### Node Gossip (Issue 6-7)

- **FR-009**: Gossip接続受け入れ時、同時接続数上限（128接続）チェックMUST
- **FR-010**: 接続数上限超過時、新規接続を拒否MUST
- **FR-011**: レジストリサイズに上限（10,000エントリ）を設けMUST
- **FR-012**: レジストリ上限到達時、古いエントリを削除または新規登録を拒否MUST

#### Wasm Engine (Issue 8-9)

- **FR-013**: sss_split_byte内でRNG失敗時、panicではなくResultエラーを返すMUST
- **FR-014**: vss_proveはコミットメントとの整合性を検証MUST
- **FR-015**: コミットメント不整合時、エラーを返すMUST

#### Storage Node (Issue 10-11)

- **FR-016**: チャレンジモニターをメインイベントループに統合MUST
- **FR-017**: subxtクライアント接続断絶時、自動再接続を試行MUST（最大10回）
- **FR-018**: 再接続にはexponential backoff（初期1秒、最大60秒）を適用MUST
- **FR-019**: 再接続失敗時、適切なエラーログを出力MUST

#### Frontend (Issue 12-13)

- **FR-020**: PostItemは共有Web Worker（またはWorkerプール）を使用MUST
- **FR-021**: Worker数の上限を設けMUST（推奨: CPU数に応じた動的調整）
- **FR-022**: useScoreは実際のブロックチェーンからスコアデータを取得MUST
- **FR-023**: useStorage.tsを適切な責務に分割MUST（各ファイル200行以下目標）

#### Shared Code (Issue 5)

- **FR-024**: TAU_G2_BYTESは単一の定義元に統合MUST
- **FR-025**: TAU_G2_BYTESがvalidなBLS12-381 G2ポイントであることを検証MUST

### Key Entities

- **PendingChallenge**: チャレンジID、発行者、対象ノード、期限ブロック番号
- **ProofRecord**: fragment ID、ノードID、検証状態
- **PendingReward**: ノードID、未払い報酬額
- **GossipConnection**: ピアID、接続時刻、最終アクティブ時刻
- **WorkerPool**: ワーカー数、タスクキュー、使用状況

## Success Criteria

### Measurable Outcomes

- **SC-001**: スパムチャレンジが100%拒否される（未登録validator/未登録ノード）
- **SC-002**: 期限切れチャレンジが該当ブロック以降の次のファイナライズで削除される
- **SC-003**: 報酬計上が1箇所のみで行われ、二重計上が0件
- **SC-004**: 外部からのregister_kzg_fragment呼び出しが100%拒否される
- **SC-005**: TAU_G2_BYTES定義が1箇所のみに統合される
- **SC-006**: Gossip同時接続数が設定上限を超えない
- **SC-007**: sss_split_byteのRNG失敗時、panicではなくエラーが返される
- **SC-008**: vss_proveのコミットメント不整合が検出可能
- **SC-009**: チャレンジモニターが自ノード宛チャレンジを検出して応答可能
- **SC-010**: RPC断絶後、設定時間内に自動再接続が成功
- **SC-011**: 100投稿表示時、Web Worker数が上限（例: 4-8）以内に収まる
- **SC-012**: useScoreがモックではなく実際のブロックチェーンデータを返す
- **SC-013**: useStorage.ts分割後、各ファイルが200行以下

## Assumptions

- チャレンジ有効期限は50ブロック（約5分@6秒/ブロック）
- チャレンジ期限切れのペナルティスコア減算量はランタイム設定で定義済み
- Gossip接続上限は128接続（ノード設定ファイルで変更可能）
- Gossipレジストリ上限は10,000エントリ
- RPC再接続: 最大10回リトライ、初期待機1秒、最大待機60秒（exponential backoff）
- Web Workerプールサイズはnavigator.hardwareConcurrencyを参考に決定
- useStorage.tsの責務分割は既存APIを破壊しない形で実施

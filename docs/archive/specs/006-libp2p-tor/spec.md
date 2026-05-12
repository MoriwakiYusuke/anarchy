# Feature Specification: libp2p + Tor統合

**Feature Branch**: `006-libp2p-tor`  
**Created**: 2026-02-08  
**Status**: Draft  
**Input**: User description: "1.3 libp2p + Tor統合: ノード間匿名通信の実現"

## 概要

Anarchyノード間の通信をTorネットワーク経由で行うことで、ノード運営者のIPアドレスを秘匿し、検閲耐性を実現する。段階的なアプローチにより、即時に検証可能な構成から始め、将来的にアプリケーション内蔵Torへ移行する。

## User Scenarios & Testing *(mandatory)*

### User Story 1 - ノード運営者が匿名でノードを起動する (Priority: P1)

ノード運営者として、自分のIPアドレスを他のノードやネットワーク監視者に知られることなく、Anarchyネットワークに参加したい。これにより、政治的圧力や検閲からノードを保護できる。

**Why this priority**: 匿名化はAnarchyプロジェクトの核心的価値であり、これなしではユーザープライバシーが担保できない。他のすべての機能の前提条件となる。

**Independent Test**: torsocksを使用してノードを起動し、外部から見えるIPアドレスがTor出口ノードのものになっていることを確認。同時に、他ノードとのブロック同期が正常に行われることを検証。

**Acceptance Scenarios**:

1. **Given** Torがインストールされた環境で、**When** `torsocks ./anarchy-node`でノードを起動する、**Then** 他のノードからは送信元IPがTor出口ノードのIPとして見える
2. **Given** Tor経由で起動したノードが、**When** ネットワークに接続を試みる、**Then** ピア発見とブロック同期が正常に完了する
3. **Given** ネットワーク監視者が通信を傍受する場合、**When** ノード間通信を分析しても、**Then** 実際のノード運営者IPを特定できない

---

### User Story 2 - ノードがOnion Serviceとして受信を受け付ける (Priority: P2)

ノード運営者として、受信接続もTorを経由して受け付けたい。これにより、双方向の匿名通信を実現し、ファイアウォール/NAT背後でも他ノードからの接続を受け入れられる。

**Why this priority**: 送信のみのTor化では受信側IPが露出する。完全な匿名化には受信側もOnion化が必要。また、NAT越えの副次的メリットもある。

**Independent Test**: Onion Serviceを設定したノードに対し、`.onion`アドレス経由で別ノードが接続でき、ブロックデータの送受信が正常に行われることを確認。

**Acceptance Scenarios**:

1. **Given** Onion Serviceが設定されたノードに対し、**When** 別のTorノードが`.onion`アドレスで接続を試みる、**Then** 接続が成功しピアとして認識される
2. **Given** NATやファイアウォール背後にあるノードが、**When** Onion Serviceを有効にする、**Then** ポート開放なしで外部ノードからの接続を受け入れられる

---

### User Story 3 - 運営者がTorモードを選択できる (Priority: P3)

ノード運営者として、環境や用途に応じてTor使用の有無を選択したい。開発・テスト環境ではTorなしで高速に、本番環境ではTor強制で運用したい。

**Why this priority**: 開発効率とセキュリティのバランス。すべての環境でTor強制は開発を遅延させる。

**Independent Test**: 起動パラメータ（`--tor-mode=off|outbound-only|forced`）を変更し、それぞれのモードで期待通りの動作をすることを確認。

**Acceptance Scenarios**:

1. **Given** `--tor-mode=off`で起動した場合、**When** ノードが通信する、**Then** 通常のTCP接続が使用される（開発用）
2. **Given** `--tor-mode=outbound-only`で起動した場合、**When** ノードが通信する、**Then** 送信はTor経由だが、受信は通常IPのまま（**警告**: 受信側IPは露出する）
3. **Given** `--tor-mode=forced`で起動した場合、**When** 非Torピアが接続を試みる、**Then** 接続が拒否される

---

### User Story 4 - ブートストラップノードへの接続 (Priority: P2)

新規ノード運営者として、ネットワークに初めて参加する際に、既知のブートストラップノードに接続してピア情報を取得したい。

**Why this priority**: ピア発見の起点となるブートストラップは、ネットワーク参加に必須。

**Independent Test**: 初期状態のノードがブートストラップノード（Onionアドレス）に接続し、その後追加のピアを発見できることを確認。

**Acceptance Scenarios**:

1. **Given** ピア情報を持たない新規ノードが、**When** ブートストラップノード（`.onion`アドレス）に接続する、**Then** ネットワークに参加しピアリストを取得できる
2. **Given** ブートストラップノードがダウンしている場合、**When** 複数のブートストラップが設定されている、**Then** 代替ノードに自動フォールバックする

---

### Edge Cases

- **Tor接続タイムアウト**: Torネットワークが不安定な場合、接続に通常より長い時間がかかる。適切なタイムアウト設定と再試行ロジックが必要
- **Torデーモン未起動**: torsocks使用時にTorが起動していない場合のエラーハンドリング
- **Onion Service証明書の期限切れ**: 長期運用時のキーローテーション
- **悪意あるブートストラップノード**: 不正なピア情報を返すノードへの対処（複数ソースからの検証）
- **ネットワーク分断**: Torノードと非Torノードが混在する過渡期の相互運用性
- **悪意ある出口ノード（Phase 1固有）**: torsocksで通常IPのピアに接続する場合、Tor出口ノードがGossipSubメッセージを盗聴・メタデータ収集する可能性がある。Substrateの署名により改ざんは困難だが、トラフィック分析リスクは残る。**対策**: ブートストラップノードを`.onion`アドレスのみで構成し、クリアネットに一度も出ない「Onion-to-Onion」通信を推奨。`--tor-mode=forced`との併用を強く推奨

## Requirements *(mandatory)*

### Functional Requirements

#### Phase 1: 外部プロキシ（torsocks）

- **FR-001**: ノードはtorsocks経由で起動した場合、すべての送信TCP接続がTor経由で行われること
- **FR-002**: torsocks起動時もブロック同期・トランザクション伝播が正常に動作すること
- **FR-003**: torsocks運用のセットアップ手順がドキュメント化されていること

#### Phase 2: Onion Service対応

- **FR-004**: ノードがOnion Service（Hidden Service）として受信接続を受け付けられること
- **FR-005**: ノードは自身の`.onion`アドレスをピアに広告できること。**実装**: ノード自体はOnion Serviceの存在を知らないため、`--public-addr /onion3/<address>:<port>`フラグで外部から見えるアドレスを手動指定する必要がある
- **FR-006**: 他ノードは`.onion`アドレスを指定してピア接続できること
- **FR-007**: Onion Service設定のセットアップ手順がドキュメント化されていること

#### ネットワーク設定

- **FR-008**: ノード起動時に`--tor-mode`オプションでTor使用モードを指定できること（off/outbound-only/forced）。`outbound-only`は受信IPが露出するリスクを明示する名称
- **FR-009**: `--tor-mode=forced`の場合、以下の2つのロックを強制すること:
  - **① 出口ロック（Outbound Enforcement）**: 環境変数`ANARCHY_RUNNING_UNDER_TORSOCKS`が未設定ならプロセスを即座に終了
  - **② 入口ロック（Inbound Enforcement）**: `listen_addresses`を`127.0.0.1:30333`に強制し、外部からの直接TCP接続を不可能にする（Onion Service経由のみ受信可能）
- **FR-010**: ブートストラップノードのアドレス（`.onion`含む）を設定ファイルで指定できること
- **FR-011**: 複数のブートストラップノードが設定可能で、フォールバック動作をすること

### Key Entities

- **ピア（Peer）**: ネットワーク上の他ノード。PeerIdで識別され、マルチアドレス（TCP/IP、Onionアドレス）で接続
- **ブートストラップノード**: 新規ノードが最初に接続する既知ノード。Onionアドレスで設定可能
- **Torモード**: ノードの匿名化レベルを示す設定（off/outbound-only/forced）。`outbound-only`は送信のみTor化で受信IPは露出するリスクがあることをユーザーに明示

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: torsocks経由で起動したノードが、10分以内にブロック同期を開始できる
- **SC-002**: Onion Service経由で、1時間あたり100件以上のトランザクションを中継できる
- **SC-003**: ネットワーク上の3台以上のノードがOnion Service経由で相互接続できる
- **SC-004**: `--tor-mode=forced`設定時に、listen_addressesが`127.0.0.1:30333`のみであること。外部IPへの直接接続が不可能であること
- **SC-005**: `--tor-mode=forced`でtorsocks環境変数が未設定の場合、ノードが起動せずエラー終了すること
- **SC-006**: ブートストラップノードが全て`.onion`アドレスの場合、ノード間通信が一度もクリアネットを経由しない（Onion-to-Onion通信）
- **SC-007**: ブートストラップノードへの初回接続から5分以内に、最低3ピアを発見できる

## Assumptions

- Torは各ノード運営者の環境に個別にインストールされている（Phase 1-2）
- ノード間のプロトコルはSubstrate標準のlibp2pを使用（TCP+Noise+Yamux）
- Arti 1.0安定版リリース（2026年後半予定）後にPhase 3（アプリ内蔵Tor）を再評価
- 初期段階ではテストネットのみで運用し、本番環境への適用は十分な検証後

## Scope Boundaries

### In Scope

- torsocksを使用した送信Tor化（Phase 1）
- Onion Service設定による受信Tor化（Phase 2）
- Tor使用モードの切り替え機能
- ブートストラップノード設定

### Out of Scope（将来検討）

- アプリケーション内蔵arti-client統合（Phase 3: arti 1.0安定後）
- sc-networkのフォークによるカスタムトランスポート
- I2P対応
- Mixnetプロトコル統合

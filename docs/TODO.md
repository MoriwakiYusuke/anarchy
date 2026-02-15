# Anarchy 実装TODO

## 前提条件

- **ブラウザ環境**: 通常HTTP/S（Torなし）
- **ノード間通信**: libp2p + Tor/I2P（Arti使用）
- **匿名性担保**: クライアント側Wasmで署名・ステルスアドレス生成

---

## Phase 1: セキュア・ファンデーション

### 1.1 プロジェクト基盤

- [x] モノレポ構成のセットアップ
  - [x] `pnpm-workspace.yaml` 作成
  - [x] `apps/blockchain/` - Substrate L1（バリデーター/フルノード）
  - [x] `apps/frontend/` - Next.js PWA（ハイドラUI）
  - [x] `apps/storage-node/` - データ保持専用ノード → **完了** (2026-02-10)
  - [ ] `packages/sdk/` - 共有暗号SDK
  - ~~[ ] `packages/wasm-engine/` - Rust→Wasm~~ → **変更**: フロントエンド内で完結（sharks crate直接使用）

- [ ] CI/CD パイプライン
  - [ ] Rust テスト・ビルド
  - [ ] TypeScript lint・テスト
  - [ ] Wasm ビルド自動化

### 1.2 Substrate L1 Core (`apps/blockchain/`)

- [x] Substrate ノードテンプレート初期化 (Polkadot SDK stable2503)

- [x] **Identity Pallet** 作成
  - ~~[x] WebAuthn公開鍵の登録ストレージ~~ → **廃止**: AccountIdのみ認証に変更（WebAuthn実装の複雑さとブラウザ互換性問題のため）
  - ~~[x] マルチデバイス対応（1 Identity → N Passkeys）~~ → **廃止**: シードフレーズからAccountId導出に一本化
  - ~~[x] 公開鍵の追加/削除エクストリンシック~~ → **廃止**: 不要

- [x] **Moral Token Pallet** 作成 → **ネイティブトークン化**: pallet_balancesで$moralを管理（2026-02-08）
  - ~~[x] トークン発行（mint）ロジック~~ → **廃止**: pallet_balancesのtransferを使用
  - [x] トークン焼却（burn）ロジック → `fungible::Mutate::burn_from`で実装
  - ~~[x] 残高管理ストレージ~~ → **廃止**: System.Account.data.freeで管理
  - ~~[x] 転送エクストリンシック~~ → **廃止**: Balances.transfer_allow_deathを使用
  - [x] Genesis設定でテストアカウントにMoral配布（10,000 MORAL/account）

- [x] **Post Pallet** 作成
  - [x] 投稿データ構造定義
  - [x] 投稿ストレージ（Posts, Contents, UserPosts）
  - [x] 投稿作成エクストリンシック
  - [x] 投稿コスト（$moral）の検証
  - [x] **動的コスト計算（byte数ベース）**
    - PostBaseCost = 10 MORAL（基本料金）
    - PostByteCost = 0.1 MORAL/byte（バイト単価）

### 1.3 libp2p + Tor 統合 → **完了** (2026-02-08)

> **設計方針**: Arti（Rust Tor）ではなく、システムTorデーモン + torsocksを使用。
> これにより、①外向き通信ロック（torsocks環境変数チェック）と②内向き通信ロック（127.0.0.1バインド）を実現。

- [x] libp2p ネットワーク層（Substrate標準sc-networkを使用）
  - [x] ノード識別（PeerId）
  - [x] Kademliaによるピア発見
  - [x] GossipSubによるメッセージ伝播

- ~~[ ] **Arti（Tor）統合**~~ → **放棄**: Artiはまだ実験的でno_std非対応、依存関係が複雑なため
  - ~~[ ] `arti-client` クレート導入~~
  - ~~[ ] Torトランスポートラッパー実装~~
  - ~~[ ] libp2p Transport として統合~~

- [x] **+ Tor統合（システムTor + torsocks方式）** ← Artiの代替として採用
  - [x] + `TorMode` enum実装 (`cli.rs`): `Off | OutboundOnly | Forced`
  - [x] + `--tor-mode` CLI引数 (`cli.rs`)
  - [x] + `apply_tor_mode()` ロジック (`command.rs`)
  - [x] + ①外向きロック: `ANARCHY_RUNNING_UNDER_TORSOCKS`環境変数チェック
  - [x] + ②内向きロック: 127.0.0.1バインド強制
  - [x] Onion Service対応 (`scripts/onion-service.sh`)

- [x] ネットワーク設定・セキュリティ
  - [x] Tor強制モード / 通常モード切替
  - ~~[ ] ブートストラップノード設定~~ → **延期**: Onion bootnode設定はテストネット公開時に実装
  - [x] + **mainnet自動強制**: chain_id に "mainnet" 含むと TorMode::Forced 強制
  - [x] + .onionアドレスサニタイズ（ログ漏洩防止）

- [x] + 運用スクリプト
  - [x] + `scripts/onion-service.sh` - Onion Service セットアップ
  - [x] + `scripts/tor-setup.sh` - torsocks実行スクリプト
  - [x] + `scripts/anarchy-tor.sh` - Torモードでのノード起動ラッパー

- [x] + テスト
  - [x] + ユニットテスト 17件 (`command.rs`)
  - [x] + 統合テスト 15件 (`tests/integration/tor_connectivity_test.sh`)

### ~~1.4 WebAuthn 署名検証~~ → **廃止** (2026-02-08)

> **廃止理由**: WebAuthn実装の複雑さ（COSE/CBOR解析、ブラウザ差異、authenticatorData検証）と、
> シードフレーズベースのAccountId認証でも十分なUXが実現できるため、シンプルな設計を優先。
> コードは`pallets/identity/src/`に残存するが未使用。

- ~~[x] **Rust署名検証ライブラリ** (`apps/blockchain/pallets/identity/src/`)~~
  - ~~[x] COSE公開鍵パーサー (`cose.rs`)~~
  - ~~[x] ES256 (P-256) 署名検証 (`webauthn.rs`)~~
  - ~~[x] authenticatorData パース (`webauthn.rs`)~~
  - ~~[x] clientDataJSON 検証 (`webauthn.rs`)~~

- ~~[x] **Substrate統合**~~
  - ~~[x] オンチェーンWebAuthn検証ロジック~~
  - ~~[x] WYSIWYS: challengeに投稿ハッシュ埋め込み~~
  - ~~[x] `create_post_with_webauthn` エクストリンシック（Post Pallet）~~

### 1.5 フロントエンド MVP (`apps/frontend/`)

- [x] Next.js プロジェクト初期化
  - [x] TypeScript設定
  - ~~[ ] PWA設定（next-pwa）~~ → **延期**: Light Client対応後に検討

- ~~[ ] WebAuthn統合~~ → **廃止**: AccountIdのみ認証に変更
  - ~~[ ] パスキー登録フロー~~
  - ~~[ ] パスキー認証フロー~~
  - ~~[ ] 署名リクエスト（投稿時）~~

- [x] 基本UI
  - [x] タイムライン表示
  - [x] 投稿フォーム（動的コスト表示付き）
  - [x] ウォレット残高表示
  - [x] PAPI (polkadot-api) によるチェーン接続
  - [x] Runtime constantsからのコスト設定取得（フォールバック対応済み）

---

## Phase 2: プライバシー・レイヤー

### 2.1 Wasm暗号エンジン

> **設計変更** (2026-02-10): `packages/wasm-engine/` は作成せず、フロントエンド内で完結。
> SSS/Merkle TreeはWeb Workerで実行し、ブラウザ側で暗号化を完結。

- [x] **シャミアの秘密分散 (SSS)** → **完了** (`apps/frontend/src/lib/`) (2026-02-10)
  - [x] + 分割（split）関数: `sharks` crate → Wasm → Web Worker
  - [x] + 復元（reconstruct）関数: 同上
  - [x] + しきい値設定: k=3, n=5 (システム固定値)
  - ~~[ ] Wasmエクスポート~~ → **変更**: フロントエンド内Wasm、package化せず

- [x] + **Merkle Tree** (`apps/frontend/src/lib/`) (2026-02-10)
  - [x] + ツリー構築（断片からルートハッシュ計算）
  - [x] + 断片ID導出: `hash(merkle_root || index)`
  - [x] + Web Worker で非同期実行

### 2.2 分散ストレージ（データ保持報酬）

> **設計方針**: バリデーター（計算と合意）とストレージノード（記憶の保持）は役割を明確に分離。
> 強力CPUはないが巨大HDD/SSDを持つユーザーも$moralを稼ぐ手段となる。

#### Phase 1: Storage MVP → **完了** (2026-02-10)

- [x] **Storage Pallet MVP** 作成（`apps/blockchain/pallets/storage/`）
  - [x] 断片メタデータストレージ（FragmentId, サイズ, 作成者）
  - [x] ストレージノード登録（PeerID, 容量）
  - [x] 保持表明（declare_holding）/ 取消（revoke_holding）
  - [x] ランタイム統合完了
  - [x] ベンチマーキング骨格

- [x] **ストレージノード・デーモン MVP** (`apps/storage-node/`)
  - [x] **libp2p P2P通信**: request-response プロトコル
  - [x] **ローカル断片ストレージ**: ファイルシステムベース + Blake2ハッシュ検証
  - [x] **ディスククォータ管理**: 設定可能な容量制限
  - [x] **設定ファイル (TOML)**: peer_id_path, data_dir, capacity, chain_url
  - [x] **graceful shutdown**: SIGINT/SIGTERM対応
  - [x] **メトリクス**: fragment_count, capacity_used_bytes など
  - [x] 25テストパス (22 unit + 3 integration)

#### Phase 1.5: Post Storage Migration → **完了** (2026-02-10)

> **目的**: 投稿コンテンツをチェーンから分散ストレージへ移行し、ストレージコスト削減 & 大容量対応

- [x] **Post Pallet改修** (`apps/blockchain/pallets/post/`)
  - ~~[ ] `Contents<T>` StorageMap廃止~~ → **変更**: V1(inline)/V2(distributed)両対応で後方互換性維持
  - [x] + `ContentV2` 構造体: `merkle_root` + `fragment_count` のみ保存
  - [x] + `create_post_v2` エクストリンシック（分散ストレージ用）
  - ~~[ ] 投稿コスト計算の変更~~ → **延期**: 現状はV1と同じバイト単価
  - [x] + V1/V2両対応（既存投稿との後方互換性）

- [x] **フロントエンド改修** (`apps/frontend/`)
  - [x] + 投稿作成: SSS分割 → Merkle Tree構築 → Storage Nodeへアップロード → `create_post_v2`
  - [x] + 投稿表示: `merkle_root` → `fragment_id`計算 → Storage Node取得 → SSS復元 → 表示
  - [x] + `useStorage` hook: SSS/Merkle Tree操作のカプセル化
  - [x] + PAPI Binary型対応（`asBytes()`メソッド検出）
  - [x] + i18n完全対応（ja/en/zh）
  - ~~[ ] キャッシュ戦略~~ → **延期**: まずは動作優先

- [x] **Storage Node拡張** (`apps/storage-node/`)
  - ~~[ ] **subxtチェーン接続**~~ → **変更**: JSON-RPC経由でブロックチェーンノードに登録
  - [x] + HTTP JSON-RPC API: `upload_fragment`, `get_fragment`
  - [x] + 自動登録: 起動時にブロックチェーンノードへ`storage_registerEndpoint`
  - [x] + 30秒heartbeat: ブロックチェーン再起動時の自動再登録
  - [x] + 共有URL状態: `Arc<RwLock<Option<String>>>` で全RPC接続で共有

#### Phase 2: Multi-Node Storage → **完了** (2026-02-14)

> **実装内容**: 010-multi-node-storage仕様に基づくマルチノード対応、セキュリティ強化、P2P通信

- [x] + **マルチノード対応** (断片の複数ノード分散配置)
  - [x] + SharedStorageNodes: 複数ストレージノード管理
  - [x] + fragment-indexベース分散: 各断片を異なるノードに配置
  - [x] + フェイルオーバー取得: 取得失敗時に他ノードへフォールバック

- [x] + **ノード選択方式**: ランダム固定（プライバシー優先）
  - [x] + ランダム選択: プライバシーと負荷分散
  - [x] + オフラインノードフィルタリング

- [x] + **ストレージノードP2P通信** (libp2p Gossipsub)
  - [x] + トピック: `/anarchy/endpoints/1.0.0`
  - [x] + Ed25519署名付きメッセージ
  - [x] + ブロックチェーンエンドポイント共有
  - [x] + レピュテーションシステム (スコア: +1有効/-20無効)
  - [x] + TTLベースエンドポイントキャッシュ + GC

- [x] + **アクティブ-スタンバイフェイルオーバー**
  - [x] + 2秒間隔liveness check
  - [x] + 3回連続失敗でフェイルオーバー発動
  - [x] + Hot Standby事前接続

- [x] + **ストレージノードアクセス認証** (署名検証)
  - [x] + SignedRequest: account_id, timestamp, nonce, signature
  - [x] + Sr25519署名検証
  - [x] + 5分タイムスタンプ有効期限
  - [x] + ナンスキャッシュ (リプレイ攻撃防止)
  - [x] + upload_fragmentに認証必須、get_fragmentは公開維持

- [x] + **Storage Palletセキュリティ強化**
  - [x] + Blake2b PoW検証 (動的難易度: 12 + recent_registrations/5)
  - [x] + レート制限: 5登録/ブロック、10宣言/ブロック/ノード
  - [x] + PeerID検証 (38-64 bytes)
  - [x] + 最小容量検証 (1GB)
  - [x] + Post-Storage Pallet密結合 (do_register_fragment)

- [x] + **チェーン間Storage Node情報共有** (Gossipプロトコル)
  - [x] + トピック: `/anarchy/storage-nodes/1`
  - [x] + オンチェーンhttp_url保存 + Runtime API
  - [x] + リアルタイム共有 (新ノード登録時に即座に伝播)

- [x] + **Observability** (NFR完了)
  - [x] + JSON構造化ログ (ANARCHY_LOG_JSON環境変数)
  - [x] + Prometheusメトリクス (/metrics エンドポイント)
  - [x] + fragment_upload_total, fragment_download_total
  - [x] + storage_node_peers, blockchain_node_failover_total

#### Phase 3: KZG Proof & Rewards → **完了** (2026-02-16)

> **実装内容**: 011-kzg-proof-rewards仕様に基づくKZG証明・報酬システム

- [x] + **Storage Pallet拡張**
  - ~~[ ] 保持証明（Proof of Spacetime）検証ロジック~~ → [x] + KZG多項式コミットメント検証 (BLS12-381)
  - [x] + **保持報酬ロジック**
    - [x] + `register_fragment_kzg`: KZG commitment登録 + deposit
    - [x] + `prove_holding_kzg`: KZG proof検証 + 報酬請求
    - [x] + RewardPool: 投稿費用の90%をプールへ、10% burn
    - [x] + 報酬分配: holder数で均等分配 / ScoreProviderベース
    - [x] + 報酬停止による「自然な忘却」メカニズム
  - [x] + **GCライフサイクル**
    - [x] + FragmentState: StateProposed → Active → ForgettingCandidate
    - [x] + `initiate_forgetting`: 明示的忘却開始
    - [x] + `on_finalize` GC: ForgettingCandidateの自動削除
  - ~~[ ] 不正ノードのスラッシング~~ → Phase 4へ延期

- [x] + **wasm-engine拡張** (`packages/wasm-engine/`)
  - [x] + KZG-VSSハイブリッド暗号化 (`hybrid.rs`)
  - [x] + `hybrid_split()`: AES-256-GCM + Reed-Solomon + SSS鍵分割
  - [x] + `hybrid_reconstruct()`: 断片からの復元
  - [x] + `generate_kzg_proof()` / `verify_kzg_proof()`: BLS12-381証明
  - [x] + MerkleTree構築 (Blake2b-256)

- [x] + **フロントエンド統合**
  - [x] + useStorage hook: KZGフロー対応
  - [x] + Reed-Solomon k-of-nエンコード/デコード
  - [x] + HybridShard構造: chunk + key_share + chunk_hash

#### Phase 4: Slashing & Repair (未実装)

- [ ] **自己修復プロトコル**
  > ストレージノードがオフライン時、自動的に断片を再配布
  - [ ] 健全性モニタリング（k-of-nのうちm個以下で警告）
  - [ ] 新規ストレージノードへの自動再分散
  - [ ] インセンティブ設計（再分散協力者に報酬）

### 2.3 PoW Faucet（アカウント初期化） → **完了** (2026-02-09)

- [x] **Faucet Pallet** 作成 (`pallets/faucet/`)
  - [x] PoWチャレンジ生成（ブロックハッシュベース）
  - [x] nonce検証（難易度調整可能: 18〜28ビット）
  - [x] 報酬: 初期$moral（100 MORAL）の付与
  - [x] レート制限（1アカウント1回のみ: `FaucetClaims`ストレージ）
  - [x] + **Unsigned Transaction対応**: 残高ゼロでも請求可能（`ValidateUnsigned`実装）

- [x] **フロントエンド統合** (`apps/frontend/`)
  - [x] Web Worker でのPoW計算 (`lib/faucet/worker.ts`)
  - [x] 計算進捗表示UI (`components/FaucetButton.tsx`)
  - [x] 自動アカウント初期化フロー (`hooks/useFaucet.ts`)
  - [x] + クリックデバウンス保護（連打防止）

- [x] **設計ポイント**
  - ~~[ ] 難易度: 数秒〜数十秒で解ける程度（ボット対策）~~ → [x] 難易度: 数秒〜数分（base=18, max=28, 動的調整）
  - ~~[ ] アルゴリズム: SHA256 or Blake2b（ASIC耐性不要）~~ → [x] アルゴリズム: Blake2b-256（Substrate標準、SHA256は不採用）
  - ~~[ ] 匿名性: KYC不要、IPログなし~~ → [x] 匿名性: KYC不要、署名不要（unsigned tx）← IPログはノード実装依存のため技術保証できる「署名不要」に変更

### 2.5 smoldot Light Client統合

> **設計方針**: Tor統合を断念したため、smoldotによるLight Client接続でRPC依存を排除し検閲耐性を確保。
> ブラウザ内でブロックを自分で検証するTrustlessな構成を実現。

- [ ] **smoldot導入** (`apps/frontend/`)
  - [ ] `polkadot-api/smoldot` パッケージ追加
  - [ ] `getWsProvider` → `getSmProvider` への切り替え
  - [ ] Web Worker でのsmoldot実行

- [ ] **チェーンスペック生成・配布**
  - [ ] ジェネシス情報を含むchain spec生成
  - [ ] ブートノードリスト設定
  - [ ] フロントエンドへのchain spec同梱

- [ ] **接続フォールバック戦略**
  - [ ] Light Client優先 → WsProvider フォールバック
  - [ ] 接続状態インジケーターUI

---

## Phase 3: 自律エコシステム

### 3.1 ステルスアドレス統合

- [ ] **クライアント側暗号実装** (`apps/frontend/`)
  - [ ] X25519鍵交換
  - [ ] ワンタイムアドレス導出
  - [ ] スキャン鍵/閲覧鍵ペア生成
  - [ ] Wasm実装 + Web Worker

- [ ] **Stealth Pallet** 作成
  - [ ] ステルスアドレス宛トランザクション
  - [ ] エフェメラル公開鍵の格納

- [ ] クライアント側スキャナー
  - [ ] バックグラウンドスキャン処理
  - [ ] 自分宛トランザクション検出
  - [ ] 復号・残高更新

### 3.2 反応マイニング

- [ ] **Reaction Pallet** 作成
  - [ ] 反応データ構造（いいね、ブースト等）
  - [ ] 反応ストレージ（PostReactions, UserReactions）
  - [ ] `react` エクストリンシック
  - [ ] 二重反応防止チェック
  - [ ] 投稿者への報酬付与
  - [ ] PoW難易度検証
  - [ ] 報酬計算: `Reward = Σ(Reaction × Power_cpu) × γ`
  - [ ] γ（インフレ調整係数）の動的計算
  - [ ] ステルスアドレス報酬先対応（名寄せ防止）

- [ ] クライアント側PoW
  - [ ] WebWorkerでのマイニング実行
  - [ ] Page Visibility API制御（フォアグラウンド強制）
  - [ ] 難易度調整パラメータ取得
  - [ ] マイニング報酬先の正当性検証

- [ ] 動的難易度調整
  - [ ] ネットワーク全体の反応レート監視
  - [ ] 難易度自動調整アルゴリズム
  - [ ] インフレ/デフレ抑制メカニズム

### 3.3 DM機能（Stealth Messaging）

- [ ] E2EE実装
  - [ ] ChaCha20-Poly1305暗号化
  - [ ] 鍵導出（HKDF）
  - [ ] メッセージパディング（固定サイズ化）

- [ ] **Messaging Pallet**
  - [ ] ステルスアドレス宛メッセージ格納
  - [ ] トラフィックパディング（ダミーメッセージ）

- [ ] クライアント側
  - [ ] メッセージスキャナー
  - [ ] 復号・表示UI
  - [ ] 送信フロー

---

## Phase 4: 本番デプロイ

### ~~4.1 Light Client 対応~~ → **Phase 2.5へ移動** (2026-02-11)

> Tor統合断念に伴い、smoldot導入を前倒し。詳細は Phase 2.5 を参照。

### 4.2 ハイドラ（フロントエンド業者）支援

- [ ] **RPCノード運用ガイド**
  - [ ] 公開RPCノード構成ドキュメント
  - [ ] ロードバランサー設定例
  - [ ] レート制限・セキュリティ設定

- [ ] **分散性の確保**
  - [ ] ハイドラ業者向けノード運用ドキュメント
  - [ ] コミュニティノード参加インセンティブ設計

### 4.3 テストネット/メインネット

- [ ] **テストネット公開**
  - [ ] パブリックブートノード設置
  - [x] Faucet（テスト用$moral配布）→ `pallet-faucet` で実装済み
  - [ ] Explorer統合

- [ ] **メインネット準備**
  - [ ] セキュリティ監査
  - [ ] Genesis設定最終化
  - [ ] バリデーター招集

### 4.4 Mainnet設計・経済パラメータ

- [ ] **経済合理性に基づく定数制定**
  - [ ] PostBaseCost / PostByteCost の最適値検証
  - [ ] Faucet報酬額・難易度の調整
  - [ ] ストレージ報酬レート設計
  - [ ] インフレ/デフレ率シミュレーション

- [ ] **トークノミクス最終設計**
  - [ ] 初期供給量・分配比率
  - [ ] バリデーター報酬設計
  - [ ] ストレージノード報酬設計
  - [ ] 反応マイニング報酬曲線

- [ ] **ガバナンスパラメータ**
  - [ ] 投票期間・クォーラム閾値
  - [ ] パラメータ変更プロセス

---

## 構想事項（検討中）

> **別ドキュメントに移動**: [CONCEPTS.md](CONCEPTS.md) を参照
>
> - 経済設計（トークノミクス）
> - コンセンサス方式の検討（PoA → PoW）
> - ブラウザ拡張ウォレット連携
> - オンチェーンガバナンス
> - 残高保護機能（Keep Alive強制）
> - ZKP匿名人間証明（Circom/Noir回路、Groth16/PLONK検証）

---

## 分散ストレージ実装順序 (2026-02-09決定)

> **設計方針**: SSSを待たずにストレージ基盤を先に構築。Phase 1は「繋がるだけ」のMVP。

| 順番 | 項目 | 内容 | 仕様書 | 状態 |
|-----|------|------|--------|------|
| **1** | 008-distributed-storage **Phase 1** | Storage Registry & P2P | [spec.md](../specs/008-distributed-storage/spec.md) | ✅完了 |
| **2** | SSS (Phase 2.1) | クライアント側暗号化・断片化 | - | ✅完了 |
| **3** | + **Post Storage Migration** | 投稿コンテンツの分散ストレージ移行 | - | ✅完了 |
| **4** | + **010-multi-node-storage** | マルチノード対応 & セキュリティ強化 | [spec.md](../specs/010-multi-node-storage/spec.md) | ✅完了 (2026-02-14) |
| **5** | + **011-kzg-proof-rewards** | KZG証明 & 報酬システム | [spec.md](../specs/011-kzg-proof-rewards/spec.md) | ✅完了 (2026-02-16) |
| **6** | 008-distributed-storage **Phase 4** | Slashing & Repair | - | 未着手 |

### Phase 1 スコープ（まず繋がるだけ） → ✅完了 (2026-02-10)

- ✅ Storage Pallet: `register_fragment`, `register_node`, `declare_holding`
- ✅ Storage Daemon: libp2p断片送受信、ディスク保存
- ✅ + HTTP JSON-RPC API: フロントエンド連携
- ✅ + 自動登録 + heartbeat: ブロックチェーンノードへの登録
- ❌ ~~PoST~~ → Phase 3 (KZG)
- ❌ ~~報酬~~ → Phase 3 (KZG)
- ❌ ~~スラッシング~~ → Phase 4
- ❌ ~~自己修復~~ → Phase 4

### + Phase 2 スコープ（010-multi-node-storage） → ✅完了 (2026-02-14)

- ✅ + マルチノード対応: SharedStorageNodes、fragment-index分散
- ✅ + ノード選択方式: ランダム固定（プライバシー優先）
- ✅ + Storage Node P2P: Gossipsub (`/anarchy/endpoints/1.0.0`)
- ✅ + アクセス認証: Sr25519署名検証、nonce replay防止
- ✅ + Storage Palletセキュリティ: PoW + レート制限
- ✅ + Post-Storage密結合: do_register_fragment
- ✅ + チェーン間Gossip: `/anarchy/storage-nodes/1`
- ✅ + Observability: 構造化ログ、Prometheusメトリクス

---

## 技術的依存関係

```
Phase 1.2 (Substrate) ✅ ──┬── Phase 1.3 (libp2p+Tor) ✅
                           │
                           └── Phase 1.5 (Frontend) ✅

Phase 2.1 (SSS/Wasm) ✅ ──── Phase 2.2 (Storage) ✅ ─┬─ + Phase 1.5 (Post Storage) ✅
                          │                          │
                          └── Phase 2.3 (PoW Faucet) ✅ 
                                                     │
                                                     ├─ + 010-multi-node-storage ✅ (2026-02-14)
                                                     │
                                                     └─ + 011-kzg-proof-rewards ✅ (2026-02-16)

Phase 3.1 (Stealth) ─── Phase 3.2 (Reaction) ─── Phase 3.3 (DM)

Phase 1-3 完了後 ────────── Phase 4 (本番デプロイ)
```

---

## 優先度マトリクス

| タスク | 重要度 | 難易度 | 優先順位 | 状態 |
|--------|--------|--------|----------|------|
| Substrate基盤 | 高 | 中 | **1** | ✅完了 |
| Moral Pallet | 高 | 低 | **1.5** | ✅完了 |
| Post Pallet | 高 | 中 | **1.5** | ✅完了 |
| フロントMVP | 高 | 低 | **2** | ✅完了 |
| ~~Identity Pallet~~ | ~~高~~ | ~~中~~ | ~~**3**~~ | ⚠️廃止 |
| ~~WebAuthn検証~~ | ~~高~~ | ~~中~~ | ~~**4**~~ | ⚠️廃止 |
| libp2p基盤 | 高 | 低 | **5** | ✅完了 |
| ~~Arti(Tor)統合~~ | ~~中~~ | ~~高~~ | ~~6~~ | ✅完了（torsocks方式） |
| SSS実装 | 中 | 低 | 7 | ✅完了 |
| **Storage Pallet** | 高 | 中 | **8** | ✅完了 |
| **ストレージノード** | 高 | 高 | **9** | ✅完了 |
| + **Post Storage統合** | 高 | 中 | **10** | ✅完了 |
| + **マルチノード対応** | 高 | 高 | **11** | ✅完了 (2026-02-14) |
| + **KZG Proof & Rewards** | 高 | 高 | **12** | ✅完了 (2026-02-16) |
| ステルスアドレス | 中 | 中 | 13 | 未着手 |
| 反応マイニング | 低 | 中 | 14 | 未着手 |
| ~~ZKP回路~~ | ~~低~~ | ~~高~~ | ~~13~~ | →構想移動 |

---

## マイルストーン

### M1: 動作するローカルネット ✅完了
- Substrateノード起動
- 基本的なトークン転送（Moral）
- シンプルな投稿機能
- **追加達成**: 動的投稿コスト、Genesis設定

### ~~M2: 認証統合~~ → **設計変更** (2026-02-08)

> **変更内容**: WebAuthn認証を廃止し、シードフレーズベースのAccountId認証に一本化。
> **理由**: WebAuthn実装の複雑さ（ブラウザ互換性、COSE解析、no_std制約）に対し、
> シードフレーズ方式でも「秘密鍵をユーザーに見せない」UXは実現可能。
> polkadot.jsやNovaウォレット等の既存エコシステムとも互換性を維持。

- ~~[x] WebAuthn署名検証（COSE/P-256/WYSIWYS）~~ → 廃止
- ~~[x] Identity Pallet（Passkey登録/管理）~~ → 廃止
- ~~[x] Post Pallet WebAuthn統合~~ → 廃止
- ~~[ ] フロントエンドでのパスキー登録/認証~~ → 廃止
- [x] **代替実装**: AccountIdのみ認証（シードフレーズから導出）
- [x] **代替実装**: `polkadot-api`のsigner連携で投稿署名

### M3: P2Pネットワーク ✅完了 (2026-02-08)
- [x] 複数ノード間の通信確立 (`run-multi-node.sh`, 最大10ノード対応)
- [x] GossipSubによる投稿伝播 (Substrate `sc-network` 標準機能)

### M4: Tor統合 ✅完了 (2026-02-08)

> **変更**: Artiではなくシステムtor + torsocks方式を採用

- [x] 匿名通信 (`--tor-mode=forced`, Onion Service)
- [x] 統合テスト18件パス (`tests/integration/tor_connectivity_test.sh`)

### M5: プライバシー機能（4週間）
- ~~SSS断片化~~ → ✅完了 (2026-02-10)
- ステルスアドレス

### M6: 分散ストレージ ✅完了 (2026-02-10)

> **実装内容**: 投稿コンテンツの分散ストレージ移行。オンチェーンはmerkle_rootのみ保存。

- [x] Storage Pallet MVP
- [x] Storage Node MVP + HTTP JSON-RPC API
- [x] SSS分割/復元 (sharks crate, Wasm, Web Worker)
- [x] Merkle Tree構築/検証
- [x] Post Pallet V2対応 (ContentV2: merkle_root + fragment_count)
- [x] フロントエンド統合 (useStorage hook, PAPI Binary対応)
- [x] 自動登録 + 30秒heartbeat

### + M7: マルチノード対応 ✅完了 (2026-02-14)

> **実装内容**: 010-multi-node-storage仕様に基づくマルチノード分散、セキュリティ強化、P2P通信

- [x] + **断片マルチノード分散配置**
  - [x] + SharedStorageNodes: 複数ノード管理
  - [x] + fragment-index分散: 各断片を異なるノードに配置
  - [x] + フォールバック取得: 失敗時に他ノードへフォールバック

- [x] + **ノード選択方式**: ランダム固定（プライバシー優先）

- [x] + **ストレージノード間P2P通信** (libp2p Gossipsub)
  - [x] + トピック: `/anarchy/endpoints/1.0.0`
  - [x] + Ed25519署名付きメッセージ
  - [x] + レピュテーションシステム
  - [x] + Active-Standbyフェイルオーバー

- [x] + **アクセス認証**
  - [x] + Sr25519署名検証
  - [x] + タイムスタンプ有効期限 (5分)
  - [x] + ナンスによるリプレイ攻撃防止

- [x] + **Storage Palletセキュリティ強化**
  - [x] + Blake2b PoW検証 (動的難易度)
  - [x] + レート制限: 5登録/ブロック、10宣言/ブロック/ノード
  - [x] + Post-Storage密結合 (do_register_fragment)

- [x] + **チェーン間Storage Node情報共有**
  - [x] + Gossipプロトコル: `/anarchy/storage-nodes/1`
  - [x] + オンチェーンhttp_url保存
  - [x] + Runtime API: `get_all_storage_nodes()`

- [x] + **Observability**
  - [x] + JSON構造化ログ
  - [x] + Prometheusメトリクス (/metrics)

### + M8: KZG Proof & Rewards ✅完了 (2026-02-16)

> **実装内容**: 011-kzg-proof-rewards仕様に基づくKZG証明・報酬システム

- [x] + **wasm-engine KZG-VSSハイブリッド暗号化**
  - [x] + `hybrid_split()` / `hybrid_reconstruct()`
  - [x] + AES-256-GCM + Reed-Solomon + SSS鍵分割
  - [x] + KZG commitment / proof生成 (BLS12-381)

- [x] + **pallet-storage KZG報酬システム**
  - [x] + `register_fragment_kzg`: commitment + deposit登録
  - [x] + `prove_holding_kzg`: KZG proof検証 + 報酬請求
  - [x] + RewardPool: 投稿費用90% → プール、10% burn
  - [x] + holder数ベース均等分配 / ScoreProvider対応

- [x] + **GCライフサイクル**
  - [x] + FragmentState: StateProposed → Active → ForgettingCandidate
  - [x] + `initiate_forgetting`: 明示的忘却開始
  - [x] + `on_finalize` GC: 自動削除

- [x] + **フロントエンド統合**
  - [x] + HybridShard構造対応
  - [x] + Reed-Solomon復元ロジック

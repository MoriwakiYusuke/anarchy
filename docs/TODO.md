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
  - ~~[ ] + GCライフサイクル~~ → **不要**: BTCと同様、オンチェーンメタデータは永続（消去機能は検閲耐性に反する）
  - ~~[ ] 不正ノードのスラッシング~~ → Phase 4へ延期

- [x] + **Storage Node KZG統合**
  - [x] + `challenge.rs`: チャレンジ監視ロジック
  - [x] + `prover.rs`: KZG証明生成 (`load_srs_from_file()`, `generate_proof()`)
  - [x] + 証明の自動提出 (`ChainClient::submit_holding_proof()`)

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

#### Phase 4: Slashing & Repair → **完了** (2026-02-24)

> **実装内容**: 013-slashing-repair仕様に基づく自己修復プロトコル

- [x] + **スラッシングシステム** (pallet-storage)
  - [x] + ProofRecord拡張: `slashed: bool`, `share_index: u8`
  - [x] + `do_slash_node()`: チャレンジ3回失敗でスラッシュ発動
  - [x] + 担保50%没収 → RepairRewardPool へ

- [x] + **FragmentState管理**
  - [x] + FragmentStateKind enum: Active/AtRisk/Repairing/Lost
  - [x] + `update_fragment_state()`: 保持者数に応じた状態遷移
  - [x] + Runtime API: `get_at_risk_fragments()`, `get_fragment_state()`

- [x] + **自己修復プロトコル**
  > ストレージノードがオフライン時、自動的に断片を再配布
  - [x] + 健全性モニタリング（k=3未満で AtRisk 状態へ遷移）
  - [x] + `regenerate_share()`: Lagrange補間でシェア再生成 (wasm-engine)
  - [x] + repair coordinator/scheduler (storage-node)
  - [x] + `confirm_repair` extrinsic: 修復完了確認

- [x] + **余剰ホルダー排除 (Stale Holder GC)**
  - [x] + `evict_stale_holder` extrinsic: 最低優先度ホルダー排除
  - [x] + `compute_eviction_candidates()`: 優先度スコア計算
  - [x] + StaleHolderGc (storage-node): 自動GCサイクル

- [x] + **インセンティブ設計**
  - [x] + 修復報酬: スラッシュプールから修復協力者へ分配
  - [x] + MinWithdrawalAmount: 500 MORAL (引き出し下限)

- [x] + **RPC/監視エンドポイント**
  - [x] + `storage_getAtRiskFragments`
  - [x] + `storage_getFragmentState`
  - [x] + `storage_getEvictionCandidates`
  - [x] + `storage_getFragmentsWithExcessHolders`
  - [x] + `storage_repairStatus` (storage-node)

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

### ~~2.5 smoldot Light Client統合~~ → **Phase B PR #53 で撤去** (2026-05-NN)

> **当初設計**: Tor統合を断念したため、smoldotによるLight Client接続でRPC依存を排除し検閲耐性を確保する想定だった。
>
> **撤去理由** (Phase B mainnet smoke で判明):
> - smoldot は内部 consensus enum に Babe/Aura/AllAuthorized しか持たず、PoW chain の block announcement を decode できない (`BadBlockAnnounce(DecodeBlockAnnounceError)` を実機 E2E で確認)
> - そもそも Anarchy の post / DM / storage は `storage_uploadFragment` 等の chain-node RPC 拡張に依存しており、smoldot からは呼べない構造だった (frontend は WS 経路を併用していた)
> - → smoldot を残しても anonymity 上の利得ゼロ + bundle ~MB の重さ + PoW 非互換 → 撤去して WebSocket (getWsProvider) に統一
>
> **代替**: chain-node を Tor hidden service として公開し `wss://<onion>:9944` で接続する運用で anonymity 担保 (docs/Tor.md)

- [x] ~~smoldot導入~~ → **撤去** (Phase B):
  - [-] ~~`smoldot` パッケージ削除~~ (`package.json`)
  - [-] ~~シングルトン管理~~ (`lib/smoldot-provider.ts` 削除)
  - [-] ~~`useSmoldot` フック~~ (`hooks/useSmoldot.ts` 削除)
  - [+] **`getWsProvider` ベースの `lib/chain-client.ts`** 新設 (Phase B)
  - [+] **`useChain` フック** (`hooks/useChain.ts`) 新設 (Phase B)
  - [+] 接続状態型定義 (`types/connection.ts`) — comment を WS 文脈に更新

- [x] ~~チェーンスペック生成・配布~~ → **撤去** (Phase B、WS は chainspec 不要):
  - [-] ~~chain spec生成スクリプト~~ (`apps/frontend/scripts/update-chainspec.sh` 削除)
  - [-] ~~ブートノードリスト設定~~
  - [-] ~~フロントエンドへの chain spec 同梱~~ (`lib/chainspec.json` 削除)
  - [+] `NEXT_PUBLIC_CHAIN_RPC_URL` で接続先 override (default: `ws://127.0.0.1:9944`)

- [x] ~~接続フロー~~ → WS 移行で簡素化:
  - [+] 同期タイムアウト 60s → 30s に短縮 (WS は smoldot より速い)
  - [+] ブロック番号自動更新 6s → 10s (PoW 30s blocktime に合わせ)
  - [+] 接続状態表示 (initializing/syncing/connected/error) — useChain で同等の状態管理

- [x] **Faucet改善**
  - [x] + RPCタイムアウト追加（30秒）- ネットワークエラー時のハング防止
  - [x] + AlreadyClaimed事前検証 - 2回目Claim即時検出
  - [x] + 送信後ストレージ検証 - smoldot非同期バリデーション対応
  - [x] + エラーボタンhover色修正（緑→赤）

- [x] **テスト**
  - [x] + useSmoldot: 15件
  - [x] + smoldot-provider: 12件  
  - [x] + connection types: 7件
  - [x] + useFaucet拡張: 4件追加（計16件）

---

### 2.6 フロントエンド拡充 → **完了** (2026-02-27)

> **実装内容**: 015-frontend-expand仕様に基づく送金・メディア添付・ニックネーム機能

- [x] + **送金フォーム** (`apps/frontend/`) ← ~~送金モーダル~~（常時表示に変更）
  - ~~[ ] 送金モーダルコンポーネント (`components/TransferModal.tsx`)~~ → [x] + `components/TransferForm.tsx` (275行)
  - [x] + 宛先AccountId入力フィールド（SS58チェックサム検証付き）
  - [x] + 送金金額入力（MORAL単位、残高上限チェック）
  - ~~[ ] `Balances.transfer_allow_death` 呼び出し~~ → [x] + `Balances.transfer_keep_alive`（PAPI経由）
  - [x] + 確認ダイアログ（宛先・金額を再確認）
  - [x] + 自己送金防止バリデーション
  - [x] + エラーハンドリング（残高不足、無効アドレス等）
  - [x] + 成功時の残高更新・トースト通知
  - [x] + `useTransfer` hook (332行): 送金ロジックカプセル化
  - [x] + `lib/addressValidation.ts` (229行): SS58アドレス検証

- [x] + **メディア添付対応** (`apps/frontend/`)
  > 動画・画像は既存分散ストレージインフラを活用（KZG-VSS暗号化 → Storage Node保存）
  - [x] + 画像アップロードUI（ドラッグ&ドロップ、ファイル選択）: `components/MediaUpload/` (544行)
  - [x] + 画像プレビュー表示: `MediaPreview.tsx` (183行)
  - [x] + 動画アップロードUI（ファイル選択）
  - [x] + 動画プレビュー/サムネイル自動生成: `lib/videoThumbnail.ts` (151行)
  - ~~[ ] ファイルサイズ制限（画像: 100MB、動画: 1000MB）~~ → [x] + ファイルサイズ制限（全メディア: 256MB、最大4ファイル）
  - ~~[ ] 対応フォーマット検証（画像: JPEG/PNG/GIF/WebP、動画: MP4/WebM）~~ → [x] + 全ファイルタイプ受付（`accept="*/*"`）、タイプ自動検出
  - ~~[ ] Post V2拡張: `media_merkle_roots: Vec<[u8; 32]>` 追加~~ → [x] + `postCodec.ts` (320行): テキスト+メディアのバイナリエンコード
  - [x] + タイムライン表示: メディア復元・インライン表示
  - [x] + `components/MediaDisplay/` (205行): 投稿内メディア表示
  - [x] + `components/VideoPlayer/` (175行): 動画再生プレイヤー
  - [x] + `components/ImageModal.tsx` (71行): フルスクリーン画像表示
  - [x] + `components/Lightbox/` (191行): 画像ギャラリー
  - [x] + `useMediaUpload` hook (435行): ファイル検証・アップロード・状態管理
  - [x] + `lib/mediaProcessor.ts` (231行): EXIF自動除去
  - [x] + `lib/mediaValidator.ts` (139行): ファイルタイプ・サイズ検証
  - [x] + 進捗表示（%）: `ProgressBar.tsx` (39行)
  - [x] + エラーリカバリーUI: `ErrorRecovery.tsx` (109行)

- [x] + **投稿者名表示**
  - ~~[ ] 投稿者AccountIdの短縮表示（先頭6文字...末尾4文字）~~ → [x] + 短縮表示（先頭8文字...末尾6文字）
  - [x] + クリックで全AccountIdコピー: `lib/clipboard.ts` (84行)
  - ~~[ ] Identity Palletとの連携?~~ → [x] + **Nickname Pallet新規作成**（下記参照）
  - [x] + `components/AddressDisplay/` (187行): 短縮表示+ツールチップ+コピー
  - [x] + ホバーツールチップでフルAccountId表示

- [x] + **Nickname Pallet** (`apps/blockchain/pallets/nickname/`) ← **新規追加**
  > 軽量オンチェーンニックネーム登録（ユニーク制約なし、自称OK）
  - [x] + `set_nickname` extrinsic: ニックネーム登録/更新
  - [x] + `clear_nickname` extrinsic: ニックネーム削除
  - [x] + ストレージ: `Nicknames: AccountId -> BoundedVec<u8, 32>`
  - [x] + 手数料: 登録・変更時に10 MORAL消費
  - [x] + 制約: 1-32 UTF-8文字
  - [x] + ランタイム統合 (`runtime/src/lib.rs`)
  - [x] + ユニットテスト 366行
  - [x] + `components/NicknameSettings/` (206行): 折りたたみ式設定UI
  - [x] + `useNickname` hook (249行): ニックネームCRUD操作

- [x] + **UI/UX改善**
  - [x] + `components/Skeleton/` (94行): ローディングスケルトン
  - [x] + `components/PostSkeleton/` (58行): 投稿読み込み中表示
  - [x] + `components/MediaPlaceholder/` (94行): メディア読み込みプレースホルダー
  - [x] + `components/Icons.tsx` (138行): SVGアイコン集
  - [x] + i18n翻訳追加 (ja/en/zh): 送金・メディア・ニックネーム関連 (+70キー)

- [x] + **テスト** (391 tests passed)
  - [x] + `TransferForm.test.tsx` (358行)
  - [x] + `MediaUpload.test.tsx` (518行)
  - [x] + `NicknameSettings.test.tsx` (363行)
  - [x] + `AddressDisplay.test.tsx` (324行)
  - [x] + `VideoPlayer.test.tsx` (150行)
  - [x] + `useTransfer.test.ts` (358行)
  - [x] + `useMediaUpload.test.ts` (632行)
  - [x] + `useMediaUpload.video.test.ts` (390行)
  - [x] + `useNickname.test.ts` (471行)
  - [x] + `postCodec.test.ts` (442行)

- [x] **いいね/bad/ギフト** → **Phase 3.2（反応マイニング）で実装**
  > オンチェーンスコア反映はReaction Palletと同時に実装。詳細は Phase 3.2 を参照。


## Phase 3: 自律エコシステム

### 3.1 ステルスアドレス統合 ✅

- [x] **クライアント側暗号実装** (`packages/wasm-engine/src/stealth/`)
  - [x] X25519鍵交換 (`keys.rs`)
  - [x] ワンタイムアドレス導出 (`address.rs`)
  - [x] スキャン鍵/閲覧鍵ペア生成 (`keys.rs`)
  - [x] Wasm実装 + Web Worker (`worker.ts`)
  - [x] + バックアップ暗号化/復号 (`backup.rs`)

- [x] **Stealth Pallet** 作成 (`apps/blockchain/pallets/stealth/`)
  - [x] ステルスアドレス宛トランザクション (`send_to_stealth`)
  - [x] エフェメラル公開鍵の格納 (`EphemeralKeys` StorageMap)

- [x] クライアント側スキャナー (`apps/frontend/src/lib/stealth/`)
  - [x] バックグラウンドスキャン処理 (`scanner.ts`)
  - [x] 自分宛トランザクション検出 (`scan_transaction`)
  - [x] 復号・残高更新 (`balanceStore.ts`)
  - [x] + コインセレクション (`coinSelection.ts`)
  - [x] + ステルス署名 (`signer.ts`)

- [x] **フロントエンドUI** (`apps/frontend/src/components/stealth/`)
  - [x] + メタアドレス生成 (`StealthAddressGenerator.tsx`)
  - [x] + 送金フォーム (`StealthSendForm.tsx`)
  - [x] + 残高一覧 (`StealthBalanceList.tsx`)
  - [x] + 使用フォーム (`StealthSpendForm.tsx`)
  - [x] + バックアップインポート (`BackupImportDialog.tsx`)
  - [x] + i18n対応 (ja/en/zh)

- ~~スキャナー設定管理 (P3優先度のためMVPスコープ外)~~
  - ~~スキャン頻度オプション~~
  - ~~バッテリー節約モード~~

### 3.2 反応マイニング

- [x] + **Reaction Pallet** 作成
  - [x] + 反応データ構造（いいね、ブースト、Bad）
  - [x] + 反応ストレージ（Reactions, ReactionStatsStorage, ReactionHistory）
  - [x] + `react` エクストリンシック
  - [x] + 二重反応防止チェック
  - [x] + 投稿者への報酬付与（ReactionRewardPoolから1 MORAL/反応）
  - [x] + PoW難易度検証（16ビット）
  - ~~報酬計算: `Reward = Σ(Reaction × Power_cpu) × γ`~~ → P4.4へ移動
  - ~~γ（インフレ調整係数）の動的計算~~ → P4.4へ移動
  - ~~ステルスアドレス報酬先対応（名寄せ防止）~~ → P3.5へ移動

- [x] + クライアント側PoW
  - [x] + WebWorkerでのマイニング実行（miningWorker.ts）
  - [x] + 難易度調整パラメータ取得
  - ~~マイニング報酬先の正当性検証~~ → ステルスアドレススキップのため不要

- [x] + 動的難易度調整
  - [x] + ネットワーク全体の反応レート監視（ReactionHistory）
  - [x] + 難易度自動調整アルゴリズム（on_finalize）
  - [x] + インフレ/デフレ抑制メカニズム（Min/MaxDifficulty制限）

### 3.3 DM機能（Stealth Messaging） → **完了** (2026-04-27 / 019-direct-messages)

> **実装内容**: 019-direct-messages 仕様に基づくフル機能 DM (送受信 / 配信レシート / バックアップ / ブロック / nickname 表示 / chain-node RPC 経由のストレージアクセス)。
> 当初仕様から拡張された箇所には `+` を付与。

- [x] **E2EE 実装**
  - ~~[ ] ChaCha20-Poly1305 暗号化~~ → [x] **AES-256-GCM 暗号化** (`packages/wasm-engine/src/dm/encrypt.rs`) で実装。鍵: HKDF-SHA256 出力 32B、Nonce: 同 12B、AAD に recipient_stealth + ephemeral_pub + padded_len を含む。
  - [x] 鍵導出 (HKDF-SHA256): `info = recipient_stealth ‖ eph_pub`、ECDH(scan_pub, eph_priv) を IKM として 44B OKM (key 32B + nonce 12B) を抽出
  - [x] メッセージパディング (ISO 7816-4 + 固定サイズ化): 5 段バケット `[1KB, 4KB, 16KB, 64KB, 256KB]` (`pallet-messaging::DM_PADDING_BUCKETS`)、`MaxDmCiphertextLen = 262_144`

- [x] **Messaging Pallet** (`apps/blockchain/pallets/messaging/`)
  - [x] ステルスアドレス宛メッセージ格納: `DmDispatchesByBlock<BlockNumber → Vec<DmDispatch>>` + `DmMessagesByRoot<[u8;32] → MessageId>` (256 dispatch/block 上限)
  - [ ] トラフィックパディング (ダミーメッセージ) → **未実装**: 本格的な timing/rate 解析耐性は Phase 4 以降で検討
  - [x] + `publish_dm_key(scan_pub, spend_pub)` extrinsic: `DmReceptionKeys: Map<AccountId → DmMetaAddress>`
  - [x] + `send_dm` extrinsic: コスト = `DmBaseCost (1 MORAL) + ct_len × DmByteCost (0.05 MORAL/byte)`、**80% storage / 10% stealth / 10% burn** で分配 (`do_deposit_to_reward_pool` / `do_deposit_to_stealth_reward_pool`)
  - [x] + Runtime API: `DmScanApi.reception_key(account)` / `DmScanApi.dispatches_range(from, to)` (1024 ブロックページング)
  - [x] + バリデーション: `k>0 && k<=n && n<=255`, `ct_len ∈ DM_PADDING_BUCKETS`, `eph_pub ≠ 0`, `merkle_root` 重複拒否

- [x] **クライアント側**
  - [x] メッセージスキャナー: `lib/dm/scanner.ts` (`scanDmInbox`) + `lib/dm/worker.ts` で foreground 15s / background 5min の visibility-aware ループ
  - [x] 復号・表示 UI: `components/dm/ConversationView.tsx` (`MessageBubble` / `GarbageCollectedBubble`)
  - [x] 送信フロー: `lib/dm/sender.ts::sendDm` 9 ステップ orchestrator + `components/dm/MessageComposer.tsx` (進捗 5 段表示、`TransactionDropped` retry)

- [x] + **送信者匿名化 (W3 / CT-1 / FR-021)**
  - [x] + `dm_generate_sender_stealth`: 32B 乱数 → `MiniSecretKey::expand_to_keypair(Ed25519)` で per-msg 使い捨て sr25519 鍵 (`packages/wasm-engine/src/dm/encrypt.rs`)
  - [x] + 2-tx 送信パターン: tx1 = `pallet_stealth.send_to_stealth` (Alice main 署名で sender_stealth へ pre-fund) → tx2 = `pallet_messaging.send_dm` (sender_stealth 署名)
  - [x] + 送信後 `senderStealthSeed.fill(0)` でゼロクリア (rented seed buffer 直接破棄、コピーは作らない)

- [x] + **受信者匿名化 (W5)**
  - [x] + `dm_derive_recipient_stealth`: ECDH(scan_pub, eph_priv) → per-msg `recipient_stealth` 派生、`ephemeral_pub` を chain dispatch に記録
  - [x] + 受信側スキャン: `dm_decrypt_scan(dispatch, scan_priv, spend_pub)` で全 dispatch を試行復号、`signature_valid == true` のみ採用 (FR-004)

- [x] + **配信レシート** (T078, FR-016 / FR-016b)
  - [x] + `delivered` レシート: 受信時に自動送信 (`/dm/page.tsx::onNewIncoming`)
  - [x] + `read` レシート: スレッド開封時に送信 (`ConversationView` の useEffect で `kind: 'read'`)
  - [x] + opt-out: `receiptOptOut: true` で受信確認を抑制
  - [x] + `applyReceipts` で送信側 deliveryState 遷移 (`sent → delivered → read`)
  - [x] + idempotent: `sentReceipts` セットで再送防止

- [x] + **ブロックリスト**
  - [x] + counterparty 別ブロック (Set<AccountId>)、ConversationList から非表示
  - [x] + DM 設定画面でブロック追加 / 解除 + IDB 永続化

- [x] + **バックアップ** (FR-022)
  - [x] + パスワード暗号化エクスポート (DM 鍵 + 会話履歴を JSON で出力)
  - [x] + インポート復元: stealth 鍵管理に書き戻し
  - [x] + 鍵は session memory only、ページ閉じで消失する設計を担保

- [x] + **永続化** (FR-019)
  - [x] + IndexedDB hydrate + subscription (`lib/dm/persistence.ts`)
  - [x] + `/dm/layout.tsx` で hydrate / persistence subscription をライフサイクル所有 → `/dm` ↔ `/dm/[id]` ナビゲーション越しに状態維持 (commit b7c90ee で fix)
  - [x] + optimistic addOutgoing → IDB に flush

- [x] + **UI 改善**
  - [x] + ConversationList でニックネーム表示 (`hooks/useNicknameOf.ts`、cache + inflight dedup)
  - [x] + ConversationView ヘッダにニックネーム + SS58 表示
  - [x] + MissingBackupNotice の単一 CTA 化 (「DM 鍵設定を開く」、commit 29877e8)
  - [x] + GarbageCollectedBubble: storage 取得不能時の placeholder (`gc:` プレフィックス)
  - [x] + 進捗表示 5 段 (`encrypting → uploading → prefunding → dispatching → done`)

- [x] + **ストレージアクセス統合** (CLAUDE.md Security Principle #5、commit 0903046)
  - [x] + storage-node 直叩き (`:3030 storage_storeFragment`) を廃止し、chain-node `:9944 storage_uploadFragment` / `storage_getFragment` 経由に移行
  - [x] + `dm_fragment_ciphertext` に per-leaf merkle proof を retain (`DmFragmentedOutput.proof(idx)`) → chain-node の `verify_merkle_proof` を通せる
  - [x] + `X-Anarchy-Auth` は frontend で生成し chain-node が body→header に展開して storage-node に forward
  - [x] + `NEXT_PUBLIC_STORAGE_ENDPOINT` 環境変数を削除、`SendDmContext.chainRpcEndpoint` に統一

- [x] + **メディア添付**
  - [x] + UI: `MessageComposer` にファイル選択 + 添付プレビュー + per-file 進捗表示
  - [x] + per-file AES-256-GCM(K_media) 暗号化、K_media は DM body envelope 内に格納 (E2E 担保)
  - [x] + DM body codec (`lib/dm/contentCodec.ts`、4-byte magic `DMC\x01` + JSON `{text, media[]}`)
  - [x] + アップロード/取得 lib (`lib/dm/media.ts`: `uploadDmMedia` / `fetchDmMedia`、内部で `dm_fragment_ciphertext` + chain-node `storage_uploadFragment` / `storage_getFragment`)
  - [x] + 受信側: `<DmMediaDisplay />` (decrypt → blob URL → image/video/file 各レンダリング)
  - [x] + EXIF 除去 (`lib/mediaProcessor.ts` 流用)
  - [x] + wasm-engine 拡張 (`dm_media_encrypt` / `dm_media_decrypt`、AES-256-GCM 統一)

### + 3.4 投稿人気度システム ✅

> **詳細**: [CONCEPTS.md](CONCEPTS.md#投稿人気度システム) / [docs/superpowers/specs/2026-05-03-post-popularity-design.md](superpowers/specs/2026-05-03-post-popularity-design.md) を参照

- [x] **人気度スコア計算** (pallet-popularity)
  - [x] 高評価（Like）: +N スコア (`LikeWeight = 100`)
  - [x] 低評価（Dislike/Bad）: +M スコア（関心として加点、`DislikeWeight = 50`）
  - [x] 時間経過: 相対減衰 (lazy `decay::apply`、`DecayRatePermill = 999_950`)
  - ~~フェッチ（閲覧）: +1 スコア~~ → **却下** (2026-05-03): Sybil 脆弱 + 匿名性矛盾 (Tor 下で IP dedup 不可) + 処理リソース (validator 負荷 / state bloat / storage→chain report 経路) の三重苦。CONCEPTS.md 参照
  - 追加変更: `pallet-reaction::ReactionType` から `Boost` を削除し Like/Bad の 2 種に整理 (Reddit 風 N/M モデル)

- [x] **Popularity Pallet** 作成 (`apps/blockchain/pallets/popularity/`)
  - [x] `PostPopularity` ストレージ（`stored_score`, `last_touched`, `like_count`, `dislike_count`, `marked_for_deletion_at`）
  - [x] `on_finalize` で bounded round-robin scan + lazy decay 適用 (`MaxPostsScannedPerBlock = 8`)
  - [x] 閾値以下の投稿をマーク + ヒステリシス復帰 (`LowPopularityThreshold = 1_000`, `HysteresisMargin = 500`)

- [x] **削除フロー**
  - [x] 猶予期間（`GracePeriod = 100_800` blocks ≈ 7 日）経過後に削除実行 (`MaxDeletionsPerBlock = 4`)
  - [x] ストレージノードへの削除通知 (`pallet_storage::Event::ForgottenByPolicy { content_hash }` を emit、`StorageInterface::do_release_fragment` 経由)
  - [x] オンチェーンメタデータ削除 (`PostMutator::delete_post` が Posts/ContentRefs/MerkleRootToPostId/UserPosts を prune)

- [x] **Sybil対策**
  - [x] 既存防御層に依存: `AlreadyReacted` チェック + PoW + faucet rate-limit + `PostBaseCost = 10 MORAL` (skin-in-the-game)。追加対策は v1 では入れず、実害確認後に v2 検討

- [x] **Runtime API** (`PopularityApi`)
  - [x] `get_effective_score(post_id) -> Option<u64>` (decay 適用後)
  - [x] `get_net_count(post_id) -> Option<i64>` (= like_count - dislike_count、派生)
  - [x] `get_post_popularity(post_id) -> Option<PostPopularityRpc>` (一括取得)

- [x] **Spec version bump**: 104 → 106 (popularity 追加 + PopularityApi 追加)

> **未実装 (v2 deferred)**: 永続化オプション（追加料金で削除対象外にする機能）、Reactor reputation/age weighting、Governance による decay rate 動的変更、frontend UI（人気度バッジ / 削除予告通知 / ランキング表示）、create-post + react PAPI helper scripts でのフル E2E。詳細は spec §1.2 参照。

<!-- ### 3.5 ステルスアドレス報酬先対応

> **目的**: 反応マイニング報酬先にステルスアドレスを指定可能にし、反応者と報酬受取口座の名寄せを防止

#あんまり意味ないからやめる捨て垢にステルス送金すればいいだけだし

- [ ] **pallet-stealth 作成**
  - [ ] ステルスアドレス生成（Ephemeral key + Recipient public key）
  - [ ] ステルスアドレス検証
  - [ ] 復号用スキャン機能

- [ ] **pallet-reaction との統合**
  - [ ] `react()` の `stealth_recipient` パラメータを有効化
  - [ ] ステルスアドレスへの報酬送付
  - [ ] 報酬先未指定時は投稿者メインアカウントへフォールバック

- [ ] **フロントエンド対応**
  - [ ] ステルスアドレス生成UI
  - [ ] 反応時の報酬先指定オプション
  - [ ] ステルス報酬スキャナー（受取確認） -->

---

## Phase 4: 本番デプロイ

### ~~4.1 Light Client 対応~~ → **Phase 2.5へ移動** (2026-02-11)

> Tor統合断念に伴い、smoldot導入を前倒し。詳細は Phase 2.5 を参照。

### ~~4.2 ハイドラ（フロントエンド業者）支援~~ → **削除** (2026-02-27)

> smoldot Light Clientにより各フロントエンドがRPC不要で接続可能。
> 業者向けドキュメントはメインネット安定後に必要に応じて作成。

### 4.3 テストネット/メインネット

- [ ] **テストネット公開**
  - [ ] パブリックブートノード設置
  - [x] Faucet（テスト用$moral配布）→ `pallet-faucet` で実装済み
  - [ ] Explorer統合

- [ ] **メインネット準備**
  - [ ] セキュリティ監査
  - [ ] Genesis設定最終化
<!-- 状況変化 (Phase B / PR #53): PoW + Permissionless GRANDPA に移行したため
     "validator 招集" の概念自体が消滅。誰でも `--mine` で参加可能、
     genesis bootstrap miner 1 名のみ chain_spec に焼き込みで完結。
  - [ ] バリデーター招集
-->


### 4.4 Mainnet設計・経済パラメータ（トークノミクス統合）

> 4.6の経済設計と統合。詳細設計は 4.5, 4.7 を参照。
> - 現状コードに存在する経済関連変数の全棚卸し: [`docs/economic_parameters.md`](economic_parameters.md)
> - **経済モデル設計提案 (TSTS) 2026-05-07**: [`docs/economic_model_proposal.md`](economic_model_proposal.md)
> - **実装計画 (8〜10 営業日)**: [`docs/economic_model_implementation_plan.md`](economic_model_implementation_plan.md)
> - シミュレータ: [`docs/economic/simulator.py`](economic/simulator.py)

- [ ] **経済合理性に基づく定数制定**
  - [ ] PostBaseCost / PostByteCost の最適値検証
  - [ ] Faucet報酬額・難易度の調整
  - [ ] ストレージ報酬レート設計
  - [ ] インフレ/デフレ率シミュレーション
  - [ ] 適切なガス代の設定
  - [ ] 初期供給量・分配比率

<!-- 状況変化 (Phase B PR #53):
     - "バリデーター" 概念は PoW 移行で消滅 (= miner)
     - 案A (ブロック報酬 mint) を採用、halving 付きで実装済み (pallet_block_reward, 5 MORAL → 4年毎半減)
     - 案D (EIP-1559) は PoA バリデーター前提で Anarchy 文脈ではミスマッチ → 不採用
- [ ] **バリデーター報酬設計**
  - [ ] 案A: ブロック報酬mint（シンプル、インフレ）
  - [ ] 案D: Ethereum EIP-1559方式（Base Fee burn + Priority Fee → バリデーター）
  - [ ] インフレ率とデフレ圧力のバランス検証
-->

- [x] **ストレージ・反応報酬設計** — TSTS v1 で全実装済 (PR #54, #55、[economic_parameters.md](economic_parameters.md))
  - [x] ストレージノード報酬設計 — [`pallets/storage_stake/`](../apps/blockchain/pallets/storage_stake/) + Phase 4 repair reward pool
  - [x] 反応マイニング報酬曲線 — [`pallets/reaction/`](../apps/blockchain/pallets/reaction/)
  - [x] 動的報酬計算: `Reward = Σ(Reaction × Power_cpu) × γ` — `pallets/economic_params/` + `pallets/reaction/`
  - [x] γ（インフレ調整係数）の動的計算（ReactionRewardPool / TotalSupply）— `pallets/economic_params/`、governance 可変

- [x] **手数料モデル** — TSTS v1 で確定済
  - [x] TX手数料: **Base Fee 導入** (EIP-1559) を採用 — [`pallets/base_fee/`](../apps/blockchain/pallets/base_fee/) (PR #54 commit `200ddb4`)
  - [x] 投稿コスト: burn 維持 — `PostBaseCost = 25 MORAL` + `PostByteCost = 0.0008 MORAL/byte` (`runtime/src/lib.rs`)
  - [x] Faucet: unsigned tx 維持 — §2.3 で完了済 (`pallets/faucet/`)

### + 4.5 オンチェーンガバナンス

> **詳細**: [CONCEPTS.md](CONCEPTS.md#オンチェーンガバナンス) を参照

- [ ] **段階的移行計画**
  - [ ] 開発〜テストネット: pallet_sudo維持（単一管理者）
  - [ ] メインネット初期: Multisig（コア開発者数名）
  - [ ] メインネット安定後: Democracy/OpenGovへ移行

- [ ] **Multisig導入**
  - [ ] pallet_multisig 設定
  - [ ] 署名者リスト・閾値設定
  - [ ] ランタイムアップグレード承認フロー

<!-- 哲学レビュー後保留 (Phase B PR #53):
     「$moral 保有量ベースの投票権」は同 sub-section の "経済的攻撃 ($moral 買い占め) 対策"
     と内在的に矛盾する (token-weighted = 大口買い占めで支配可能)。
     さらに on-chain vote は public ledger に記録されるため、Anarchy 匿名性原則と衝突。
     完全解決には zk-SNARK 投票が必要 = 大物別タスク。
     governance は Multisig (上記 §4.5 Multisig 導入で対応) に留め、本格 OpenGov は再設計後。
     spec は: 採掘で finality 投票権が自動付与される現状で十分という見方もある。
- [ ] **Democracy/OpenGov導入**（将来）
  - [ ] pallet_democracy / pallet_referenda 導入
  - [ ] $moral保有量ベースの投票権
  - [ ] Conviction voting（ロック期間に応じた投票力増加）
  - [ ] Track別投票システム（技術提案 vs コミュニティ提案）
  - [ ] 緊急時対応（セキュリティパッチ等）の特別ルート
  - [ ] 投票期間・クォーラム閾値の設定
  - [ ] パラメータ変更プロセス
-->

- [ ] **セキュリティ考慮**
  - [ ] 経済的攻撃（$moral買い占め）対策
  - [ ] 最小投票期間の設定
  - [ ] 提案スパム防止（デポジット要求）
<!-- 状況変化: Anarchy は anonymity 原則 (Tor/I2P 強制) のため frontend は
     原則 .onion service 経由で接続する想定。clearnet 公開しないので HTTPS 単独の
     対応は不要 (Tor 経由なら transport 暗号化は libp2p/Tor が担う)。
     clearnet ミラーを置く運営者は HTTPS 必須だが、それは運営者責任。
  - [ ] https対応
-->

### ~~+ 4.6 経済設計（トークノミクス）~~ → 4.4に統合

### + 4.7 コンセンサス方式の検討（PoA → PoW/NPoS）

> **詳細**: [CONCEPTS.md](CONCEPTS.md#コンセンサス方式の検討poa--pow) を参照
> **実装**: Phase A PR #52 (merged) + Phase B PR #NN (in review)
> **Spec**: [docs/superpowers/specs/2026-05-06-pow-migration-design.md](superpowers/specs/2026-05-06-pow-migration-design.md)

- [x] **PoW移行検討** (2026-05-NN 完了)
  - [x] アルゴリズム選定: **RandomX** 採用 (ASIC 耐性 / Anarchy 原則 "誰でも参加" と整合)
  - [x] ASIC耐性の要否判断: **必要** (匿名・分散原則のため CPU 優位な RandomX を選定)
  - [x] 難易度調整アルゴリズム実装: **LWMA-3** (Kulupu 流派, unweighted harmonic mean)
  - [x] ファイナリティ方式変更: **PoW + Permissionless GRANDPA** (top-K miner rotation, sudo 介在なし)

- [x] **NPoS（Hybrid）検討** → 不採用 (Permissionless GRANDPA で代替)
  - 理由: NPoS は MORAL ステークが必要で「誰でも参加」原則と矛盾。top-K miner rotation で
    permissionless finality を実現することで NPoS なしで分散性を確保。

- [x] **移行計画**
  - [x] Phase A: pallet 3 個 + node/pow モジュール追加 (#52)
  - [x] Phase B: runtime cutover + RandomX verify + miner loop + chain_spec / CLI / CI / staging integration / docs (#53)
  - [x] mainnet runbook 公開: [docs/operations/pow-mainnet-runbook.md](operations/pow-mainnet-runbook.md)

- [x] **Phase B 副作用 — frontend 接続経路変更**
  - [-] ~~smoldot light client~~ (PoW 非互換、§2.5 参照)
  - [+] **WebSocket (`getWsProvider`)** に統一: `lib/chain-client.ts` / `hooks/useChain.ts`
  - [+] dev / testnet 起動コマンドに `--mine --coinbase` を自動付与 (`package.json`, `run-multi-node.sh`)
  - [+] E2E spec 拡充 (`transfer.spec.ts` / `nickname.spec.ts` / `stealth-transfer.spec.ts` / `pow-chain-sync.spec.ts`)
  - [+] hooks/services の signAndSubmit timeout 30s → 240s (PoW 30s blocktime 対応)

- [ ] **Phase C 残タスク** (mainnet 投入前 or 直後に対応)
  - [ ] **Equivocation slashing**: `EquivocationReportSystem = ()` で現状二重投票し放題。`pallet_offences` + `pallet_grandpa::report_equivocation` 連動で top-K position を BAN する
  - [ ] **RandomX seed の epoch rotation**: 現状 genesis hash 固定 (`randomx_algo.rs`)。`RANDOMX_EPOCH_BLOCKS = 2048` 単位で seed 切替へ
  - [ ] **本番 MinDifficulty チューニング**: 現状 dev 用 `100`。`scripts/bench-randomx.sh` で reference HW の hashrate 実測 → mainnet chain_spec の initial_difficulty を確定
  - [ ] **Faucet と halving の整合**: 現 100 MORAL/claim 永久 → halving 連動で減額検討 (mainnet 経済データ次第)
  - [ ] **Genesis bootstrap miner key の運用方針確定**: 焼き込んだ GRANDPA key の秘密鍵を破棄するか、destroy ceremony 公開するか
  - [ ] **WSL2 timestamp drift**: dev 環境で散発的に "block timestamp too far in the future" reject。実 Linux で再現するか確認、しなければ無対応

### + 4.8 Storage ↔ Chain Session 認証強化 (TODO 追加 2026-04-27)

> **目的**: chain-node ↔ storage-node 認証を [docs/storage_logic.md §7](storage_logic.md#7-セッション認証システム) に書かれた **session-token 方式** に実装し直す。
>
> **背景**: 現状は per-request の `X-Anarchy-Auth` + `X-Chain-Auth` ヘッダ方式 (`apps/storage-node/src/rpc/auth.rs`) で動作しているが、`X-Chain-Auth` は同ファイルの comment で「なりすましは許容＝公開鍵のオンチェーン確認はしない」と明言されており、**sr25519 鍵を持つ任意のユーザが chain-node を装って storage-node に書き込める**。docs §7 が想定する libp2p P2P 接続経由 (`peer_id ∈ connected_peers`) での peer 認証は実装ファイル (`apps/storage-node/src/session/`, `apps/blockchain/node/src/storage/session_client.rs`) ごとまだ存在しない。
>
> **緊急度**: 低。Principle #1/#5 の匿名性は現状でも担保されており、攻撃面 (悪意ある "chain-node" による storage 書き込み) を塞ぐ強化策。019-direct-messages リリース後に着手で良い。
>
> **ボリューム感**: 半日〜1.5 日 (Rust 側のみ、フロントは触らない)

- [ ] **storage-node 側** (`apps/storage-node/src/session/` 新設)
  - [ ] `token.rs` — `SessionToken` (UUID + expiry), `SessionInfo` (issued_to, last_seen_at)
  - [ ] `registry.rs` — `SessionRegistry: Map<token, SessionInfo>` + GC ループ (期限切れ削除)
  - [ ] `peers.rs` — `ConnectedPeers: Set<PeerId>` (libp2p 接続イベントから更新)
  - [ ] `protocol.rs` — `SessionRequest { public_key, timestamp, nonce, signature }` の型 + 署名検証
  - [ ] `error.rs` — `SessionError` 列挙
  - [ ] `rpc/mod.rs` に `POST /session` 追加 (peer_id が `connected_peers` に居るときのみ token 発行)
  - [ ] `rpc/auth.rs` 改修: `X-Session-Token` 検証経路を追加 (旧ヘッダ方式は dev fallback として残す)
  - [ ] `storage/store`, `storage/delete` を session-token 必須に切替

- [ ] **chain-node 側** (`apps/blockchain/node/src/storage/` 新設)
  - [ ] `session_client.rs` — `StorageSessionClient { http, token: Mutex<Option<(SessionToken, Instant)>>, signing_key, target_url }`
  - [ ] `ensure_session()` 実装 — token 期限切れなら `/session` に再取得 (libp2p で繋がっている前提)
  - [ ] `upload()` / `get()` — 内部で `X-Session-Token` ヘッダ付与
  - [ ] `rpc/storage.rs` の `StorageNodeClient` を `StorageSessionClient` に置き換え (or 内包)
  - [ ] `node/main.rs`: 起動時に storage-node と libp2p 接続を確立する初期化シーケンス

- [ ] **整合性 / テスト**
  - [ ] `docs/storage_logic.md §7` の図と実装が一致することを確認
  - [ ] storage-node auth テスト更新 (X-Session-Token 経路の正常系・異常系)
  - [ ] 統合テスト: chain-node ↔ storage-node の session 確立 → upload → token 失効 → 再取得 のフロー
  - [ ] 既存 `X-Anarchy-Auth` テストが dev fallback 経路として残ることを確認

### + 4.9 Storage Node DB 最適化 (TODO 追加 2026-05-07)

> **進捗 (2026-05-10)**: Phase 1 として redb (4.x, pure Rust, ACID, B-tree) を即採用し、`fragments` / `post_fragments` 2 テーブル構成で backend を入替済 (PR `feature/storage-node-kv-redb`)。公開 API は維持、書き込みは 1 redb txn で原子化、起動時 `walkdir` を redb iter に置換。inode インフレと部分書き残留はこの時点で解消。
>
> **進捗 (2026-05-10) Phase 2 着手**: 基盤 + 可観測性 (PR `feature/storage-node-kv-redb-phase2`)。`storage/{engine,metadata}.rs` 分割 + `fragment_meta` / `post_fragment_meta` SCALE-encoded メタデータテーブル + `system.total_used_bytes` 永続化 (起動時 O(N) scan 廃止) + `verify_on_read` config flag (Blake2 hash mismatch を HTTP RPC で error 化、E2E で 1 byte flip を検出済) + Phase 1 で見つかった metrics 未配線バグ修正 (`record_put` / `record_get` を rpc handler に追加 + `/metrics` の `storage_capacity_used_bytes` / `storage_fragments_total` を store からライブで読み出し)。
>
> **進捗 (2026-05-10) Phase 2 完了** (PR `feature/storage-node-kv-redb-phase2-finish`): LRU eviction (`evict_lru`) + touch-on-read 非同期更新 (60s tick で flush) + main.rs に capacity 95% 超過時の自動 eviction trigger + bench scripts (`bench-storage.sh` + `bench-storage` bin、TSV 出力) + `storage-backup.sh` (SIGSTOP/SIGCONT による hot backup) + SIGKILL crash test (6/20 fragments survived, no half-written, used_bytes 完全一致) + 5K fragments load test (1780 ops/s, quota 完全遵守). **§4.9 の全項目クローズ**。

> **目的**: storage-node の fragment 永続化層を「1 fragment = 1 ファイル」のナイーブ実装から、embedded KV ストア (sled / redb / fjall / RocksDB) ベースに置き換えて、fragment 数 100 万件超でもスケールするようにする。
>
> **背景**: 現在の [`apps/storage-node/src/storage/mod.rs`](../apps/storage-node/src/storage/mod.rs) は `fs::create_dir_all` + `File::create` + `file.write_all` で fragment ごとに 1 ファイル書き出す。問題点:
> - **inode インフレ**: 100 万 fragment = 100 万 inode (ext4 で `ls` / `find` が秒オーダー、xfs 推奨だが SD/HDD では fragmentation 累積)
> - **fsync per fragment**: 書き込みが一切 batch されず writeback 圧迫
> - **GC / capacity が O(N) 全走査**: `walkdir` crate でディレクトリ再帰 (起動時 + 周期実行)、再起動が遅い
> - **メタデータ取得に毎回 stat(2)**: `status` RPC で fragment_id ごとに `fs::metadata` を呼ぶ → ホットパスで syscall 過多
> - **integrity check なし**: 読み出し時に hash 検証していない (line 157 コメントで明言、bit rot 検知不可)
> - **atomic rename ではない**: `File::create` → `write_all` → close、途中 crash で部分書き fragment が残る (削除側は `path.exists()` で誤検知)
> - **圧縮なし**: 暗号化済み binary でも prefix 共通領域あり、storage 効率が悪い
> - **WAL / snapshot ベースのバックアップが取れない**: rsync しかない、incremental が雑
>
> **緊急度**: 中。MVP / testnet では問題ないが、real-world dataset (1M+ fragments / per node) で確実に頭打ちになる。mainnet 公開前には完了させたい。
>
> **ボリューム感**: 1〜2 週間 (KV 選定 0.5 日 + 移行設計 1 日 + 実装 5〜7 日 + ベンチ 2 日 + テスト/PR review)。
>
> **互換性方針** (CLAUDE.md §Compatibility Policy より): 旧フォーマットからのマイグレーション不要。新 storage は wipe して再生成可。

- [x] **KV エンジン選定** (PoC + ベンチ) — Phase 1 では redb 即採用。形式ベンチは Phase 2。
  <!-- 不採用 (1.0 未到達 + 後継 bloodstone へ移行中):
  - [ ] **候補 A: sled** — pure Rust, log-structured, embed しやすいが mature でない (1.0 未到達, 後継 bloodstone へ移行中) → 不採用
  -->
  - [x] **候補 B: redb** — pure Rust, ACID, B-tree、4.x stable、tuple key + range scan 良好 → **採用**
  <!-- 検討終了 (2026-05-10): Anarchy のワークロードは投稿作成 (write rare)
       + ページ閲覧 (read frequent) で read-heavy。Phase 2 smoke で 4KB
       put 27K ops/s に対し get 167K ops/s と read が 6× 速く B-tree が
       適合中。LSM (fjall) のメリットが薄いと判断し採用見送り。将来
       workload が write-heavy 化したら再検討。
  - [ ] **候補 C: fjall** — pure Rust, LSM-tree, write-heavy 向け → Phase 2 で write 比重が高ければ再検討
  -->
  <!-- 見送り (C++ FFI による build 時間 / バイナリサイズ増、pure Rust スタックを崩したくない):
  - [ ] **候補 D: rocksdb** — C++ FFI, 実績豊富だが build 時間 + バイナリサイズ増 → 見送り
  -->
  - [x] ベンチ条件: 1M fragments × {64 KiB, 256 KiB, 1 MiB} で `put` / `get` / `delete` / `range_scan` の throughput と p99 レイテンシ、起動時間、`du -sh` (on-disk size) — Phase 2 finish で [`scripts/bench-storage.sh`](../scripts/bench-storage.sh) + `bench-storage` bin として実装。1M cell は operator 実行 (TB 級ディスク要)、CI では `--quick` (10K cell)

- [x] **データモデル設計** (`apps/storage-node/src/storage/`) — `index_by_post` 独立化を除き完了 (条件付き future work)
  - [x] `fragments` table: `key = FragmentId (&[u8;32]) → value = bytes` (Phase 1)
  - [x] `post_fragments` table: `key = (u64 post_id, u32 index) → value = bytes` (Phase 1, 逆引きと値兼用 — challenge/repair で fragment_id 逆引きが要求されたら細分化)
  - [x] `fragment_meta` / `post_fragment_meta` table: `key → SCALE-encoded { version, size, created_at, last_accessed_at, ref_count, data_hash }` (Phase 2、`metadata.rs` に格納)
  - [ ] `index_by_post` table を独立化: 値を `fragment_id` に切り替え、bytes 本体は `fragments` 経由 — **次フェーズ** (challenge / repair で fragment_id 逆引きが必要になったタイミング)
  - [x] `total_used_bytes` を redb の system table に永続化 (起動時 scan 廃止、ただし key 欠損時は fallback 1 回だけ scan して書き戻し) (Phase 2)

- [x] **実装 (`apps/storage-node/src/storage/`)**
  - [x] `mod.rs` を `engine.rs` (Database + 全 table 定義) と `metadata.rs` (SCALE struct) と `mod.rs` (FragmentStore = ドメイン層) に分割 (Phase 2)
  - [x] `store(fragment_id, data)` → redb の `insert` を 1 write txn (atomic) (Phase 1)
  - [x] `retrieve(fragment_id)` → redb の `get` (Phase 1)
  - [x] `delete(fragment_id)` → 1 write txn、`used` 減算 (Phase 1)
  - [x] `list_post_fragments(post_id)` → `(post_id, 0)..=(post_id, u32::MAX)` の range scan (Phase 1)
  - [x] `last_accessed_at` の非同期 update (Phase 2 finish): `record_touch_*` で in-memory TouchBuffer に積み、main.rs の 60s tick で `flush_touch_buffer` が 1 write txn で書き出す (read ホットパスに fsync を載せない)
  - [x] integrity verify: 読み出し時 blake2 hash 比較フラグ (`config.toml` の `verify_on_read = true|false`、`Metadata.data_hash` と比較、E2E で 1 byte flip 検出済) (Phase 2)
  - [x] backup API: SIGSTOP で書き込みを一瞬止めて redb single-file を `cp -p` するシンプルな方式に着地 (`apps/storage-node/scripts/storage-backup.sh`) — `begin_read` 経由の consistent snapshot は将来 redb 側に hot-snapshot API が入ったら切り替え

- [x] **可観測性 (rpc/mod.rs metrics 配線)** — Phase 2 E2E で発覚した既存バグの修正
  - [x] `record_put()` / `record_get()` を `handle_store_fragment` / `handle_get_fragment` / `handle_store_kzg_shard` に追加
  - [x] `/metrics` の `storage_fragments_total` / `storage_capacity_used_bytes` を `state.store` からライブで読み出すように変更 (`metrics.fragment_count` などの内部 atomic は不使用)

- [x] **GC / capacity 改修** (`apps/storage-node/src/gc/`)
  - [x] `walkdir` 全走査を redb の range scan に置換 (Phase 1 で `list_fragments()` 内部、Phase 2 final で `evict_lru` の `collect_eviction_candidates` も redb iter ベース)
  - [x] capacity check は atomic counter 参照のみ (Phase 1 で達成、ファイル書き出しの 2 段階は redb txn 1 段階に集約)
  - [x] LRU eviction: `metadata.last_accessed_at` で sorted scan → 古い順に eviction (Phase 2 final、`evict_lru` + main.rs gc tick で capacity > 95% 時に target 85% へ自動 evict)

- [x] **マイグレーション** (= 不要、wipe & rebuild)
  - [x] CLAUDE.md §Compatibility Policy より migration コードは書かない。旧 `.bin` データは捨て、新 redb DB を起動時に自動生成。
  - [x] `pnpm storage:purge` (= `apps/storage-node/scripts/run-storage-nodes.sh purge`) を [docs/storage_logic.md §3 運用コマンド](storage_logic.md) に明記

- [x] **テスト**
  - [x] 既存 15 件の `storage::tests` が redb 化後も通過 (Phase 1)
  - [x] `test_persistence_across_reopen` を追加 (drop → 再 open で fragment と used counter が復元) (Phase 1)
  - [x] `test_store_writes_metadata` / `test_verify_on_read_passes_for_clean_data` / `test_verify_on_read_catches_bit_rot` / `test_persistent_counter_loaded_on_reopen` / `test_counter_recovers_when_system_key_missing` (Phase 2)
  - [x] metadata.rs unit: SCALE roundtrip + 未知 version 拒否 (Phase 2)
  - [x] `test_evict_lru_noop_below_target` / `test_evict_lru_removes_oldest_first` / `test_touch_buffer_persists_last_accessed_at` (Phase 2 final、計 26/26 storage unit test PASS)
  - [x] [`scripts/bench-storage.sh`](../scripts/bench-storage.sh) + `bench-storage` bin (TSV 出力、`--quick`/`--full`/カスタム): 1K × {4K, 64K} smoke で put 27K ops/s、p99=83μs を確認。1M × {64K, 256K, 1M} の "--full" は operator 実行 (TB 級ディスク必要)
  - [x] crash test: [`apps/blockchain/tests/integration/test_storage_crash_recovery.sh`](../apps/blockchain/tests/integration/test_storage_crash_recovery.sh) — 20 並行 store 中に SIGKILL → 6 fragments 生存、verify_on_read 全 PASS、used_bytes 完全一致
  - [x] [`apps/blockchain/tests/integration/test_storage_load.sh`](../apps/blockchain/tests/integration/test_storage_load.sh) — 5K fragments × 1KiB 投入 (default) / 100K (`--large`)、quota 完全遵守 + 早期 fragments の verify_on_read PASS

- [x] **ドキュメント**
  - [x] [docs/storage_logic.md](storage_logic.md) §Persistence 章を追記 / 更新 (Phase 1) + LRU eviction 行 + 運用コマンド表 (Phase 2 finish)
  <!-- 現状 redb 一択で書く対象がない。fjall を Phase 2 で評価して採用したら復活:
  - [ ] config option `storage.engine = "redb" | "fjall" | …` の README 追加
  -->

### + 4.10 ノード運用者ダッシュボード (TODO 追加 2026-05-07)

> **目的**: storage-node / mining-node を自分で運用している人が、自分のノードの状態 (容量・hashrate・収益・peer 健全性・スラッシング履歴) を見るための **ローカル限定 Web UI** を用意する。
>
> **スコープ外** (Anarchy 哲学との衝突を避けるため明記):
> - ❌ 他人のノードを管理する画面 (= 中央集権化)
> - ❌ 投稿削除 / user ban / KYC 操作 (= "Order Without Rulers" に反する)
> - ❌ 公開 endpoint (常に `127.0.0.1` バインド + 起動時に認可トークン file 出力)
>
> **背景**: 現状の運用者向け可視化は以下のみで、UI が無いため運用者は自分でクエリを書く必要がある:
> - storage-node: [`apps/storage-node/src/metrics.rs`](../apps/storage-node/src/metrics.rs) に `AtomicU64` カウンタ群 (fragment_count / capacity_used / connected_peers / gossip_messages_* / auth_failures / chain_failovers / chain_latency_ms 等) があるが Prometheus exporter は未配線
> - chain-node: Substrate 標準の Prometheus exporter (`--prometheus-port 9615`) で best/finalized/peers などは出るが、Anarchy 固有の miner 状態 (hashrate / coinbase 残高 / RandomX mode / authority set 所属 / Tor mode) は出ない
>
> **緊急度**: 低-中。MVP では nice-to-have だが、testnet 〜 mainnet で第三者運用者を集めるには必須 (運用者体験の悪さは participation rate に直結)。
>
> **ボリューム感**: 1〜2 週間 (バックエンド metrics 追補 3〜4 日 + フロント 4〜5 日 + auth 1 日 + パッケージング 1 日)。
>
> **配布方針**: 別バイナリに切らず、storage-node / mining-node の **同一バイナリに静的アセット同梱** (`rust-embed` 等) し、`--console-port 9090` で起動。Tor mode と独立、必ず `127.0.0.1` 限定。

- [ ] **共通バックエンド** (`apps/storage-node/src/console/` & `apps/blockchain/node/src/console/`)
  - [ ] `mod.rs` — axum サブ router、`127.0.0.1` 強制バインド
  - [ ] `auth.rs` — 起動時に `~/.anarchy/console-token` (chmod 600) を出力、HTTP は `Authorization: Bearer <token>` 必須
  - [ ] `assets.rs` — `rust-embed` で SPA 静的ファイル配信
  - [ ] `events.rs` — Server-Sent Events で metrics を 1s 間隔 push (poll より省 CPU)

- [ ] **Storage operator metrics 拡充** (`apps/storage-node/src/`)
  - [ ] `metrics.rs` を Prometheus exposition format で `/metrics` endpoint に出す (運用者は Grafana 連携も可能に)
  - [ ] 追加 metrics:
    - 自分の AccountId に対する on-chain `Storage.NodeRewards` (累計報酬)
    - 自分宛の最近のスラッシング event (last 100)
    - Repair queue 深さ (`apps/storage-node/src/repair/`)
    - 自分が hold している post 数と byte 数 (現状の fragment_count とは別軸)
  - [ ] `console/` ハンドラ:
    - `GET /api/status` → `{ peer_id, version, uptime_sec, tor_mode, onion_addr?, capacity, fragments }`
    - `GET /api/peers` → libp2p connected peers (peer_id, multiaddr, latency)
    - `GET /api/rewards/summary` → 24h / 7d / 30d 集計
    - `GET /api/slashing/recent?limit=20`

- [ ] **Mining operator metrics 拡充** (`apps/blockchain/node/src/`)
  - [ ] miner thread (`service.rs` 周辺) に hashrate sliding window (1s / 1m / 5m) を追加
  - [ ] `console/` ハンドラ:
    - `GET /api/status` → `{ peer_id, best, finalized, tor_mode, onion_addr?, miner_running, randomx_mode }`
    - `GET /api/miner` → `{ coinbase, hashrate_1s, hashrate_1m, blocks_won_24h, last_won_block_height, current_difficulty, last_10_targets }`
    - `GET /api/coinbase/balance` → `pallet_balances::Account[coinbase].free` (chain RPC で自分自身に問い合わせ)
    - `GET /api/authority` → GRANDPA authority set 所属判定 + 直近の vote 統計 (該当時のみ)

- [ ] **共通フロント** (`apps/node-console/`, 新パッケージ)
  - [ ] Vite + React (SSR 不要、SPA で OK) または Next.js static export — Wasm 不要なので軽量化優先
  - [ ] 単一ページに `<NodeStatusCard>` (storage / mining 自動判別) + `<MetricsTimeSeries>` (SSE 受信) + `<PeerList>` + `<RewardsPanel>` + `<EventLogTail>`
  - [ ] i18n EN/JA/ZH (frontend と同じ I18n 規約)
  - [ ] `pnpm build` で `dist/` を生成 → Rust 側で `rust-embed("./apps/node-console/dist/")`
  - [ ] **DM などユーザ機能は持たない** (運用者画面と user 画面は完全分離する)

- [ ] **CLI / config**
  - [ ] storage-node: `--console-port 9090` / `--no-console` フラグ追加 ([`apps/storage-node/src/config/`](../apps/storage-node/src/config/))
  - [ ] chain-node: 同様 ([`apps/blockchain/node/src/cli.rs`](../apps/blockchain/node/src/cli.rs))
  - [ ] `mainnet` chain id では default 無効化 (誤って exposed されないように、`--console-port` 明示時のみ有効)

- [ ] **セキュリティ**
  - [ ] `127.0.0.1` 以外のリクエストを reject (Tor mode と独立にチェック)
  - [ ] CSRF 対策: SameSite=strict + Origin チェック
  - [ ] token rotate: SIGHUP で再生成、ファイルパーミッション (chmod 600) を起動時に強制
  - [ ] 監査ログ: console アクセスは別ファイルに JSON で記録 (誰がいつ何を見たか、ローカル運用者の self-audit 用)

- [ ] **テスト**
  - [ ] `apps/storage-node/src/console/tests.rs`: 認証失敗 / 認証成功 / 非 localhost reject / SSE 切断 reconnect
  - [ ] フロント Jest: 各カードがバックエンド response shape の breaking change に追従できること (型生成は OpenAPI or `tRPC` を検討)
  - [ ] E2E: storage-node 起動 → console 開く → fragment 投入 → カウンタが 1s 内に更新されることを確認

- [ ] **ドキュメント**
  - [ ] `docs/operator_console.md` 新規 — 起動方法、token 場所、画面の見方、Grafana 連携 (Prometheus `/metrics` 経由)
  - [ ] [README.md](../README.md) の「ノードを運用する」節からリンク

---

## 構想事項（検討中）

> **別ドキュメントに移動**: [CONCEPTS.md](CONCEPTS.md) を参照
>
> - ~~経済設計（トークノミクス）~~ → Phase 4.6へ移動
> - ~~コンセンサス方式の検討（PoA → PoW）~~ → Phase 4.7へ移動
> - ブラウザ拡張ウォレット連携
> - ~~オンチェーンガバナンス~~ → Phase 4.5へ移動
> - 残高保護機能（Keep Alive強制）
> - ~~投稿人気度システム~~ → Phase 3.4へ移動
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
| **6** | + **013-slashing-repair** | Slashing & 自己修復プロトコル | [spec.md](../specs/013-slashing-repair/spec.md) | ✅完了 (2026-02-24) |
| **7** | + **016-stealth-address** | ステルスアドレス統合 | [spec.md](../specs/016-stealth-address/spec.md) | ✅完了 (2026-02-28) |

### Phase 1 スコープ（まず繋がるだけ） → ✅完了 (2026-02-10)

- ✅ Storage Pallet: `register_fragment`, `register_node`, `declare_holding`
- ✅ Storage Daemon: libp2p断片送受信、ディスク保存
- ✅ + HTTP JSON-RPC API: フロントエンド連携
- ✅ + 自動登録 + heartbeat: ブロックチェーンノードへの登録
- ~~❌ PoST~~ → ✅ Phase 3 (KZG証明)
- ~~❌ 報酬~~ → ✅ Phase 3 (KZG報酬システム)
- ~~❌ スラッシング~~ → ✅ Phase 4 (013-slashing-repair)
- ~~❌ 自己修復~~ → ✅ Phase 4 (013-slashing-repair)

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
                                                     ├─ + 011-kzg-proof-rewards ✅ (2026-02-16)
                                                     │
                                                     └─ + 013-slashing-repair ✅ (2026-02-24)

Phase 3.1 (Stealth) ✅ (2026-02-28) ─── Phase 3.2 (Reaction) ✅ (2026-03-01) ─┬─ Phase 3.3 (DM)
                                                     │                         │
                                                     │                         └─ Phase 3.4 (Popularity)
                                                     │
                                                     └── Phase 3.5 (Stealth Rewards) → P4.4 or 後続

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
| + **Slashing & Self-Repair** | 高 | 高 | **13** | ✅完了 (2026-02-24) |
| + **ステルスアドレス** | 中 | 中 | **14** | ✅完了 (2026-02-28) |
| + **反応マイニング** | 中 | 中 | **15** | ✅完了 (2026-03-01) |
| ~~ZKP回路~~ | ~~低~~ | ~~高~~ | ~~16~~ | →構想移動 |

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

### M5: プライバシー機能 ✅完了 (2026-02-28)
- ~~SSS断片化~~ → ✅完了 (2026-02-10)
- ステルスアドレス → ✅完了 (2026-02-28)

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
  - [x] + Blake2b PoWマイニング (`compute_pow_js`, 反応マイニング用)

- [x] + **pallet-storage KZG報酬システム**
  - [x] + `register_fragment_kzg`: commitment + deposit登録
  - [x] + `prove_holding_kzg`: KZG proof検証 + 報酬請求
  - [x] + RewardPool: 投稿費用90% → プール、10% burn
  - [x] + holder数ベース均等分配 / ScoreProvider対応

- [x] + **GCライフサイクル** ✅ 完了 (2026-02-16)
  - [x] + FragmentState: StateProposed → Active → ForgettingCandidate
  - ~~[x] + `on_finalize` GC: 自動削除~~ → **オンチェーン削除なし**: BTCと同様メタデータ永続
  - [x] + **Storage Node側GC**: RewardPool閾値ベースで物理データ自動削除
    - [x] + `storage_getRewardPoolBalance` RPC追加
    - [x] + `GarbageCollector::update_pool_balance()`: 5分間隔でプール残高チェック
    - [x] + `FragmentStore::delete_all()`: プール枯渇時に全断片削除

- [x] + **Storage Node証明提出** ✅ 完了
  - [x] + challenge.rs構造
  - [x] + prover.rs: SRS読み込み (`load_srs_from_file`, `load_srs_from_ceremony_text`)
  - [x] + 証明の自動提出 (`chain/mod.rs::submit_holding_proof`)

- [x] + **フロントエンド統合**
  - [x] + HybridShard構造対応
  - [x] + Reed-Solomon復元ロジック

### + M9: Slashing & Self-Repair ✅完了 (2026-02-24)

> **実装内容**: 013-slashing-repair仕様に基づく自己修復プロトコル

- [x] + **スラッシングシステム**
  - [x] + ProofRecord拡張: `slashed: bool`, `share_index: u8`
  - [x] + `do_slash_node()`: チャレンジ3回失敗でスラッシュ
  - [x] + 担保50%没収 → RepairRewardPool

- [x] + **FragmentState管理**
  - [x] + FragmentStateKind: Active/AtRisk/Repairing/Lost
  - [x] + `update_fragment_state()`: 状態遷移ロジック
  - [x] + Runtime API: `get_at_risk_fragments()`, `get_fragment_state()`

- [x] + **自己修復プロトコル**
  - [x] + `regenerate_share()`: Lagrange補間でシェア再生成 (wasm-engine)
  - [x] + repair coordinator/scheduler (storage-node)
  - [x] + `confirm_repair` extrinsic: 修復完了確認

- [x] + **余剰ホルダーGC**
  - [x] + `evict_stale_holder` extrinsic
  - [x] + `compute_eviction_candidates()`: 優先度計算
  - [x] + StaleHolderGc (storage-node): 自動GCサイクル

- [x] + **RPC監視エンドポイント**
  - [x] + `storage_getAtRiskFragments`, `storage_getFragmentState`
  - [x] + `storage_getEvictionCandidates`, `storage_getFragmentsWithExcessHolders`
  - [x] + `storage_repairStatus` (storage-node)

### + M10: 反応マイニング ✅完了 (2026-03-01)

> **実装内容**: 017-reaction-mining仕様に基づくPoWベースの反応システムとクリエイター報酬

- [x] + **pallet-reaction**
  - [x] + 反応データ構造: Like, Bad (旧 Boost は §3.4 で Like/Bad へ統合・削除)
  - [x] + ストレージ: Reactions, ReactionStatsStorage, ReactionHistory, ReactionRewardPool
  - [x] + `react()` extrinsic: PoW検証 + 報酬付与
  - [x] + 二重反応防止チェック
  - [x] + PostAuthorProvider trait (pallet-postから投稿者取得)
  - [x] + 報酬フロー: ReactionRewardPoolから1 MORAL/反応をmint
  - [x] + 動的難易度調整 (on_finalize, AdjustmentWindow)

- [x] + **報酬システム**
  - [x] + Genesis: ReactionRewardPool 10,000,000 MORAL
  - [x] + 投稿コスト: 80% Storage pool, 10% Reaction pool, 10% burn
  - [x] + 固定報酬: 1 MORAL/反応 (Like のみ、Bad=0)
  - [x] + プール残高不足時: 反応は記録、報酬なし

- [x] + **クライアント側PoW**
  - [x] + miningWorker.ts: WebWorkerでBlake2bマイニング
  - [x] + useReactionMining.ts: マイニングフック
  - [x] + 難易度16ビット (faucetの18ビットより簡単)
  - [x] + チャレンジ有効期限: 100ブロック

- [x] + **フロントエンド統合**
  - [x] + ReactionButton.tsx: X風ハート/リツイート/Badボタン
  - [x] + マイニング進捗表示 (ハッシュレート, 経過時間)
  - [x] + 反応数表示 (ReactionStatsStorage from chain)
  - [x] + エラー表示 (すでに反応済み, 接続してください)
  - [x] + 投稿ごとに1回のみ反応可能

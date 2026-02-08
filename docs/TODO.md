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
  - [ ] `apps/storage-node/` - データ保持専用ノード
  - [ ] `packages/sdk/` - 共有暗号SDK
  - [ ] `packages/wasm-engine/` - Rust→Wasm

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
  - [ ] PWA設定（next-pwa）

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

### 2.1 Wasm暗号エンジン (`packages/wasm-engine/`)

- [ ] **シャミアの秘密分散 (SSS)**
  - [ ] 分割（split）関数
  - [ ] 復元（reconstruct）関数
  - [ ] しきい値設定（k-of-n）
  - [ ] Wasmエクスポート

- [ ] **ステルスアドレス生成**
  - [ ] X25519鍵交換
  - [ ] ワンタイムアドレス導出
  - [ ] スキャン鍵/閲覧鍵ペア生成
  - [ ] Wasmエクスポート

### 2.2 分散ストレージ（データ保持報酬）

> **設計方針**: バリデーター（計算と合意）とストレージノード（記憶の保持）は役割を明確に分離。
> 強力CPUはないが巨大HDD/SSDを持つユーザーも$moralを稼ぐ手段となる。

- [ ] **Storage Pallet** 作成（`apps/blockchain/pallets/storage/`）
  - [ ] 断片データメタストレージ（Share ID, サイズ, 保持者リスト）
  - [ ] 保持証明（Proof of Spacetime）検証ロジック
  - [ ] **保持報酬ロジック**
    - [ ] 保持継続ノードへの$moral分配
    - [ ] 報酬停止による「自然な忘却」メカニズム
    - [ ] 需要ベースの報酬調整（人気データ = 高報酬）
  - [ ] 不正ノードのスラッシング（持っているふりの検出）

- [ ] **ストレージノード・デーモン** (`apps/storage-node/`)
  > バリデーターとは独立した「報酬を得るための証明マシン」として機能
  - [ ] **データレセプション**: libp2pを介した断片データ（Share）の受信と保存
  - [ ] **ディスククォータ管理**: 指定容量内でのデータ管理と優先順位付け
  - [ ] **Proof of Spacetime (PoST) 生成**: 「データを持ち続けている」ことを証明する計算
  - [ ] **自動報酬請求**: 生成した証明を定期的にStorage Palletへ提出
  - [ ] **ガベージコレクション**: 報酬停止データの自動削除

- [ ] **自己修復プロトコル**
  > ストレージノードがオフライン時、自動的に断片を再配布
  - [ ] 健全性モニタリング（k-of-nのうちm個以下で警告）
  - [ ] 新規ストレージノードへの自動再分散
  - [ ] インセンティブ設計（再分散協力者に報酬）

### 2.3 PoW Faucet（アカウント初期化）

- [ ] **Faucet Pallet** 作成
  - [ ] PoWチャレンジ生成（ブロックハッシュベース）
  - [ ] nonce検証（難易度調整可能）
  - [ ] 報酬: 初期$moral（ネイティブトークン）の付与
  - [ ] レート制限（1アカウント1回のみ）

- [ ] **フロントエンド統合**
  - [ ] Web Worker でのPoW計算
  - [ ] 計算進捗表示UI
  - [ ] 自動アカウント初期化フロー

- [ ] **設計ポイント**
  - [ ] 難易度: 数秒〜数十秒で解ける程度（ボット対策）
  - [ ] アルゴリズム: SHA256 or Blake2b（ASIC耐性不要）
  - [ ] 匿名性: KYC不要、IPログなし

---

## Phase 3: 自律エコシステム

### 3.1 ステルスアドレス統合

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

### 3.3 ZKP匿名人間証明 (`packages/circuits/`)

- [ ] **Circom/Noir回路設計**
  - [ ] 「ユニークな人間である」証明
  - [ ] Nullifier生成（二重証明防止）
  - [ ] 属性非開示での検証

- [ ] オンチェーン検証
  - [ ] Groth16/PLONK検証パレット
  - [ ] 証明データ格納
  - [ ] Nullifier重複チェック

### 3.4 DM機能（Stealth Messaging）

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

### 4.1 Light Client 対応

- [ ] **smoldot 統合**
  - [ ] `polkadot-api/smoldot` 導入
  - [ ] チェーンスペック生成・配布
  - [ ] Web Worker でのsmoldot実行
  - [ ] ブートノード設定

- [ ] **接続フォールバック戦略**
  - [ ] Light Client優先 → 公開RPC フォールバック
  - [ ] 複数RPC自動切替
  - [ ] 接続状態インジケーター

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
  - [ ] Faucet（テスト用$moral配布）
  - [ ] Explorer統合

- [ ] **メインネット準備**
  - [ ] セキュリティ監査
  - [ ] Genesis設定最終化
  - [ ] バリデーター招集

---

## 構想事項（検討中）

以下は将来的に実装を検討している機能。優先度・実現可能性は未確定。

### ブラウザ拡張ウォレット連携

**背景**: 
現在の実装ではフロントエンドにシードフレーズを直接入力するため、
悪意あるフロントに秘密鍵を抜かれるリスクがある。
WebAuthnなら秘密鍵はハードウェアから出なかったが、廃止により保護が弱まった。

**解決策案**:
- Polkadot.js Extension / Talisman / SubWallet 等と連携
- シードフレーズはウォレット内に保存（フロントに渡さない）
- フロントエンドは署名リクエストのみ送信、ユーザーがウォレットUIで承認
- PAPIは `@polkadot-api/pjs-signer` で拡張ウォレットと連携可能

**暫定対応**:
- 現在のシードフレーズ入力は「開発用 / SBOM検証済みフロント専用」として運用
- 本番環境ではウォレット拡張連携を必須とする予定

**備考**: ハイドラ戦略（複数フロントエンド運営者）を維持するための前提条件

---

## 技術的依存関係

```
Phase 1.2 (Substrate) ──┬── Phase 1.3 (libp2p+Tor)
                        │
                        └── Phase 1.5 (Frontend)

Phase 2.1 (Wasm) ──────── Phase 2.2 (Storage Pallet + Storage Node)
                        │
                        └── Phase 2.3 (PoW Faucet)

Phase 3.1 (Stealth) ─── Phase 3.2 (Reaction) ─┬── Phase 3.3 (ZKP)
                                              │
                                              └── Phase 3.4 (DM)

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
| libp2p基盤 | 高 | 低 | **5** | 未着手 |
| Arti(Tor)統合 | 中 | 高 | 6 | 未着手 |
| SSS実装 | 中 | 低 | 7 | 未着手 |
| **Storage Pallet** | 高 | 中 | **8** | 未着手 |
| **ストレージノード** | 高 | 高 | **9** | 未着手 |
| ステルスアドレス | 中 | 中 | 10 | 未着手 |
| 反応マイニング | 低 | 中 | 11 | 未着手 |
| ZKP回路 | 低 | 高 | 12 | 未着手 |

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
- [ ] パブリックテストネット公開（次フェーズ）

### M5: プライバシー機能（4週間）
- SSS断片化
- ステルスアドレス

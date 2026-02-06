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
  - [x] `apps/blockchain/` - Substrate L1
  - [x] `apps/frontend/` - Next.js PWA
  - [ ] `packages/sdk/` - 共有暗号SDK
  - [ ] `packages/wasm-engine/` - Rust→Wasm

- [ ] CI/CD パイプライン
  - [ ] Rust テスト・ビルド
  - [ ] TypeScript lint・テスト
  - [ ] Wasm ビルド自動化

### 1.2 Substrate L1 Core (`apps/blockchain/`)

- [x] Substrate ノードテンプレート初期化 (Polkadot SDK stable2503)

- [ ] **Identity Pallet** 作成
  - [ ] WebAuthn公開鍵の登録ストレージ
  - [ ] マルチデバイス対応（1 Identity → N Passkeys）
  - [ ] 公開鍵の追加/削除エクストリンシック

- [x] **Moral Token Pallet** 作成
  - [x] トークン発行（mint）ロジック
  - [x] トークン焼却（burn）ロジック
  - [x] 残高管理ストレージ
  - [x] 転送エクストリンシック
  - [x] Genesis設定でテストアカウントにMoral配布

- [x] **Post Pallet** 作成
  - [x] 投稿データ構造定義
  - [x] 投稿ストレージ（Posts, Contents, UserPosts）
  - [x] 投稿作成エクストリンシック
  - [x] 投稿コスト（$moral）の検証
  - [x] **動的コスト計算（byte数ベース）**
    - PostBaseCost = 10 MORAL（基本料金）
    - PostByteCost = 0.1 MORAL/byte（バイト単価）

### 1.3 libp2p + Tor 統合

- [ ] libp2p ネットワーク層実装
  - [ ] ノード識別（PeerId）
  - [ ] Kademliaによるピア発見
  - [ ] GossipSubによるメッセージ伝播

- [ ] **Arti（Tor）統合**
  - [ ] `arti-client` クレート導入
  - [ ] Torトランスポートラッパー実装
  - [ ] libp2p Transport として統合
  - [ ] Onion Service対応（オプション）

- [ ] ネットワーク設定
  - [ ] Tor強制モード / 通常モード切替
  - [ ] ブートストラップノード設定

### 1.4 WebAuthn 署名検証

- [ ] **Rust署名検証ライブラリ** (`packages/sdk/`)
  - [ ] COSE公開鍵パーサー
  - [ ] ES256 (P-256) 署名検証
  - [ ] authenticatorData パース
  - [ ] clientDataJSON 検証

- [ ] **Substrate統合**
  - [ ] `sp-io` カスタムホスト関数（オプション）
  - [ ] オンチェーンWebAuthn検証ロジック
  - [ ] WYSIWYS: challengeに投稿ハッシュ埋め込み

### 1.5 フロントエンド MVP (`apps/frontend/`)

- [x] Next.js プロジェクト初期化
  - [x] TypeScript設定
  - [ ] PWA設定（next-pwa）

- [ ] WebAuthn統合
  - [ ] パスキー登録フロー
  - [ ] パスキー認証フロー
  - [ ] 署名リクエスト（投稿時）

- [x] 基本UI
  - [x] タイムライン表示
  - [x] 投稿フォーム（動的コスト表示付き）
  - [x] ウォレット残高表示
  - [x] PAPI (polkadot-api) によるチェーン接続
  - [ ] Runtime constantsからのコスト設定取得（フォールバック対応済み）

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

### 2.2 分散ストレージ

- [ ] **Storage Pallet** 作成
  - [ ] 断片データストレージ
  - [ ] 保持証明（Proof of Storage）
  - [ ] 報酬分配ロジック

- [ ] ノード側ストレージ実装
  - [ ] ローカル断片保存
  - [ ] 定期的な保持証明提出
  - [ ] ガベージコレクション（報酬停止時）

### 2.3 ステルスアドレス統合

- [ ] **Stealth Pallet** 作成
  - [ ] ステルスアドレス宛トランザクション
  - [ ] エフェメラル公開鍵の格納

- [ ] クライアント側スキャナー
  - [ ] バックグラウンドスキャン処理
  - [ ] 自分宛トランザクション検出
  - [ ] 復号・残高更新

---

## Phase 3: 自律エコシステム

### 3.1 反応マイニング

- [ ] **Reaction Pallet** 作成
  - [ ] 反応（いいね等）データ構造
  - [ ] PoW難易度検証
  - [ ] 報酬計算: `Reward = Σ(Reaction × Power_cpu) × γ`

- [ ] クライアント側PoW
  - [ ] WebWorkerでのマイニング実行
  - [ ] Page Visibility API制御
  - [ ] 難易度調整パラメータ取得

- [ ] 動的難易度調整
  - [ ] ネットワーク全体の反応レート監視
  - [ ] 難易度自動調整アルゴリズム

### 3.2 ZKP匿名人間証明 (`packages/circuits/`)

- [ ] **Circom/Noir回路設計**
  - [ ] 「ユニークな人間である」証明
  - [ ] Nullifier生成（二重証明防止）
  - [ ] 属性非開示での検証

- [ ] オンチェーン検証
  - [ ] Groth16/PLONK検証パレット
  - [ ] 証明データ格納
  - [ ] Nullifier重複チェック

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

## 技術的依存関係

```
Phase 1.2 (Substrate) ──┬── Phase 1.3 (libp2p+Tor)
                        │
                        └── Phase 1.4 (WebAuthn) ── Phase 1.5 (Frontend)

Phase 2.1 (Wasm) ────────── Phase 2.2 (Storage)
                        │
                        └── Phase 2.3 (Stealth)

Phase 3.1 (Reaction) ─┬── Phase 3.2 (ZKP)
                      │
                      └── Phase 3.3 (DM)
```

---

## 優先度マトリクス

| タスク | 重要度 | 難易度 | 優先順位 | 状態 |
|--------|--------|--------|----------|------|
| Substrate基盤 | 高 | 中 | **1** | ✅完了 |
| Moral Pallet | 高 | 低 | **1.5** | ✅完了 |
| Post Pallet | 高 | 中 | **1.5** | ✅完了 |
| フロントMVP | 高 | 低 | **2** | ✅完了 |
| Identity Pallet | 高 | 中 | **3** | 未着手 |
| WebAuthn検証 | 高 | 中 | **4** | 未着手 |
| libp2p基盤 | 高 | 低 | **5** | 未着手 |
| Arti(Tor)統合 | 中 | 高 | 6 | 未着手 |
| SSS実装 | 中 | 低 | 7 | 未着手 |
| ステルスアドレス | 中 | 中 | 8 | 未着手 |
| 反応マイニング | 低 | 中 | 9 | 未着手 |
| ZKP回路 | 低 | 高 | 10 | 未着手 |

---

## マイルストーン

### M1: 動作するローカルネット ✅完了
- Substrateノード起動
- 基本的なトークン転送（Moral）
- シンプルな投稿機能
- **追加達成**: 動的投稿コスト、Genesis設定

### M2: 認証統合（2週間）
- WebAuthn署名検証
- フロントエンドでのパスキー登録/認証

### M3: P2Pネットワーク（2週間）
- 複数ノード間の通信確立
- GossipSubによる投稿伝播

### M4: Tor統合（2週間）
- Artiによる匿名通信
- テストネット公開

### M5: プライバシー機能（4週間）
- SSS断片化
- ステルスアドレス

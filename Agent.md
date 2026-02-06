# Anarchy - AIエージェント開発ガイド

> **支配なき秩序（Order without Masters）**
> 中央集権的な管理者を介さず,数学的・経済的メカニズムによってユーザーの言論の自由を保護するL1ブロックチェーンベースの分散型SNSプロトコル.

---

## 1. プロジェクト概要

| 項目 | 内容 |
|------|------|
| **名称** | Anarchy |
| **種別** | L1ブロックチェーン + 分散型SNSプロトコル |
| **コアバリュー** | 完全匿名性, 検閲耐性, 自律経済 |
| **独自トークン** | $moral（投稿コスト・報酬・ガバナンス） |

### 設計思想

- **ゼロトラスト・ソーシャル**: 悪意あるフロントエンド（ハイドラ）の存在を許容しつつ,プロトコル層で数学的に無効化
- **クレンジング・パラダイム**: フロントエンドで情報が漏れることを前提とし,それをプロトコル層で切断
- **経済的忘却**: 需要のないデータは報酬停止により自然消滅,需要のあるデータのみ生存

---

## 2. セキュリティ・アノニミティ原則（妥協不可）

以下の原則は**絶対に破ってはならない**.

| # | 原則 | 詳細 |
|---|------|------|
| 1 | **ネットワーク秘匿** | libp2pトランスポート層にTor/I2Pを強制統合し,IPアドレス等のメタデータを物理的に遮断 |
| 2 | **秘密鍵の排除** | ユーザーに秘密鍵（シードフレーズ）を扱わせない. WebAuthn + AAでSecure Enclave署名を前提 |
| 3 | **クライアントサイド完結** | 暗号化,断片化（SSS）,メタデータ削除は必ずクライアント側で実行してから送信 |
| 4 | **フォアグラウンド処理** | 反応マイニング（PoW）はPage Visibility API等で制御し,ユーザー可視範囲で実行 |

---

## 3. 技術スタック

### ブロックチェーン（L1 Core）

```
Rust + Polkadot SDK (stable2503)
├── Runtime: FRAME pallets
├── Consensus: Aura (PoA for dev) → NPoS (production)
└── Networking: libp2p + Tor/I2P
```

### フロントエンド

```
Next.js 15 + TypeScript + PWA
├── API: PAPI (polkadot-api) ※@polkadot/apiは非推奨
├── Keys: @polkadot-labs/hdkd
└── Crypto: WebAssembly (Rust-Wasm)
```

### 暗号技術

| 用途 | 技術 |
|------|------|
| ZKP | Circom / Noir |
| 秘密分散 | Shamir's Secret Sharing (SSS) |
| ステルスアドレス | X25519 + ECDH |
| 署名検証 | schnorrkel / dalek |

---

## 4. ディレクトリ構造

```
anarchy/
├── apps/
│   ├── blockchain/           # Substrate L1 Core
│   │   ├── node/             # ノード実装
│   │   ├── runtime/          # ランタイム（パレット統合）
│   │   ├── pallets/          # カスタムパレット
│   │   │   ├── post/         # 投稿管理
│   │   │   └── moral/        # $moralトークン
│   │   └── tests/
│   │       └── integration/  # 統合テスト
│   │           ├── utils.sh
│   │           ├── test_block_sync.sh
│   │           ├── test_consensus.sh
│   │           ├── test_invalid_data.sh
│   │           ├── test_node_recovery.sh
│   │           └── test_scalability.sh
│   └── frontend/             # Next.js PWA
│       ├── src/
│       │   ├── app/          # App Router
│       │   ├── components/   # UIコンポーネント
│       │   └── hooks/        # カスタムフック（useApi等）
│       └── package.json
├── scripts/
│   └── run-multi-node.sh     # マルチノードテストネット起動
├── packages/                  # 共有パッケージ（将来）
│   ├── circuits/             # ZKP回路
│   ├── sdk/                  # 暗号SDKs
│   └── wasm-engine/          # Rust-Wasm実装
├── docs/                     # ドキュメント
└── pnpm-workspace.yaml       # モノレポ設定
```

---

## 5. 開発ワークフロー

### ブロックチェーン開発

```bash
# ビルド
cd apps/blockchain
cargo build --release

# テスト
cargo test --all

# ノード起動（開発モード）
./target/release/anarchy-node --dev

# ノードは ws://127.0.0.1:9944 でリッスン
```

### フロントエンド開発

```bash
# 依存関係インストール
cd apps/frontend
pnpm install

# 開発サーバー起動
pnpm dev  # http://localhost:3000

# ビルド
pnpm build
```

### マルチノードテストネット

```bash
# 3ノードテストネットを起動（Alice + Bob: バリデータ, Charlie: フルノード）
./scripts/run-multi-node.sh

# 任意のノード数で起動（最大10）
./scripts/run-multi-node.sh 5

# 停止
pkill -f anarchy-node
```

### 統合テスト

```bash
# 全テスト実行
pnpm test:integration

# 個別テスト
pnpm test:sync          # ブロック同期テスト
pnpm test:consensus     # コンセンサス/フォーク解決テスト
pnpm test:invalid       # 不正データ拒否テスト
pnpm test:recovery      # ノードリカバリテスト
pnpm test:scalability   # スケーラビリティテスト（10ノード）
```

### 重要: PAPI使用時の注意点

Polkadot SDK stable2503はメタデータv16を使用するため,**@polkadot/apiは使用不可**.
代わりに**PAPI (polkadot-api)**を使用すること.

```typescript
// ✅ 正しい: PAPI
import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'

const client = createClient(getWsProvider('ws://127.0.0.1:9944'))
const api = client.getUnsafeApi()

// ストレージ読み取り（getEntriesを使用）
const entries = await api.query.Post.Posts.getEntries()

// トランザクション送信
const tx = api.tx.Post.create_post({ content, parent_id: undefined })
await tx.signAndSubmit(signer)

// ❌ 間違い: @polkadot/api（署名エラーになる）
// import { ApiPromise } from '@polkadot/api'
```

---

## 6. パレット仕様

### Post パレット

| ストレージ | 型 | 説明 |
|-----------|-----|------|
| `Posts` | `StorageMap<u64, Post<T>>` | 投稿データ |
| `NextPostId` | `StorageValue<u64>` | 次の投稿ID |

```rust
pub struct Post<T: Config> {
    pub author: T::AccountId,
    pub content_hash: [u8; 32],
    pub created_at: BlockNumberFor<T>,
    pub parent_id: Option<u64>,
}
```

| Extrinsic | パラメータ | 説明 |
|-----------|-----------|------|
| `create_post` | `content: Vec<u8>, parent_id: Option<u64>` | 新規投稿作成 |

### Moral パレット

| ストレージ | 型 | 説明 |
|-----------|-----|------|
| `TotalSupply` | `StorageValue<u128>` | 総発行量 |
| `Balances` | `StorageMap<AccountId, u128>` | 残高 |

| Extrinsic | パラメータ | 説明 |
|-----------|-----------|------|
| `transfer` | `to: AccountId, amount: u128` | 送金 |
| `mint` | `to: AccountId, amount: u128` | 発行（sudo） |
| `burn` | `amount: u128` | 焼却 |

---

## 7. コーディング規約

### 言語

- **コード**: Rust / TypeScript
- **コメント**: 日本語または英語（論理的かつ明確に）
- **ドキュメント**: 日本語優先

### Rust スタイル

```rust
// パレット内のエラー定義
#[pallet::error]
pub enum Error<T> {
    /// 投稿が存在しない
    PostNotFound,
    /// 残高不足
    InsufficientBalance,
}

// イベント定義
#[pallet::event]
#[pallet::generate_deposit(pub(super) fn deposit_event)]
pub enum Event<T: Config> {
    /// 投稿が作成された [投稿者, 投稿ID]
    PostCreated { author: T::AccountId, post_id: u64 },
}
```

### TypeScript スタイル

```typescript
// 型安全なAPI呼び出し
interface Post {
  id: number
  author: string
  contentHash: string
  createdAt: number
  parentId: number | null
}

// エラーハンドリング必須
try {
  const entries = await api.query.Post.Posts.getEntries()
} catch (err) {
  console.error('投稿の取得に失敗:', err)
}
```

---

## 8. 実装ロードマップ

### Phase 1: セキュア・ファンデーション（現在）

- [x] Substrateノード基盤構築
- [x] Post / Moral パレット実装
- [x] Next.js + PAPI フロントエンド
- [x] オンチェーンコンテンツ保存（投稿本文をチェーンに記録）
- [x] Moralトークン消費（投稿時に100 moral消費）
- [x] マルチノードテストネット（3ノード構成）
- [x] 統合テストフレームワーク（ブロック同期、コンセンサス、リカバリ、スケーラビリティ）
- [ ] libp2p + Tor統合
- [ ] WebAuthn署名検証

### Phase 2: プライバシー・レイヤー

- [ ] SSS によるデータ断片化
- [ ] ステルスアドレス（DM機能）
- [ ] 分散ストレージ報酬

### Phase 3: 自律エコシステム

- [ ] 反応マイニング（PoW）
- [ ] 動的難易度調整
- [ ] ZKP匿名人間証明

---

## 9. トラブルシューティング

### よくある問題

| 問題 | 原因 | 解決策 |
|------|------|--------|
| `Invalid Transaction: Transaction has a bad signature` | @polkadot/api使用 | PAPIに移行 |
| `Incompatible runtime entry Storage(...)` | `getValue()`の型不一致 | `getEntries()`を使用 |
| ノード接続失敗 | ノード未起動 / ポート競合 | ノード起動確認, `lsof -i:9944` |
| フロントエンドビルドエラー | 依存関係不整合 | `rm -rf node_modules && pnpm install` |
| 統合テスト失敗 | 前回のノードプロセス残存 | `pkill -f anarchy-node` |
| テストでタイムアウト | ノード起動待ち不足 | `wait_for_node`の待ち時間を延長 |

### デバッグコマンド

```bash
# ノードログ確認
journalctl -f -u anarchy-node

# ブロック生成確認
curl -s localhost:9944 -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"chain_getHeader"}' | jq

# ピア接続確認
curl -s localhost:9944 -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"system_peers"}' | jq

# フロントエンドからの接続テスト
node -e "
const { createClient } = require('polkadot-api');
const { getWsProvider } = require('polkadot-api/ws-provider/node');
const client = createClient(getWsProvider('ws://127.0.0.1:9944'));
client.getUnsafeApi().query.Post.NextPostId.getValue().then(console.log);
"

# 残存ノードプロセス確認・停止
ps aux | grep anarchy-node
pkill -f anarchy-node
```

---

## 10. 参考リンク

- [Polkadot SDK Documentation](https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/index.html)
- [PAPI (polkadot-api)](https://papi.how/)
- [Substrate Tutorials](https://docs.substrate.io/tutorials/)
- [libp2p](https://libp2p.io/)

---

**Last Updated**: 2026-02-07
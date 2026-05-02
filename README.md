# Anarchy

**支配なき秩序** — 中央集権を排除した匿名分散型SNSプロトコル

## プロジェクト構成

```
anarchy/
├── apps/
│   ├── blockchain/     # Substrate L1ブロックチェーン
│   │   ├── node/       # ノード実行ファイル
│   │   ├── runtime/    # ランタイム
│   │   └── pallets/    # カスタムパレット
│   │       ├── post/   # 投稿機能 (create_post_v2: MerkleRoot記録)
│   │       └── faucet/ # Faucetパレット
│   ├── storage-node/   # 分散ストレージノード (libp2p)
│   └── frontend/       # Next.js フロントエンド
├── packages/
│   └── wasm-engine/    # Wasm暗号エンジン (SSS + MerkleTree)
├── docs/               # ドキュメント
└── specs/              # 機能仕様書
```

## 必要環境

### ブロックチェーン / Storage Node
- Rust 1.74+

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32v1-none
rustup component add rust-src
```

### フロントエンド
- Node.js 18+
- pnpm

```bash
npm install -g pnpm
```

## 起動方法

### 一括起動 (推奨)

testnet (3-node) + storage (5-node) + frontend を依存関係順に立ち上げて、frontend が応答するまで待つラッパー。

```bash
pnpm stack:start         # 全部起動
pnpm stack:status        # 各層の稼働状況
pnpm stack:stop          # 逆順で全停止
pnpm stack:restart       # stop → start
pnpm stack:purge         # stop + 全データ消去 (.next/ も含む)
```

オプション:

```bash
./scripts/dev-stack.sh start --single-node   # 3-node ではなく `cargo run -- --dev` 単一ノード
```

ログ:
- frontend: `.dev-stack/frontend.log`
- single-node: `.dev-stack/single-node.log`
- testnet/storage: 既存スクリプト準拠 (`apps/{blockchain,storage-node}/logs/`)

個別管理が必要なときは下記の手順を参照。

### 1. ブロックチェーンノードの起動

```bash
cd apps/blockchain

# ビルド（初回のみ）
cargo build --release

# 起動（デフォルト3ノード: Alice/Bob=Validator, Charlie=Full）
./scripts/run-multi-node.sh start

# ノード数を指定（最大10）
./scripts/run-multi-node.sh start 5

# 停止・ステータス・削除
./scripts/run-multi-node.sh stop
./scripts/run-multi-node.sh status
./scripts/run-multi-node.sh purge
```

| ノード | 役割 | RPC | P2P |
|--------|------|-----|-----|
| Alice | Validator | ws://127.0.0.1:9944 | 30333 |
| Bob | Validator | ws://127.0.0.1:9945 | 30334 |
| Charlie〜 | Full Node | :9946〜 | 30335〜 |

### 2. Storage Nodeの起動

```bash
cd apps/storage-node
cargo build --release

# 起動（デフォルト5ノード）
./scripts/run-storage-nodes.sh start
./scripts/run-storage-nodes.sh start 3   # ノード数指定

# 停止・ステータス・削除
./scripts/run-storage-nodes.sh stop
./scripts/run-storage-nodes.sh status
./scripts/run-storage-nodes.sh purge
```

📖 詳細: [apps/storage-node/README.md](apps/storage-node/README.md)

### 3. Wasm暗号エンジンのビルド

```bash
cd packages/wasm-engine

# wasm-packのインストール（初回のみ）
cargo install wasm-pack

# Wasmビルド
wasm-pack build --target web --out-dir pkg
```

生成物は `packages/wasm-engine/pkg/` に配置されます。

### 4. フロントエンドの起動

```bash
# 依存関係のインストール
pnpm install

# 開発サーバーの起動
pnpm dev:frontend
```

ブラウザで http://localhost:3000 を開きます。

## 開発コマンド

```bash
# 一括 (testnet + storage + frontend)
pnpm stack:start         # 全部起動 (依存順)
pnpm stack:stop          # 全部停止 (逆順)
pnpm stack:status        # 各層の稼働状況
pnpm stack:restart       # 再起動
pnpm stack:purge         # stop + データ全消去

# ブロックチェーン
pnpm build:blockchain    # ビルド
pnpm dev:node           # 開発ノード起動

# テストネット（3ノード）
pnpm testnet:start      # テストネット起動
pnpm testnet:stop       # テストネット停止
pnpm testnet:status     # ステータス確認
pnpm testnet:logs       # ログ表示
pnpm testnet:purge      # データ削除

# Storage Node
pnpm storage:start      # 5ノード起動
pnpm storage:stop       # 全停止
pnpm storage:status     # ステータス確認
pnpm storage:purge      # データ削除

# フロントエンド
pnpm dev:frontend       # 開発サーバー
pnpm build:frontend     # 本番ビルド

# 統合テスト
pnpm test:integration   # 全テスト実行
pnpm test:sync          # ブロック同期テスト
pnpm test:consensus     # コンセンサス/フォーク解決テスト
pnpm test:invalid       # 不正データ拒否テスト
pnpm test:recovery      # ノードリカバリテスト
pnpm test:scalability   # スケーラビリティテスト（10ノード）
```

## $moral トークンのMint（開発用）

開発環境でテスト用の$moralトークンをmintするスクリプトです。

### 方法1: Sudo mint（推奨）

Sudo権限で直接残高を設定します。Aliceに残高がなくても実行可能です。

```bash
# テストアカウント名でmint（Alice, Bob, Charlie, Dave, Eve, Ferdie）
node scripts/sudo-mint.mjs Alice 1000000
node scripts/sudo-mint.mjs Bob 500000

# アドレス指定でmint
node scripts/sudo-mint.mjs 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY 10000
```

### 方法2: 転送（Aliceに残高がある場合）

Aliceから他のアカウントに転送します。

```bash
# テストアカウント名でmint
node scripts/mint-moral.mjs Bob 5000

# アドレス指定でmint
node scripts/mint-moral.mjs 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY 10000

# シードフレーズから導出したアドレスにmint
node scripts/mint-moral-seed.mjs "word1 word2 word3 ... word12" 10000

# 開発用シードフレーズ（DEV_PHRASE）を使用
node scripts/mint-moral-seed.mjs dev 10000
```

> **Note**: トランザクション手数料は0に設定されています。$moralの投稿コストでスパム対策を行います。

## アーキテクチャ

### 5層構造

1. **ネットワーク層**: libp2p + Tor/I2P（ノード間通信の匿名化）
2. **アイデンティティ層**: シードフレーズベースのAccountId認証
3. **コンセンサス層**: Substrate + 反応マイニング
4. **ストレージ層**: シャミアの秘密分散（SSS）
5. **インターフェース層**: ハイドラ戦略（複数のフロントエンド）

### クレンジング・パラダイム

フロントエンド（ハイドラ）へのIP露出は許容し、**プロトコル層で匿名性を数学的に保証**する設計。

## 統合テスト

ブロックチェーンの正当性を検証するテストスイート:

| テスト | 内容 |
|--------|------|
| ブロック同期 | 新規ノードがチェーンを同期、GRANDPA finality |
| コンセンサス | ネットワーク分断・復旧、ファイナリティ |
| 不正データ拒否 | ランダムバイト、壊れた署名の拒否 |
| ノードリカバリ | クラッシュ後のデータ復旧、履歴アクセス |
| スケーラビリティ | 10ノード協調、ピア伝播、障害耐性 |

- ブラウザ環境: 通常HTTP/S（Torなし）
- ノード間通信: libp2p + Tor over Arti
- 匿名性: ステルスアドレス、ZKP

## Tor匿名モード（オプション）

ノード間通信をTorネットワーク経由で匿名化できます。

```bash
cd apps/blockchain

# Tor/torsocksインストール
./scripts/tor-setup.sh install

# 匿名モードでノード起動（Onion Service設定済みの場合）
./scripts/anarchy-tor.sh ./target/release/anarchy-node \
  --tor-mode=forced \
  --public-addr=/onion3/YOUR_ONION_ADDRESS:30333
```

| モード | 説明 |
|--------|------|
| `off` | 通常接続（開発用） |
| `outbound-only` | 送信のみTor（受信IP露出） |
| `forced` | 完全匿名（**本番推奨**） |

> ⚠️ mainnetでは `--tor-mode=forced` が自動強制されます

📖 詳細: [apps/blockchain/docs/tor-deployment.md](apps/blockchain/docs/tor-deployment.md)

## ライセンス

MIT License
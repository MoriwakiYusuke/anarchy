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
│   │       ├── post/   # 投稿機能
│   │       └── moral/  # $moralトークン
│   └── frontend/       # Next.js フロントエンド
├── docs/               # ドキュメント
└── packages/           # 共有パッケージ（今後追加）
```

## 必要環境

### ブロックチェーン
- Rust 1.74+
- Cargo

```bash
# Rustのインストール
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Wasmターゲットの追加
rustup target add wasm32-unknown-unknown

# 必要なツール
rustup component add rust-src
```

### フロントエンド
- Node.js 18+
- pnpm

```bash
# pnpmのインストール
npm install -g pnpm
```

## 起動方法

### 1. ブロックチェーンノードの起動

```bash
cd apps/blockchain

# ビルド（初回のみ、時間がかかります）
cargo build --release

# 開発モードで起動
./target/release/anarchy-node --dev
```

ノードが起動すると `ws://127.0.0.1:9944` でWebSocket接続を受け付けます。

### 2. フロントエンドの起動

```bash
# 依存関係のインストール
pnpm install

# 開発サーバーの起動
pnpm dev:frontend
```

ブラウザで http://localhost:3000 を開きます。

## 開発コマンド

```bash
# ブロックチェーン
pnpm build:blockchain    # ビルド
pnpm dev:node           # 開発ノード起動

# フロントエンド
pnpm dev:frontend       # 開発サーバー
pnpm build:frontend     # 本番ビルド
```

## アーキテクチャ

### 5層構造

1. **ネットワーク層**: libp2p + Tor/I2P（ノード間通信の匿名化）
2. **アイデンティティ層**: WebAuthn + ZKP（パスキー認証）
3. **コンセンサス層**: Substrate + 反応マイニング
4. **ストレージ層**: シャミアの秘密分散（SSS）
5. **インターフェース層**: ハイドラ戦略（複数のフロントエンド）

### クレンジング・パラダイム

フロントエンド（ハイドラ）へのIP露出は許容し、**プロトコル層で匿名性を数学的に保証**する設計。

- ブラウザ環境: 通常HTTP/S（Torなし）
- ノード間通信: libp2p + Tor over Arti
- 匿名性: ステルスアドレス、ZKP

## ライセンス

MIT License

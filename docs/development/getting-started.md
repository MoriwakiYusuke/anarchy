# Getting Started

Anarchy をローカルで動かすための完全ガイドです。一括起動だけ知りたい場合は [リポジトリルートの README](../../README.md#quick-start) を参照してください。

## 1. 必要環境

### ブロックチェーン / Storage Node (Rust)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32v1-none
rustup component add rust-src
```

`apps/blockchain/rust-toolchain.toml` で stable チャンネル + `wasm32v1-none` + `rust-src` が固定されています。

### フロントエンド (Node.js)

```bash
# Node.js 18+
npm install -g pnpm
```

### Wasm Engine

```bash
cargo install wasm-pack
```

---

## 2. 一括起動 (推奨)

`scripts/dev-stack.sh` が testnet (3-node) + storage (5-node) + frontend を依存順に立ち上げます。

```bash
pnpm stack:start         # 全部起動
pnpm stack:status        # 稼働状況
pnpm stack:stop          # 逆順で停止
pnpm stack:restart       # stop → start
pnpm stack:purge         # stop + 全データ消去 (.next/ も含む)
```

オプション:

```bash
./scripts/dev-stack.sh start --single-node   # cargo run -- --dev 単一ノード
```

ログ:

- frontend: `.dev-stack/frontend.log`
- single-node: `.dev-stack/single-node.log`
- testnet / storage: `apps/{blockchain,storage-node}/logs/`

---

## 3. 個別起動

### 3.1 ブロックチェーンノード

```bash
cd apps/blockchain
cargo build --release

# デフォルト 3 ノード (Alice/Bob = Validator, Charlie = Full)
./scripts/run-multi-node.sh start

# ノード数指定 (最大 10)
./scripts/run-multi-node.sh start 5

./scripts/run-multi-node.sh stop
./scripts/run-multi-node.sh status
./scripts/run-multi-node.sh purge
```

| ノード | 役割 | RPC | P2P |
|---|---|---|---|
| Alice | Validator | `ws://127.0.0.1:9944` | 30333 |
| Bob | Validator | `ws://127.0.0.1:9945` | 30334 |
| Charlie〜 | Full Node | `:9946〜` | 30335〜 |

### 3.2 Storage Node

```bash
cd apps/storage-node
cargo build --release

./scripts/run-storage-nodes.sh start       # 5 ノード起動
./scripts/run-storage-nodes.sh start 3     # 3 ノード起動

./scripts/run-storage-nodes.sh stop
./scripts/run-storage-nodes.sh status
./scripts/run-storage-nodes.sh purge
```

詳細は [apps/storage-node/README.md](../../apps/storage-node/README.md)。

### 3.3 Wasm 暗号エンジン

```bash
cd packages/wasm-engine
wasm-pack build --target web --out-dir pkg
```

生成物は `packages/wasm-engine/pkg/` に配置され、フロントエンドからファイル依存として参照されます。

### 3.4 フロントエンド

```bash
pnpm install
pnpm dev:frontend
```

ブラウザで <http://localhost:3000> を開きます。

---

## 4. $MORAL トークン Mint (開発用)

### 方法 1: Sudo mint (推奨)

Sudo 権限で直接残高を設定します。Alice に残高がなくても実行可能。

```bash
# 開発用テストアカウント名で mint
node scripts/sudo-mint.mjs Alice 1000000
node scripts/sudo-mint.mjs Bob 500000

# アドレス指定で mint
node scripts/sudo-mint.mjs 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY 10000
```

### 方法 2: 転送 (Alice に残高がある場合)

```bash
node scripts/mint-moral.mjs Bob 5000
node scripts/mint-moral.mjs 5Grwv... 10000
node scripts/mint-moral-seed.mjs "word1 word2 ... word12" 10000
node scripts/mint-moral-seed.mjs dev 10000     # DEV_PHRASE 使用
```

> トランザクション手数料は 0 です。$MORAL の投稿コストでスパム対策しています。

---

## 5. Tor 匿名モード (オプション)

ノード間通信を Tor で匿名化します。

```bash
cd apps/blockchain

# Tor / torsocks インストール
./scripts/tor-setup.sh install

# 匿名モードで起動 (Onion Service 設定済みの場合)
./scripts/anarchy-tor.sh ./target/release/anarchy-node \
  --tor-mode=forced \
  --public-addr=/onion3/YOUR_ONION_ADDRESS:30333
```

| モード | 説明 |
|---|---|
| `off` | 通常接続 (開発用) |
| `outbound-only` | 送信のみ Tor (受信 IP 露出) |
| `forced` | 完全匿名 (**本番推奨**) |

> mainnet では `--tor-mode=forced` が自動強制されます

詳細: [apps/blockchain/docs/tor-deployment.md](../../apps/blockchain/docs/tor-deployment.md)

---

## 次のステップ

- 全コマンド一覧: [commands.md](commands.md)
- アーキテクチャ全体像: [../architecture/overview.md](../architecture/overview.md)
- 進行中タスク: [todo.md](todo.md)

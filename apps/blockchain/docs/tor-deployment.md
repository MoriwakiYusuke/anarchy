# Tor Deployment Guide for Anarchy Nodes

Anarchyノードを Tor 経由で匿名運用するためのガイドです。

## 目次

1. [クイックスタート: 完全な Tor セットアップ](#クイックスタート-完全な-tor-セットアップ)
2. [概要](#概要)
3. [前提条件](#前提条件)
4. [Phase 1: 送信の匿名化 (torsocks)](#phase-1-送信の匿名化-torsocks)
5. [Phase 2: 受信の匿名化 (Onion Service)](#phase-2-受信の匿名化-onion-service)
6. [Tor モード](#tor-モード)
7. [ブートストラップノード](#ブートストラップノード)
8. [トラブルシューティング](#トラブルシューティング)

---

## クイックスタート: 完全な Tor セットアップ

P2P と RPC の両方を Tor 経由でアクセスできる、完全匿名な Anarchy ノードをセットアップする手順です。

### ステップ 1: Tor と torsocks のインストール

```bash
# Ubuntu/Debian
sudo apt update && sudo apt install -y tor torsocks

# macOS
brew install tor torsocks

# Arch Linux
sudo pacman -S tor torsocks

# バージョン確認
tor --version   # 0.4.x 以上
torsocks --version  # 2.3.0 以上
```

### ステップ 2: Tor サービスの起動

```bash
# Tor デーモンを起動・有効化
sudo systemctl start tor
sudo systemctl enable tor

# Tor が動作していることを確認
curl --socks5 localhost:9050 https://check.torproject.org/api/ip
# 戻り値: {"IsTor":true,"IP":"xxx.xxx.xxx.xxx"}
```

### ステップ 3: Onion Service の設定 (P2P + RPC)

専用の torrc 設定ファイルを作成します：

```bash
# 設定ディレクトリを作成（存在しない場合）
sudo mkdir -p /etc/tor/torrc.d

# Anarchy 専用の設定を作成
sudo tee /etc/tor/torrc.d/anarchy.conf << 'EOF'
# Anarchy Node Onion Service
HiddenServiceDir /var/lib/tor/anarchy-node/
HiddenServicePort 30333 127.0.0.1:30333
HiddenServicePort 9944 127.0.0.1:9944
EOF

# torrc が設定ディレクトリを include していることを確認
# 以下の行が /etc/tor/torrc に存在しない場合は追加:
# %include /etc/tor/torrc.d/*.conf
sudo grep -q "^%include /etc/tor/torrc.d" /etc/tor/torrc || \
  echo "%include /etc/tor/torrc.d/*.conf" | sudo tee -a /etc/tor/torrc

# 設定を反映するために Tor をリロード
sudo systemctl reload tor
```

### ステップ 4: Onion アドレスの取得

```bash
# Onion Service の初期化を数秒待つ
sleep 5

# .onion アドレスを取得
sudo cat /var/lib/tor/anarchy-node/hostname
# 出力例: zjnzfe3rv3yhwrxt6vwu6yeq3xi3kqxvepjfysaj2j7plysduuucvcqd.onion
```

このアドレスを保存してください。これがノードの匿名IDになります。

### ステップ 5: ノードのビルドと起動

```bash
cd apps/blockchain

# ノードをビルド（まだの場合）
cargo build --release

# 完全な Tor 匿名モードで起動
./scripts/anarchy-tor.sh ./target/release/anarchy-node \
  --chain local \
  --alice \
  --tor-mode=forced \
  --rpc-cors=all \
  --rpc-external \
  --public-addr=/onion3/YOUR_ONION_ADDRESS:30333

# YOUR_ONION_ADDRESS は実際の .onion アドレス（.onion サフィックスなし）に置き換え
```

### ステップ 6: Tor 接続の検証

**P2P 接続テスト（Onion 経由）:**
```bash
# 別マシンから Tor 経由で接続テスト
ONION="zjnzfe3rv3yhwrxt6vwu6yeq3xi3kqxvepjfysaj2j7plysduuucvcqd.onion"
torsocks nc -zv $ONION 30333
```

**RPC 接続テスト（Onion 経由）:**
```bash
# Tor SOCKS プロキシ経由で HTTP RPC
curl --socks5-hostname 127.0.0.1:9050 \
  "http://${ONION}:9944" \
  -X POST -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"system_health"}'
# 戻り値: {"jsonrpc":"2.0","result":{"peers":X,"isSyncing":false,...},"id":1}
```

**トランザクション送信テスト（Onion 経由）:**
```bash
# Tor 経由で送金（リポジトリルートから実行）
WS_ENDPOINT="ws://${ONION}:9944" torsocks node scripts/transfer-native.mjs Bob 10
# 出力: 送金成功！ブロック: #XXX
```

### ステップ 7: 統合テストの実行

```bash
cd apps/blockchain
./tests/integration/tor_connectivity_test.sh
# 期待結果: 18 passed, 0 failed, 0 skipped
```

### クイックリファレンス

| サービス | ローカルポート | Onion ポート | プロトコル |
|---------|--------------|-------------|----------|
| P2P | 30333 | 30333 | TCP (libp2p) |
| RPC | 9944 | 9944 | HTTP/WebSocket |

| 環境変数 | 説明 |
|---------|------|
| `ANARCHY_TOR_MODE` | Tor モードを設定 (off/outbound-only/forced) |
| `ANARCHY_RUNNING_UNDER_TORSOCKS` | anarchy-tor.sh が torsocks ラッパーを示すために設定 |

---

## 概要

Anarchy ノードは3段階の Tor 統合をサポートしています：

| モード | 送信 | 受信 | 用途 |
|--------|------|------|------|
| `off` | 直接 IP | 直接 IP | 開発専用 |
| `outbound-only` | Tor | 直接 IP ⚠️ | 部分的匿名化 |
| `forced` | Tor | Onion Service のみ | 完全匿名化 |

**⚠️ 警告**: `outbound-only` モードでは受信 IP アドレスが露出します。本番環境では `forced` モードを使用してください。

---

## 前提条件

### Tor と torsocks のインストール

```bash
# セットアップスクリプトを使用
./scripts/tor-setup.sh install

# インストールを確認
./scripts/tor-setup.sh verify

# Tor 接続をテスト
./scripts/tor-setup.sh test
```

### 手動インストール

**Debian/Ubuntu:**
```bash
sudo apt-get install tor torsocks
sudo systemctl enable tor
sudo systemctl start tor
```

**macOS:**
```bash
brew install tor torsocks
brew services start tor
```

---

## Phase 1: 送信の匿名化 (torsocks)

### 基本的な使い方

torsocks を使ってノードのすべての送信接続を Tor 経由にルーティングします：

```bash
# ラッパースクリプトを使用（推奨）
./scripts/anarchy-tor.sh ./target/release/anarchy-node --tor-mode=outbound-only

# 手動で torsocks を使用
torsocks ./target/release/anarchy-node --tor-mode=outbound-only
```

### これが行うこと

- ノードからのすべての TCP 接続が Tor ネットワークを経由
- あなたの IP は他のピアから Tor 出口ノードの IP として見える
- **警告**: 受信接続は依然として直接到達する

### 確認方法

ノードが Tor を使用していることを確認：

```bash
# ノードのログで Tor 回路経由の接続が表示されるはず
# 外部ピアはあなたの実際の IP ではなく Tor 出口ノード IP を見る
```

---

## Phase 2: 受信の匿名化 (Onion Service)

### Onion Service のセットアップ

1. **Onion Service 設定を生成:**

```bash
./scripts/onion-service.sh setup
```

2. **torrc に追加（通常 `/etc/tor/torrc`）:**

```
HiddenServiceDir /var/lib/tor/anarchy-node/
HiddenServicePort 30333 127.0.0.1:30333
```

3. **Tor を再起動して .onion アドレスを取得:**

```bash
sudo systemctl restart tor
cat /var/lib/tor/anarchy-node/hostname
# 出力: xyz123...abc.onion
```

4. **Onion Service でノードを起動:**

```bash
./scripts/anarchy-tor.sh ./target/release/anarchy-node \
  --tor-mode=forced \
  --listen-addr=/ip4/127.0.0.1/tcp/30333 \
  --public-addr=/onion3/xyz123...abc:30333
```

### 完全匿名モード

完全な匿名性のために `--tor-mode=forced` を使用します:

```bash
./scripts/anarchy-tor.sh ./target/release/anarchy-node \
  --tor-mode=forced \
  --listen-addr=/ip4/127.0.0.1/tcp/30333 \
  --public-addr=/onion3/YOUR_ONION_ADDRESS:30333
```

これにより以下が強制されます:
- ① **出口ロック**: torsocks 下で実行していない場合ノードが終了
- ② **入口ロック**: 127.0.0.1 のみでリッスン（Onion Service のみ）

---

## Tor モード

### メインネットでの強制

**メインネットでは `--tor-mode=forced` が必須です**。これはプロトコルレベルで強制されます:

```rust
// メインネットでは --tor-mode 設定が自動的に forced に上書きされる
if chain_spec.id().contains("mainnet") {
    tor_mode = TorMode::Forced;
}
```

メインネットで `--tor-mode=off` を指定すると、警告がログに記録され自動的に `forced` モードに切り替わります。

### `--tor-mode=off` (デフォルト)

- 通常の TCP 接続
- **用途**: ローカル開発のみ
- **リスク**: IP が完全に露出

### `--tor-mode=outbound-only`

- 送信: Tor 経由（torsocks が必要）
- 受信: 直接 IP 接続
- **用途**: Tor セットアップのテスト
- **リスク**: 受信 IP が露出

### `--tor-mode=forced`

- 送信: Tor 経由（torsocks 強制）
- 受信: Onion Service のみ（27.0.0.1 でリッスン）
- **用途**: 本番ノード
- **リスク**: 最小限（完全匿名）

---

## ブートストラップノード

ブートストラップノードは、ネットワークに参加する際にノードが最初に接続するピアです。匿名ネットワークでは Onion アドレスを使用します。

### マルチアドレス形式

Onion v3 マルチアドレス形式:
```
/onion3/<56文字のbase32>:<ポート>/p2p/<ピアID>
```

構成要素:
- `/onion3/` - Tor v3 Onion Service のプロトコル識別子
- `<56文字のbase32>` - Onion アドレス（`.onion` サフィックスなし）
- `:<ポート>` - ポート番号（通常 30333）
- `/p2p/<ピアID>` - libp2p ピア ID（`12D3KooW...` で始まる）

### 例

**単一の Onion ブートノード:**
```
/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
```

**TCP + Onion 混合ブートノード**（最大の接続性のため）:
```
/ip4/1.2.3.4/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
```

### Onion ブートストラップノードへの接続

チェーンスペックに Onion ベースのブートストラップノードを追加:

```json
{
  "bootNodes": [
    "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooW..."
  ]
}
```

### 独自のブートストラップノードを運用する

1. Onion Service をセットアップ（Phase 2 参照）
2. `--tor-mode=forced` でノードを実行
3. `.onion` アドレスをネットワークと共有

---

## トラブルシューティング

### "torsocks not detected" エラー

ラッパースクリプトを使用していることを確認:
```bash
./scripts/anarchy-tor.sh ./target/release/anarchy-node --tor-mode=forced
```

### 接続が遅い

Tor はレイテンシを追加します。タイムアウトを増やしてください:
```bash
--network-request-timeout=90
```

### ブートストラップノードに接続できない

1. Tor が実行中か確認: `pgrep tor`
2. Tor 接続をテスト: `./scripts/tor-setup.sh test`
3. ブートストラップノードアドレスが有効な `.onion` 形式か確認

### Onion Service が動作しない

1. torrc 設定を確認
2. HiddenServiceDir のパーミッションを確認
3. Tor を再起動: `sudo systemctl restart tor`

---

## セキュリティノート

### 出口ノードのリスク

`outbound-only` モード使用時:
- トラフィックは他のピアに到達する前に Tor 出口ノードを通過
- 出口ノードは非暗号化トラフィックを検査できる可能性がある（ただし libp2p は暗号化を使用）
- 受信 IP アドレスは依然としてピアに露出

**緩和策**: 本番環境では常に `--tor-mode=forced` を使用。

### 秘密鍵の保護

Onion Service の秘密鍵は hidden service ディレクトリに保存されます:
- パス: `/var/lib/tor/anarchy-node/hs_ed25519_secret_key`
- このファイルを**絶対に**共有や公開しないでください
- `.onion` アドレスを復元する必要がある場合は安全に（暗号化して）バックアップ
- 漏洩した場合は新しい Onion Service を生成

### Onion-to-Onion ベストプラクティス

最大の匿名性のために、Onion-to-Onion のみの通信用にネットワークを設定:

1. チェーンスペックで **Onion ブートノードのみを使用**
2. すべてのノードで **`--tor-mode=forced` を設定**
3. `--public-addr /onion3/...` で **Onion アドレスのみを広告**
4. **クリアネットピアなし**: TCP/IP ピアへの接続を避ける

完全匿名ノードのセットアップ例:
```bash
./scripts/anarchy-tor.sh ./target/release/anarchy-node \
  --tor-mode=forced \
  --public-addr=/onion3/YOUR_ONION_ADDRESS:30333 \
  --chain mainnet
```

### 一般的なセキュリティ推奨事項

- 本番ノードでは **絶対に** `--tor-mode=off` を使用しない
- Onion Service の秘密鍵（`hs_ed25519_secret_key`）を安全に保管
- Tor を定期的に最新バージョンに更新
- Tor ログで不審なアクティビティを監視
- バリデータノードには専用マシン/VM を使用

---

## タイムアウト設定

Tor 接続は直接 TCP より高いレイテンシがあります。適切なタイムアウトを設定してください:

```bash
# Tor 用の推奨タイムアウト設定
./scripts/anarchy-tor.sh ./target/release/anarchy-node \
  --tor-mode=forced \
  --network-request-timeout=90 \
  --sync-mode=full
```

| パラメータ | デフォルト | Tor 推奨値 |
|-----------|----------|-----------|
| network-request-timeout | 30秒 | 90秒 |
| rpc-timeout | 30秒 | 60秒 |

# Quickstart: libp2p + Tor統合

**Feature**: 006-libp2p-tor  
**Date**: 2026-02-08

このガイドでは、Anarchyノードをtor経由で運用する方法を説明します。

---

## 前提条件

- Anarchyノードがビルド済み（`cargo build --release`）
- Linux または macOS環境
- sudo権限（Torインストール時）

---

## Phase 1: torsocksによる送信Tor化

### Step 1: Torとtorsocksのインストール

**Ubuntu/Debian**:
```bash
sudo apt update
sudo apt install tor torsocks
```

**macOS (Homebrew)**:
```bash
brew install tor torsocks
```

**Arch Linux**:
```bash
sudo pacman -S tor torsocks
```

### Step 2: Torサービスの起動

```bash
# systemdの場合
sudo systemctl start tor
sudo systemctl enable tor

# macOSの場合
brew services start tor
```

**確認**:
```bash
# Torが動作しているか確認
curl --socks5 localhost:9050 https://check.torproject.org/api/ip
# => {"IsTor":true,"IP":"xxx.xxx.xxx.xxx"}
```

### Step 3: torsocks経由でノードを起動

```bash
cd apps/blockchain
torsocks ./target/release/anarchy-node \
  --dev \
  --tor-mode=outbound-only
```

**確認ポイント**:
- ログに`Tor mode: outbound-only (**WARNING: Inbound IP exposed**)`が表示される
- ピア接続が確立される（数十秒〜数分かかる場合あり）

> **⚠️ 重要**: `outbound-only`モードでは送信のみTor化され、受信IPは露出します。
> 完全な匿名性が必要な場合はPhase 2（Onion Service）または`--tor-mode=forced`を使用してください。

---

## Phase 2: Onion Serviceによる受信Tor化

### Step 1: Onion Serviceの設定

`/etc/tor/torrc`を編集（sudo必要）:

```bash
sudo nano /etc/tor/torrc
```

以下を追加:
```
# Anarchy Node Onion Service
HiddenServiceDir /var/lib/tor/anarchy-node/
HiddenServicePort 30333 127.0.0.1:30333
```

### Step 2: Torを再起動

```bash
sudo systemctl restart tor
```

### Step 3: Onionアドレスの取得

```bash
sudo cat /var/lib/tor/anarchy-node/hostname
# => vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion
```

### Step 4: Onionアドレスを広告してノードを起動

```bash
ONION_ADDR=$(sudo cat /var/lib/tor/anarchy-node/hostname)

torsocks ./target/release/anarchy-node \
  --dev \
  --tor-mode=outbound-only \
  --public-addr=/onion3/${ONION_ADDR%.onion}:30333 \
  --listen-addr=/ip4/127.0.0.1/tcp/30333
```

> **ポイント**: `--public-addr`は「外部から見た私のアドレス」をノードに教える設定です。
> ノード自体はOnion Serviceの存在を知らないため、手動指定が必要です。

### Step 5: 別のTorノードから接続テスト

別のマシンで:
```bash
torsocks ./target/release/anarchy-node \
  --dev \
  --tor-mode=outbound-only \
  --bootnodes="/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooW..."
```

> **推奨**: ブートノードも自分も`.onion`アドレスを使うことで、通信が一度もクリアネットを経由しない「Onion-to-Onion」構成になります。
> これにより、悪意のあるTor出口ノードによる盗聴リスクを排除できます。

---

## 完全匿名モード（forced）

非Torピアを完全に拒否するモード:

```bash
torsocks ./target/release/anarchy-node \
  --tor-mode=forced \
  --reserved-only \
  --reserved-nodes="/onion3/boot1...:30333/p2p/12D3KooW..." \
  --reserved-nodes="/onion3/boot2...:30333/p2p/12D3KooW..."
```

**注意**: このモードではOnionアドレスを持つブートノードが必須です。

---

## トラブルシューティング

### 「Connection refused」エラー

1. Torが起動しているか確認:
   ```bash
   systemctl status tor
   ```
2. SOCKSポートが正しいか確認（通常9050）

### 接続が遅い

Torネットワークは通常のTCPより遅いです。以下を調整:
- タイムアウト値を長めに設定
- 複数のブートノードを設定してフォールバックを有効化

### 「torsocks not detected」エラー

`--tor-mode=outbound-only`でtorsocksなしで起動した場合に表示されます。
必ず`torsocks`コマンドでラップして起動してください。

### Onionアドレスが生成されない

1. `HiddenServiceDir`のパーミッション確認:
   ```bash
   sudo ls -la /var/lib/tor/anarchy-node/
   ```
2. Torユーザーが書き込み権限を持っているか確認

---

## 次のステップ

- [ ] 複数ノードでのテストネット構築
- [ ] ブートノードリストの共有（chain_spec.json更新）
- [ ] モニタリング・アラート設定

---

## 参考リンク

- [Tor Project - Onion Services](https://community.torproject.org/onion-services/)
- [torsocks GitHub](https://github.com/dgoulet/torsocks)
- [Substrate Network Configuration](https://docs.substrate.io/reference/command-line-tools/node-template/)

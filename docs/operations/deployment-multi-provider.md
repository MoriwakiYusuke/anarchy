# マルチプロバイダ デプロイ手順書

> **対象構成**: さくら (有料) + GCP (Always Free) + AWS (Free Tier) + Cloudflare Workers
> **接続パターン**: [tor-connection-patterns.md §2.5](tor-connection-patterns.md)
> **実装計画**: [docs/superpowers/plans/2026-09-11-multi-provider-deployment.md](../superpowers/plans/2026-09-11-multi-provider-deployment.md)

## 1. トポロジ

```
ユーザー ──HTTPS──▶ Cloudflare Workers (フロント / 静的 export)
                          │ wss://<GCPドメイン>/rpc
                          ▼
                     GCP e2-micro (1GB, Always Free)
                     chain×1 + nginx
                          │
                          │ Tor (torsocks + Hidden Service)
                          ├──────────────▶ さくら (8GB, 有料)
                          │                chain×3 + storage×3 + 採掘
                          │                公開ポートなし
                          └──────────────▶ AWS t3.micro (1GB, 12ヶ月無料)
                                           storage×1
```

**有料はさくら 1 台のみ。** 公開されるポートは GCP の 443 だけ。

### なぜ GCP がフロントの接続先か

さくらは重いバックエンド (採掘 + 大半のストレージ) を持ち、公開ポートを一切開けない。
GCP を公開エントリポイントにすることで、さくらの所在を晒さずに済む。

GCP のチェーンは **さくら/AWS のストレージへ直接 fan-out する** (さくらのチェーンを
中継しない)。エンドポイントはオンチェーンと gossip の二重経路で伝播するため、
GCP はチェーンを同期するだけで全ストレージノードを把握できる。

## 2. 実測値 (2026-09-11)

サイジングの根拠。推測ではなく計測値。

| 項目 | 実測 | 備考 |
|---|---|---|
| chain-node RSS (採掘なし) | **462 MB** (コンテナ) / 525 MB (ネイティブ) | GCP の 1GB に収まる |
| chain-node RSS (採掘あり) | **996 MB** | RandomX light の 256MB データセット込み。さくら向け |
| storage-node RSS | **20 MB** | どの無料枠にも入る |
| 採掘スレッド CPU | **29.7% of 1 core** | 90 秒計測、6 blocks |
| フロント静的 export | **4.8 MB** (`out/`) | wasm 604KB を含む |
| chain イメージ | **233 MB** | |
| storage イメージ | **156 MB** | |
| Tor connect (同一ホスト) | **1.4〜3.9 秒** | 跨プロバイダはさらに伸びる |

## 3. 前提

- 全ホスト **x86_64 / Ubuntu 24.04 LTS**、Docker と Docker Compose
- GCP のドメインと DNS A レコード (Let's Encrypt 用)
- Cloudflare アカウント (Workers)
- イメージは `ghcr.io/moriwakiyusuke/anarchy-node` / `anarchy-storage-node`

**GCP Always Free の条件**: `e2-micro` かつ **us-west1 / us-central1 / us-east1**、
標準永続ディスク 30GB まで。それ以外は課金される。

**AWS Free Tier は 12 ヶ月で失効する。** 期限をカレンダーに入れること。

## 4. 共通の準備

### 4.1 chainspec

全ノードが同一 genesis を共有する。リポジトリの
`infra/deploy/anarchy-portfolio-raw.json` をそのまま配る。

```
name: Anarchy Portfolio
id:   anarchy_portfolio
genesis: 0x425a4ce3ca639bf2c06e57ca72317fd34b6c6071b7c283c517e134b6c95baae7
```

同じバイナリなら再生成しても genesis は一致する (別プロセス 2 回で確認済み)。

### 4.2 GCP には swap を入れる

1GB に 462MB のノードが乗る上、断片は base64 の `String` として
`response.text()` で全量メモリに載る (`MAX_FRAGMENT_SIZE` = 128MB → base64 で約 171MB)。

```bash
sudo fallocate -l 2G /swapfile && sudo chmod 600 /swapfile
sudo mkswap /swapfile && sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

## 5. Tor の設定で必ず踏む罠

**設定を書く前にここを読むこと。** コンテナで実際に疎通させるまで表面化しなかったもの。

### 5.1 `HiddenServicePort` の転送先にホスト名は使えない

IP かソケットのみ。compose のサービス名で書くと tor が起動に失敗する:

```
[warn] Unparseable address in hidden service port configuration.
[err]  Reading config failed--see warnings above.
```

**対処**: compose のネットワークに固定 IP を割り当て、torrc からは IP で参照する。

```yaml
networks:
  anarchy:
    ipam:
      config:
        - subnet: 172.28.0.0/24
```
```
HiddenServicePort 30333 172.28.0.20:30333
```

### 5.2 `torsocks.conf` の `TorAddress` もホスト名不可

解決されず `socks5 libc connect: Connection refused` になる。IP で書く。
イメージ内の既定は `127.0.0.1` なので、コンテナを分ける構成では
compose から設定ファイルを上書きマウントする。

### 5.3 torsocks は **全ての** outbound を Tor に流す

同一ホスト宛の通信まで Tor で名前解決しようとして落ちる:

```
ERROR torsocks: Unable to resolve. Status reply: 4
```

**同一ホストは直結、サーバー間だけ Tor** という切り分けが要る。

| プロセス | torsocks | 理由 |
|---|---|---|
| チェーンノード | **包む** | ストレージが `.onion` で登録されるため fan-out に SOCKS 経由の名前解決が要る |
| ストレージ (チェーンと同一ホスト) | **包まない** | outbound は同一ホストのチェーンだけ。inbound は Hidden Service が転送 |
| ストレージ (チェーンが別ホスト = AWS) | **包む** | `--chain-url` に `.onion` を指定する |

### 5.4 Tor の connect は遅い

`.onion` 宛の `connect_timeout` は 60 秒に設定済み
([storage.rs](../../apps/blockchain/node/src/rpc/storage.rs) の `ANONYMOUS_CONNECT_TIMEOUT`)。
直結は 5 秒のまま (死んだエンドポイントの早期検出)。

以前は一律 5 秒で、**`.onion` のストレージには永久に到達できなかった**
(`tcp connect error: deadline has elapsed`)。

## 6. 各ホストの構築

### 6.1 さくら (chain×3 + storage×3 + 採掘)

1. tor を起動し onion を 2 本取得する (chain 用 / storage 用)
2. chain×3 を起動。**採掘は 1 台だけ** (`--mine --randomx-mode light`)。
   複数採掘は reorg を招く
3. storage×3 を起動。`--public-url` に **storage onion** を指定する
4. `storage_getNodes` で 3 台が `http://<onion>:303X` として登録されたことを確認。
   **`127.0.0.1` が 1 件でもあれば `--public-url` が効いていない**

### 6.2 GCP (chain×1 + nginx)

1. swap 2GB (§4.2)
2. tor を起動 (SOCKS のみ。Hidden Service は不要 — さくらへは outbound のみ)
3. chain×1 を起動。bootnode に さくらの chain onion を指定
4. **さくら/AWS のストレージに到達できることを確認してから先に進む**
5. nginx + Let's Encrypt で `wss://<domain>/rpc` を公開

nginx は `proxy_read_timeout 3600s` が **必須**。デフォルトの 60 秒では PoW の
ブロック間隔 (30 秒目標、指数分布で 40 秒超が日常的) で WS が切られる。

**Cloudflare Tunnel は使えない。** CF はアイドル WebSocket を閉じ、そのタイムアウトは
非公開かつ Enterprise でしか変更できない。PAPI の ws-provider は受信側の
ウォッチドッグだけで **ping を送らない** ため、ブロック間隔の隙間で接続が
本当にアイドルになり切断される。

### 6.3 AWS (storage×1)

セキュリティグループは **22 番のみ**。ストレージは Tor 経由でのみ公開する。
チェーンが別ホストなので **torsocks で包み**、`--chain-url` に `.onion` を指定する。

### 6.4 Cloudflare Workers (フロント)

```bash
cd packages/wasm-engine && wasm-pack build --target web --out-dir pkg
cd ../.. && pnpm install          # file: 依存はコピーなので wasm-pack 後に必須
NEXT_PUBLIC_CHAIN_RPC_URL=wss://<GCPのドメイン>/rpc pnpm --filter @anarchy/frontend build
cd apps/frontend && npx wrangler deploy
```

`NEXT_PUBLIC_*` は **ビルド時に焼き込まれる**。付け忘れると `ws://127.0.0.1:9944`
のままになるので、`out/_next/static/chunks/` を grep して確認すること。

デプロイ後、GCP のチェーンの `--rpc-cors` に払い出された URL を設定する
(フロントと RPC が別オリジンになるため)。

## 7. ローカルからの接続

### パターン A: フルノード型 (フロント + チェーン)

自分のマシンで両方動かし、さくらと P2P 同期する。NAT の内側でも
hidden service 不要 (こちらから dial するだけ)。

```bash
ANARCHY_RUNNING_UNDER_TORSOCKS=1 torsocks ./target/release/anarchy-node \
  --chain infra/deploy/anarchy-portfolio-raw.json \
  --base-path ~/.anarchy/chain \
  --bootnodes /dns4/<さくらのchain onion>/tcp/30333/p2p/<peer-id>

NEXT_PUBLIC_CHAIN_RPC_URL=ws://127.0.0.1:9944 pnpm dev:frontend
```

`peers: 0` が続く場合は `apps/blockchain/scripts/onion-proxy.sh` で socat トンネルを
立て、`/ip4/127.0.0.2/tcp/30333/p2p/<peer-id>` を bootnode にする
(sc-network の transport は `/onion3/` を dial できないため)。

### パターン B: フロントのみローカル

```bash
NEXT_PUBLIC_CHAIN_RPC_URL=wss://<GCPのドメイン>/rpc pnpm dev:frontend
```

### パターン C: 全部ローカル (compose)

```bash
cd infra/docker
cp ../deploy/anarchy-portfolio-raw.json chainspec.json
cp storage.toml.example storage.toml    # signer_seed を openssl rand -hex 32 で生成
docker compose up -d tor
docker compose exec tor cat /var/lib/tor/anarchy-storage/hostname   # .env に設定
docker compose up -d chain storage
```

## 8. トラブルシューティング

| 症状 | 原因と対処 |
|---|---|
| tor が起動ループ / `Unparseable address` | §5.1。`HiddenServicePort` の転送先を IP にする |
| `socks5 libc connect: Connection refused` | §5.2。`torsocks.conf` の `TorAddress` を IP にする |
| `Unable to resolve. Status reply: 4` | §5.3。同一ホスト宛のプロセスを torsocks で包んでいる |
| `tcp connect error: deadline has elapsed` | §5.4。`.onion` 宛の connect timeout。修正済みのバイナリか確認 |
| `Registering ... url=http://127.0.0.1:3030` | `--public-url` が効いていない。古いバイナリの可能性 |
| `No Storage Nodes connected` | チェーンがレジストリを持っていない。同期完了と `storage_getNodes` を確認 |
| 起動直後に `timestamp of the block is too far in the future` | **一過性**。`MinimumPeriod` が 15 秒なので、低難易度でブロックが 15 秒未満で出ると発生する。難易度が上がれば収まる |
| フロントの WS が数十秒で切れる | nginx の `proxy_read_timeout`。3600s にする |

## 9. 既知の制約

- **`X-Chain-Auth` は timestamp とメソッド名しか署名していない** (nonce も body
  ハッシュも無い)。窓の間は replay 可能。チェーン↔ストレージが Tor 経由なので
  下回りで緩和されているが、session-token 方式への移行が宿題
- **reqwest に `socks` feature が無い**ため torsocks (LD_PRELOAD) に依存している。
  明示的なプロキシ指定に移行すれば依存を外せる
- **rustc は 1.93.1 に固定**。`stable` にすると新しい rustc が引かれ、pinned な
  polkadot-sdk stable2503 の wasm ランタイムがリンクできなくなる。
  **SDK を上げるまで rustc も上げられない**
- **GCP Always Free は US 3 リージョン限定**なので、日本からは RPC 往復に
  120〜150ms 乗る
- **フロントは clearnet のみ。** GCP のノードは全訪問者の生 IP を見る。これは
  匿名 SNS 本来の脅威モデルではなく、公開デモとしての割り切り
- **ノード側に Tor 強制機構は無い** (`--tor-mode` は削除済み)。torsocks で包むかは
  運用側の判断

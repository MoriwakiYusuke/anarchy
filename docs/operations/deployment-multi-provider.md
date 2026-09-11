# マルチプロバイダ デプロイ手順書

> **対象構成**: core + gateway + storage-only + フロント
> (今回の割り当て: さくらのVPS / GCP Always Free / AWS Free Tier / Cloudflare Workers)
> **接続パターン**: [tor-connection-patterns.md §2.5](tor-connection-patterns.md)
> **実装計画**: [docs/superpowers/plans/2026-09-11-multi-provider-deployment.md](../superpowers/plans/2026-09-11-multi-provider-deployment.md)

## 1. トポロジ

```
ユーザー ──HTTPS──▶ フロント (静的 export / Cloudflare Workers)
                          │ wss://<gateway のドメイン>/rpc
                          ▼
                     gateway            ← GCP e2-micro (1GB, Always Free)
                     chain×1 + nginx
                          │
                          │ Tor (torsocks + Hidden Service)
                          ├──────────────▶ core       ← さくらのVPS 4G (有料)
                          │                chain×3 + storage×3 + 採掘
                          │                公開ポートなし
                          └──────────────▶ storage-only ← AWS t3.micro (12ヶ月無料)
                                           storage×1
```

**有料は core の 1 台のみ。** 公開されるポートは gateway の 443 だけ。

### なぜ gateway がフロントの接続先か

core は重いバックエンド (採掘 + 大半のストレージ) を持ち、公開ポートを一切開けない。
gateway を公開エントリポイントにすることで、core の所在を晒さずに済む。

gateway のチェーンは **core / storage-only のストレージへ直接 fan-out する** (core のチェーンを
中継しない)。エンドポイントはオンチェーンと gossip の二重経路で伝播するため、
gateway はチェーンを同期するだけで全ストレージノードを把握できる。

## 2. 実測値 (2026-09-11)

サイジングの根拠。推測ではなく計測値。

| 項目 | 実測 | 備考 |
|---|---|---|
| chain-node RSS (採掘なし) | **462 MB** (コンテナ) / 525 MB (ネイティブ) | gateway の 1GB に収まる |
| chain-node RSS (採掘あり) | **996 MB** | RandomX light の 256MB データセット込み。core 向け |
| storage-node RSS | **20 MB** | どの無料枠にも入る |
| 採掘スレッド CPU | **29.7% of 1 core** | 90 秒計測、6 blocks |
| フロント静的 export | **4.8 MB** (`out/`) | wasm 604KB を含む |
| chain イメージ | **233 MB** | |
| storage イメージ | **156 MB** | |
| Tor connect (同一ホスト) | **1.4〜3.9 秒** | 跨プロバイダはさらに伸びる |

## 3. 前提

- 全ホスト **x86_64 / Ubuntu 24.04 LTS**、Docker と Docker Compose
- gateway のドメインと DNS A レコード (Let's Encrypt 用)
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

### 5.4 同一ホストのチェーンノード同士は 127.0.0.1 でピアさせる

§5.3 の帰結。torsocks の除外設定は **`AllowOutboundLocalhost` (127.0.0.0/8) だけ**で、
サブネット単位の除外ができない (設定キーは `AllowInbound` / `AllowOutboundLocalhost` /
`IsolatePID` / `OnionAddrRange` / `TorAddress` / `TorPort` の 6 つのみ)。

固定 IP でピアさせようとすると **チェーンが互いに繋がらない**:

```
chain-2 | PERROR torsocks: socks5 libc connect: Connection refused
chain-2 | 💤 Idle (0 peers), best: #0
```

**対処**: 同一ホストのチェーンを 1 つのネットワーク名前空間に相乗りさせ、
`127.0.0.1` + `AllowOutboundLocalhost 1` でピアさせる。

### 5.5 名前空間の保持は tor ではなく専用コンテナに持たせる

チェーンを `network_mode: "service:tor"` にすると、**tor を再起動したときに
名前空間が作り直され、相乗りしているチェーンが取り残される**。

実測: tor 再起動後、チェーンは動き続けるが `peers=0` になり、
採掘ノードだけが単独で進んで **チェーンが分岐した** (chain-1 が #44、chain-2 が #29)。
自然復帰せず、チェーンコンテナの再起動が必要だった。

**対処**: 何もしない `netns` コンテナ (`alpine sleep infinity`) に名前空間を持たせ、
tor もチェーンもそこに相乗りする。これで tor の再起動と切り離せる。

修正後の実測: tor 再起動を跨いで `peers=1` を維持し、ブロックも揃って進行
(#8 → #11 → #14)、`.onion` fan-out も復帰した。

### 5.6 `/dns4/<onion>/` の bootnode は動かない — socat が必須

libp2p に `.onion` を dial させる方法は **socat でローカル TCP に落とすしかない**。

| 形式 | 結果 |
|---|---|
| `/onion3/<addr>:30333/p2p/<id>` | ❌ sc-network の `build_transport` は DNS(TCP)+WS のみで onion transport を持たない |
| `/dns4/<addr>.onion/tcp/30333/p2p/<id>` | ❌ **実測で `peers=0` のまま 5 分経過。dial の形跡すら残らない** |
| `/ip4/127.0.0.1/tcp/<local>/p2p/<id>` + socat | ✅ **実測で 210 秒後にピア確立、同期開始** |

`/dns4/` が通らないのは、torsocks が `getaddrinfo` を hook するのに対し、
**rust-libp2p の DNS transport は hickory-dns で独自に UDP の DNS 問い合わせを行い
libc の resolver を通らない**ため。torsocks が名前解決を捕捉できず、
`.onion` を解決できる者が誰もいないまま終わる。

socat は `SOCKS4A` で Tor の SOCKS ポートに繋ぐ (SOCKS4A はホスト名解決を
プロキシ側に委ねるので `.onion` が渡せる)。

### 5.7 Tor の connect は遅い

`.onion` 宛の `connect_timeout` は 60 秒に設定済み
([storage.rs](../../apps/blockchain/node/src/rpc/storage.rs) の `ANONYMOUS_CONNECT_TIMEOUT`)。
直結は 5 秒のまま (死んだエンドポイントの早期検出)。

以前は一律 5 秒で、**`.onion` のストレージには永久に到達できなかった**
(`tcp connect error: deadline has elapsed`)。

## 6. 各ホストの構築

### 6.1 core (chain×3 + storage×3 + 採掘)

1. tor を起動し onion を 2 本取得する (chain 用 / storage 用)
2. chain×3 を起動。**採掘は 1 台だけ** (`--mine --randomx-mode light`)。
   複数採掘は reorg を招く
3. storage×3 を起動。`--public-url` に **storage onion** を指定する
4. `storage_getNodes` で 3 台が `http://<onion>:303X` として登録されたことを確認。
   **`127.0.0.1` が 1 件でもあれば `--public-url` が効いていない**

### 6.2 gateway (chain×1 + nginx)

1. swap 2GB (§4.2)
2. tor を起動 (SOCKS のみ。Hidden Service は不要 — core へは outbound のみ)
3. chain×1 を起動。bootnode に core の chain onion を指定
4. **core / storage-only のストレージに到達できることを確認してから先に進む**
5. nginx + Let's Encrypt で `wss://<domain>/rpc` を公開

nginx は `proxy_read_timeout 3600s` が **必須**。デフォルトの 60 秒では PoW の
ブロック間隔 (30 秒目標、指数分布で 40 秒超が日常的) で WS が切られる。

**Cloudflare Tunnel は使えない。** CF はアイドル WebSocket を閉じ、そのタイムアウトは
非公開かつ Enterprise でしか変更できない。PAPI の ws-provider は受信側の
ウォッチドッグだけで **ping を送らない** ため、ブロック間隔の隙間で接続が
本当にアイドルになり切断される。

### 6.3 storage-only (storage×1)

セキュリティグループは **22 番のみ**。ストレージは Tor 経由でのみ公開する。
チェーンが別ホストなので **torsocks で包み**、`--chain-url` に `.onion` を指定する。

### 6.4 フロント (Cloudflare Workers)

```bash
cd packages/wasm-engine && wasm-pack build --target web --out-dir pkg
cd ../.. && pnpm install          # file: 依存はコピーなので wasm-pack 後に必須
NEXT_PUBLIC_CHAIN_RPC_URL=wss://<GCPのドメイン>/rpc pnpm --filter @anarchy/frontend build
cd apps/frontend && npx wrangler deploy
```

`NEXT_PUBLIC_*` は **ビルド時に焼き込まれる**。付け忘れると `ws://127.0.0.1:9944`
のままになるので、`out/_next/static/chunks/` を grep して確認すること。

デプロイ後、gateway のチェーンの `--rpc-cors` に払い出された URL を設定する
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

**`/dns4/<onion>/` 形式は使えない (実測済み)。** socat トンネルが必須:

```bash
socat TCP-LISTEN:30350,bind=127.0.0.1,reuseaddr,fork \
      SOCKS4A:127.0.0.1:<さくらのchain onion>:30333,socksport=9050
# bootnode は /ip4/127.0.0.1/tcp/30350/p2p/<peer-id>
```

理由は §5.7 を参照。

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
| 同一ホストのチェーンが `peers=0` のまま | §5.4。固定 IP でピアさせている。名前空間を共有して 127.0.0.1 にする |
| tor 再起動後にチェーンが分岐した | §5.5。`netns` コンテナを使っていない。応急処置はチェーンコンテナの再起動 |

## 9. 障害・復帰シナリオの実測結果

compose で実際に停止・再起動して観測したもの。**推測ではない。**

| シナリオ | 結果 |
|---|---|
| ストレージ停止 → 再起動 | ✅ 自動再登録。ハートビート (30 秒間隔) で復帰。データは volume で永続 |
| 採掘ノード停止 | ✅ 他チェーンは状態を保持 (best が巻き戻らない)。レジストリもオンチェーンから読めるため `total=2 online=2` を維持 |
| 採掘ノード再起動 | ✅ 自動でピア再確立、ブロック追従を再開。genesis から始まり直さない |
| tor 再起動 (netns コンテナあり) | ✅ onion アドレス不変、`peers=1` 維持、fan-out 復帰 |
| tor 再起動 (netns コンテナなし) | ❌ **チェーンが分岐する**。§5.5 参照 |
| チェーン間のレジストリ伝播 | ✅ 直接登録を受けていないノードもオンチェーン経由で全ストレージを把握 |

**onion アドレスは tor の volume に永続する。** volume を消さない限り再起動で変わらないので、
`--public-url` や bootnode の設定を書き直す必要はない。
逆に `docker compose down -v` すると **onion が再生成され全設定が無効になる**ので注意。

## 10. 既知の制約

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

# Tor sidecar (Docker)

Anarchy のローカル開発で **Tor デーモンと Hidden Service だけ Docker で立てる** ための最小構成。
ブロックチェーン / storage-node / フロントエンドは従来通り host のバイナリ (`cargo run` / `pnpm dev`)
で動かし、Tor だけコンテナ側に分離する。

## なぜ Docker (Tor だけ)

- システムワイドな `systemctl start tor` や `sudo` 編集の `/etc/tor/torrc.d/*.conf` を
  追加せずに、HiddenService + SOCKS5 が立ち上がる。
- `torrc` がリポジトリ内に commit されるので、**本番 (systemd で `tor.service` 起動) の
  設定がそのまま流用できる** (`HiddenServiceDir` の絶対パスだけ差し替え)。
- Tor デーモン自体は idle で RAM ~50MB / image ~30MB。host バイナリの再ビルドサイクルに
  影響を与えない。
- chain-node / storage-node まで含めて全部 Docker 化すると Rust ビルドキャッシュが効かず
  iteration が遅いため、コンテナ化するのは Tor のみに留めている。

## 提供されるエンドポイント

| 種類 | host 側アドレス | 用途 |
|---|---|---|
| SOCKS5 | `127.0.0.1:9050` | host バイナリの outbound 匿名化 (`ALL_PROXY=socks5h://...` / `torsocks`) |
| Hidden Service: `anarchy-node` | `/var/lib/tor/anarchy-node/hostname` | libp2p P2P (`30333`) + Substrate RPC/WS (`9944`) |
| Hidden Service: `anarchy-storage` | `/var/lib/tor/anarchy-storage/hostname` | storage-node HTTP JSON-RPC (`3030`) |

> chain-node / storage-node / frontend は host の loopback (`127.0.0.1:{30333,9944,3030}`)
> で listen していれば、コンテナ側 Tor がそのまま転送する (`network_mode: host` のため)。

## 使い方

### 1. 起動

```bash
cd infra/docker/tor
docker compose up -d --build
docker compose logs -f tor   # "Bootstrapped 100% (done)" が出ればOK
```

または dev-stack 経由:

```bash
./scripts/dev-stack.sh start --with-tor
```

> **host で system tor が走っている場合 (9050 衝突)**:
> `pnpm stack:start:tor` / `dev-stack.sh start --with-tor` 経由なら **9150 に自動フォールバック**
> されるため何もしなくて良い (起動時に warn でその旨が表示される)。
> `docker compose` を直接叩く場合は env var で明示する:
> ```bash
> TOR_SOCKS_HOST_PORT=9150 docker compose up -d
> ```
> その場合 host バイナリ側の torsocks も `~/.torsocks.conf` または `TORSOCKS_CONF_FILE`
> 経由で 9150 を見るよう設定する必要がある (`TorAddress 127.0.0.1` / `TorPort 9150`)。
> system tor を完全に置き換えたい場合は `sudo systemctl stop tor && sudo systemctl disable tor`。

### 2. .onion アドレスの取得

```bash
docker compose exec tor cat /var/lib/tor/anarchy-node/hostname
docker compose exec tor cat /var/lib/tor/anarchy-storage/hostname
```

それぞれ `xxxxx...xxxx.onion` (56 文字 base32 + `.onion`) が返る。volume `anarchy-tor-data`
が残っている限り `.onion` は同じものが使われる。

### 3. host バイナリを Tor 経由で動かす

```bash
# outbound だけ Tor 化
cd apps/blockchain
./scripts/anarchy-tor.sh ./target/release/anarchy-node --tor-mode=outbound-only

# 完全匿名 (outbound + inbound) — .onion を public-addr に流す
ONION="$(docker compose -f ../../infra/docker/tor/compose.yml exec -T tor cat /var/lib/tor/anarchy-node/hostname)"
./scripts/anarchy-tor.sh ./target/release/anarchy-node \
  --tor-mode=forced \
  --public-addr "/onion3/${ONION%.onion}:30333"
```

`anarchy-tor.sh` は host の `torsocks` を呼ぶが、`torsocks` はデフォルトで
`127.0.0.1:9050` の SOCKS5 を見にいくため、追加設定なしで Docker 側 Tor に乗る。

### 4. SOCKS5 経由の疎通確認

```bash
curl --socks5-hostname 127.0.0.1:9050 https://check.torproject.org/api/ip
# 期待: {"IsTor":true,"IP":"..."}
```

### 5. 停止 / クリーンアップ

```bash
docker compose down       # 停止 (Hidden Service の鍵は保持される)
docker compose down -v    # volume も削除 → 次回起動で .onion が再生成される
```

## 本番 (バイナリのみ運用) との関係

- **dev**: Docker compose で tor デーモン
- **prod**: host に `apt install tor` + `systemctl enable --now tor`
- **torrc は共通**: `infra/docker/tor/torrc` をそのまま `/etc/tor/torrc.d/anarchy.conf` に置き、
  `HiddenServiceDir` を `/var/lib/tor/anarchy-node` (debian-tor 所有) に揃えるだけで動く

詳細な本番デプロイ手順は [`apps/blockchain/docs/tor-deployment.md`](../../../apps/blockchain/docs/tor-deployment.md) を参照。

## トラブルシューティング

| 症状 | 原因と対処 |
|---|---|
| `docker compose up` が `port is already allocated` で失敗 | host で別の `tor` が走っていないか確認: `pgrep -a tor`、`sudo systemctl stop tor` |
| `.onion` が空 / 存在しない | bootstrap 完了前に取りに行っている。`docker compose logs tor` で `Bootstrapped 100%` を待つ (通常 5〜30 秒) |
| host バイナリから SOCKS5 に繋がらない | `curl --socks5-hostname 127.0.0.1:9050 https://check.torproject.org/api/ip` でまず疎通確認。失敗するなら compose 側 health を確認 |
| macOS で `network_mode: host` が効かない | compose.yml を bridge + `host.docker.internal` + `ports: ["127.0.0.1:9050:9050"]` に書き換える。HiddenServicePort の転送先も `host.docker.internal:<port>` に変更が必要 |

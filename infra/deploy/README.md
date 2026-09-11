# infra/deploy — ホスト別のデプロイ構成

`core/` `gateway/` `storage-only/` は **すべて [gen.py](gen.py) が生成する**。手で YAML を編集しない。
各 `compose.yml` の先頭に再生成コマンドが書いてあるので、それが現在の構成の正。

```sh
# core: chain×3 (2 validator, 1 採掘) + storage×3。他ホストの bootnode になるので node-key を固定
./gen.py core --chains 3 --validators 2 --mine --storage 3 --capacity 10G \
    --node-key 0000000000000000000000000000000000000000000000000000000000000001

# gateway: chain×1 + storage×3。core を bootnode にし、RPC を nginx 経由で公開
./gen.py gateway --chains 1 --storage 3 --capacity 5G --bootnode-core \
    --public-rpc --domain rpc.anarchy2026.org --cors-origin https://anarchy2026.org --subnet 30

# storage-only: storage×10。チェーンは core (Tor 経由)
./gen.py storage-only --storage 10 --capacity 2G
```

台数・容量を変えるときは引数を変えて再生成し、差分を見て commit する。
`./gen.py --help` で全引数。

| 引数 | 意味 |
|---|---|
| `--chains N` | チェーン台数。0 なら storage が torsocks で包まれ、`CORE_CHAIN_ONION` に繋ぐ |
| `--validators N` | 先頭 N 台を `--validator` にする (GRANDPA。2/3 以上必要) |
| `--mine` | chain-1 で採掘。**1 ホストだけ**にする (複数採掘は reorg) |
| `--storage N --capacity SIZE` | ストレージ台数と 1 台あたりの宣言容量 (実ディスクから逆算) |
| `--bootnode-core` | core の chain-1 を socat トンネル経由で bootnode にする |
| `--public-rpc --domain HOST --cors-origin ORIGIN` | chain-1 の RPC を外に出し、`nginx-anarchy.conf` も生成。ORIGIN はフロントのオリジン |
| `--node-key HEX64` | chain-1 の libp2p 鍵を固定。**他ホストから bootnode として参照されるホストだけ** |
| `--subnet N` | `172.N.0.0/24`。既定 28。nginx の `proxy_pass` もこれに追従する |

## ホストの調達

- core (さくら VPS 4G) / gateway (GCP e2-micro) はコンソール / `gcloud` で手作業
- storage-only (AWS t3.micro) は [aws-storage-only.sh](aws-storage-only.sh) が冪等に作る (`stop` / `start` / `destroy` も)

## 生成後の手順 (ホスト上)

`.env` `storage-*.toml` `chainspec.json` は gitignore 済み。ホスト上で作る。

```sh
./bootstrap.sh                          # 初回のみ: apt / UTC / swap / Docker
cd infra/deploy/<core|gateway|storage-only>
cp ../anarchy-portfolio-raw.json chainspec.json         # chain があるホストのみ
for i in $(seq 1 <storage 台数>); do
  sed "s/CHANGE_ME/$(openssl rand -hex 32)/" storage.toml.tmpl > storage-$i.toml
done
cp .env.example .env

docker compose up -d netns tor           # storage-only は tor だけ
docker compose exec tor cat /var/lib/tor/anarchy-storage/hostname   # → .env STORAGE_PUBLIC_URL_BASE
docker compose exec tor cat /var/lib/tor/anarchy-chain/hostname     # core のみ。gateway/storage-only の CORE_CHAIN_ONION に

# validator がいるホストのみ: gran 鍵を keystore に入れる (chain-N ごと、Alice/Bob …)
docker compose run --rm --entrypoint anarchy-node chain-1 key insert \
    --base-path /data --chain /chainspec.json --scheme ed25519 --suri //Alice --key-type gran

docker compose up -d chain-1
docker compose logs chain-1 | grep "Local node identity"            # → .env CHAIN1_PEER_ID
docker compose up -d
```

gateway はさらにホスト側で nginx + certbot:
`sudo cp nginx-anarchy.conf /etc/nginx/sites-available/anarchy && sudo ln -s ... sites-enabled/`

## 再生成して既存ホストに当てるとき

- サービス名・volume 名は `chain-N` / `chainN-data` に統一されている。旧構成 (`chain` / `chain-data`) から
  切り替えると **チェーンは空 volume で起動して再同期**し、node-key も再生成される (`--node-key` 固定のホスト以外)。
  `docker compose up -d --remove-orphans` で旧コンテナを片付ける。
- `.env` の `STORAGE<N>_PUBLIC_URL` は `STORAGE_PUBLIC_URL_BASE` (ポート無し) に統一された。

詳細な設計根拠と罠は [docs/operations/deployment-multi-provider.md](../../docs/operations/deployment-multi-provider.md) §5。

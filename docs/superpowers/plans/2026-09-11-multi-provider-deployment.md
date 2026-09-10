# Anarchy マルチプロバイダ デプロイ実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **これは [2026-09-10-two-vps-deployment.md](2026-09-10-two-vps-deployment.md) を置き換える。** 旧計画は「VPS 2 台」前提で、事業者選定・無料枠・Cloudflare フロントの検討を経て構成が変わった。旧計画の Task 1 (storage-node `--public-url`) は実装済み。

**Goal:** 4 プロバイダに Anarchy を分散デプロイし、Cloudflare 上のフロントから常時アクセスできる状態にする。有料はさくら 1 台のみ。

**Architecture:** さくら (有料・非公開) がチェーン 3 台 + ストレージ 3 台 + 採掘を持つ重いバックエンド。GCP の無料枠 (e2-micro) がチェーン 1 台と公開 `wss` エンドポイントを持つエントリポイント。AWS 無料枠にストレージ 1 台。フロントは静的 export して Cloudflare Workers Static Assets に置く。サーバ間は全て Tor hidden service 経由 (torsocks)、公開されるのは GCP の 443 のみ。

**Tech Stack:** Polkadot SDK stable2503 (PoW/RandomX), Rust storage-node (libp2p + axum), Next.js 16 静的 export + PAPI, Tor (hidden service + SOCKS5), nginx, systemd, Wrangler

**Spec:** 本計画内の「Requirements」節

---

## Requirements

| 項目 | 決定 |
|---|---|
| さくら (8GB クラス) | chain×3 + storage×3 + 採掘。**公開ポートを開けない** (Tor のみ) |
| GCP e2-micro (Always Free, 1GB) | chain×1 + nginx。**フロントの接続先**。常時稼働 |
| AWS t3.micro (Free Tier, 1GB) | storage×1。**無料枠は 12 ヶ月で失効** |
| Cloudflare Workers | フロント (静的 export)。Static Assets は課金対象外 |
| 配布方式 | **コンテナ (ghcr.io) + docker compose**。バイナリ転送はしない |
| Tor | サーバー間のみ。同一ホスト内は直結 |
| フロント公開形態 | clearnet のみ |

## 実測値 (2026-09-11)

推測ではなく計測値。サイジングと設定値の根拠。

| 項目 | 実測 |
|---|---|
| chain-node RSS (採掘なし) | **462 MB** (コンテナ) / 525 MB (ネイティブ) |
| chain-node RSS (採掘あり) | **996 MB** |
| storage-node RSS | **20 MB** |
| 採掘スレッド CPU | **29.7% of 1 core** (90 秒計測、6 blocks) |
| chain イメージ | **233 MB** |
| storage イメージ | **156 MB** |
| フロント静的 export | **4.8 MB** (`out/`、wasm 604KB 込み) |
| Tor connect (同一ホスト、ディスクリプタ公開済み) | **1.4〜3.9 秒** |
| genesis hash | `0x425a4ce3ca639bf2c06e57ca72317fd34b6c6071b7c283c517e134b6c95baae7` |

## Global Constraints

- **全ホスト x86_64 / Ubuntu 24.04 LTS + Docker。** バイナリは配らない
  (イメージに glibc ごと入っているため、ホストのディストリを揃える制約が消える)
- **rustc は 1.93.1 固定。** `stable` にすると新しい rustc が引かれ、pinned な
  polkadot-sdk stable2503 の wasm ランタイムがリンクできない。SDK を上げるまで
  rustc も上げられない
- **`HiddenServicePort` の転送先にホスト名は使えない** (IP かソケットのみ)。
  compose のネットワークに固定 IP を割り当てること
- **`torsocks.conf` の `TorAddress` もホスト名不可**
- **torsocks は全ての outbound を Tor に流す。** 除外設定は
  `AllowOutboundLocalhost` (127.0.0.0/8) だけでサブネット単位の除外はできない。
  同一ホストのチェーン同士は **1 つのネットワーク名前空間を共有し 127.0.0.1 で
  ピアさせること**。固定 IP でピアさせると `peers=0` のまま繋がらない (実測済み)
- **`/dns4/<onion>/` の bootnode は動かない。** libp2p に `.onion` を dial させるには
  socat でローカル TCP に落とすしかない (rust-libp2p の DNS transport は hickory-dns で
  独自に UDP 問い合わせをするため torsocks が捕捉できない)。実測済み
- **名前空間の保持は tor ではなく `netns` コンテナに持たせる。**
  `network_mode: "service:tor"` にすると tor 再起動で名前空間が作り直され、
  チェーンが取り残されて **分岐する** (実測: chain-1 が #44、chain-2 が #29)
- **採掘は 1 ノードのみ** (`chain-s1`)。`--randomx-mode light`
- **GCP には swap 2GB** を入れる
- **既存データとの互換性は考慮しない** (CLAUDE.md Compatibility Policy)
- ポート割り当て:
  - さくら chain: p2p `30333/30334/30335`, rpc `9944/9945/9946`
  - さくら storage: http `3030-3032`, libp2p `4001-4003`
  - GCP chain: p2p `30333`, rpc `9944`, nginx `80/443`
  - AWS storage: http `3030`, libp2p `4001`
  - Tor SOCKS `9050` (全ホスト)

## 完了済み

本計画を書く過程で実装・検証まで終わったもの。**再実行は不要。**

| 内容 | コミット |
|---|---|
| storage-node `--public-url` (跨ホストのエンドポイント広告) | `7fc943c` |
| チェーンの stale 掘り直し修正 (100% → 29.7%) | `d0c6978` |
| フロント静的 export 対応 | `6b3b947` |
| Tor 強制機構の削除 (`--tor-mode`) | `6a97593` |
| Dockerfile / compose / ghcr ワークフロー | `530ae30` |
| rustc 1.93.1 固定 | `f0a6f75` |
| chainspec 生成 (`infra/deploy/anarchy-portfolio-raw.json`) | `7983cbf` |
| **Tor fan-out の疎通** (`.onion` connect timeout ほか 4 件) | `372387e` |
| wrangler 設定 / `socks-proxy-agent` 削除 | `fda4917` |
| 運用手順書 / tor-connection-patterns の訂正 | `24fcb21` |

**compose で tor + chain + storage を実際に起動し、`storage_getFragment` が
`.onion` 経由で `Fragment not found` を返すところまで確認済み** (= 経路が通っている)。

さらに 2 チェーン + 2 ストレージ構成で障害・復帰シナリオを実測した
(詳細は [deployment-multi-provider.md §9](../../operations/deployment-multi-provider.md)):

| シナリオ | 結果 |
|---|---|
| ストレージ停止 → 再起動 | ✅ 自動再登録、データ永続 |
| 採掘ノード停止 → 再起動 | ✅ 他ノードは状態保持、復帰後にピア再確立 |
| tor 再起動 | ✅ onion 不変、ピア維持 (`netns` コンテナ導入後) |
| チェーン間のレジストリ伝播 | ✅ 直接登録を受けていないノードも把握 |

**さらに本番形状 (さくら chain×3 + storage×3) と跨ホスト接続を実測した:**

| 検証 | 結果 |
|---|---|
| さくら本番形状 | ✅ チェーン 3 台が `peers=2` で全て best 一致、ストレージ 3 台が `.onion` 登録 |
| 跨ホストのピア確立 (別 compose プロジェクト間、Tor のみ) | ✅ socat 経由で 210 秒後に確立、同期開始 |
| 跨ホストのレジストリ伝播 | ✅ GCP 側は直接登録を受けずに さくらのストレージ 3 台を把握 |
| **跨ホストの fan-out** | ✅ **GCP のチェーンが さくらのストレージ 3 台すべてに到達** (各ノードのログに受信を確認) |

## 残っている作業

VPS が要るものだけ。ローカルで済むものは全て完了している。

| Task | 内容 | 前提 |
|---|---|---|
| 1 | ghcr へのイメージ公開 | main への push (CI が自動実行) |
| 2 | さくら構築 | さくら VPS |
| 3 | GCP 構築 | GCP インスタンス + ドメイン |
| 4 | AWS 構築 | AWS インスタンス |
| 5 | Cloudflare デプロイ | CF アカウント + GCP のドメイン確定 |
| 6 | ブラウザ E2E 検証 | 上記すべて |

## File Structure

| ファイル | 状態 |
|---|---|
| `infra/deploy/anarchy-portfolio-raw.json` | ✅ 作成済み |
| `infra/docker/chain/Dockerfile` | ✅ ビルド検証済み |
| `infra/docker/storage/Dockerfile` | ✅ ビルド検証済み |
| `infra/docker/compose.yml` | ✅ 疎通検証済み (要ポート増設) |
| `infra/docker/torrc` / `torsocks.conf` | ✅ 疎通検証済み (要ポート増設) |
| `infra/docker/storage.toml.example` | ✅ 作成済み |
| `.github/workflows/container-images.yml` | ⚠️ 未実行 (CI 初回で確認) |
| `apps/frontend/wrangler.jsonc` | ✅ 作成済み (deploy 未実行) |
| `docs/operations/deployment-multi-provider.md` | ✅ 作成済み |

---

### Task 1: ghcr へのイメージ公開

`.github/workflows/container-images.yml` は作成済みだが **一度も実行されていない**。
main への push で初回ビルドが走る。Dockerfile はローカルでビルド検証済みなので
通る見込みだが、GitHub Actions 特有の失敗 (キャッシュ、権限) はここで初めて分かる。

**Files:** 変更なし (既存ワークフローの実行)

**Interfaces:**
- Produces: `ghcr.io/moriwakiyusuke/anarchy-node:latest` /
  `ghcr.io/moriwakiyusuke/anarchy-storage-node:latest`

- [ ] **Step 1: PR を出してビルドのみ実行させる**

ワークフローは PR では push せずビルドだけ行う。まずここで Dockerfile の
CI 上での成否を確認する。

Expected: 2 つの matrix job (`anarchy-node` / `anarchy-storage-node`) が緑

**失敗しやすい箇所**: `type=gha` キャッシュの容量制限、Substrate ビルドの
timeout-minutes: 90 超過。超える場合は cache-to の mode を `min` に落とす

- [ ] **Step 2: main にマージして push させる**

Expected: ghcr にイメージが上がる

- [ ] **Step 3: パッケージを public にする**

GitHub の Packages 設定で 2 つとも public にする。
各ホストに認証情報を置かずに `docker pull` できるようにするため。

- [ ] **Step 4: 認証なしで pull できることを確認**

```bash
docker logout ghcr.io
docker pull ghcr.io/moriwakiyusuke/anarchy-node:latest
docker run --rm ghcr.io/moriwakiyusuke/anarchy-node:latest --help > /dev/null && echo OK
```
Expected: `OK`

---

### Task 2: さくら — chain×3 + storage×3 + 採掘

`infra/docker/compose.yml` は 1 ノードずつの最小構成なので、3 台ずつに増やす。
固定 IP と torrc の `HiddenServicePort` を対応させること。

**Files:**
- Create: `infra/deploy/sakura/compose.yml`
- Create: `infra/deploy/sakura/torrc`

**Interfaces:**
- Consumes: ghcr のイメージ、`infra/deploy/anarchy-portfolio-raw.json`
- Produces: chain onion (仮想ポート 30333-30335, 9944-9946) と
  storage onion (3030-3032)

- [ ] **Step 1: 固定 IP を割り当てた compose を書く**

`infra/docker/compose.yml` をベースに、サービスを `chain-1..3` / `storage-1..3` に増やす。
IP 割り当て例:

| サービス | IP |
|---|---|
| tor | 172.28.0.10 |
| chain-1 / 2 / 3 | 172.28.0.21 / .22 / .23 |
| storage-1 / 2 / 3 | 172.28.0.31 / .32 / .33 |

**採掘は chain-1 だけ**に `--mine --coinbase <SS58> --randomx-mode light` を付ける。
chain-2 / chain-3 は `--bootnodes /ip4/172.28.0.21/tcp/30333/p2p/<peer-id>` で繋ぐ。

- [ ] **Step 2: torrc を IP で書く**

```
HiddenServiceDir /var/lib/tor/anarchy-chain
HiddenServicePort 30333 172.28.0.21:30333
HiddenServicePort 30334 172.28.0.22:30334
HiddenServicePort 30335 172.28.0.23:30335
HiddenServicePort 9944  172.28.0.21:9944
HiddenServicePort 9945  172.28.0.22:9945
HiddenServicePort 9946  172.28.0.23:9946

HiddenServiceDir /var/lib/tor/anarchy-storage
HiddenServicePort 3030 172.28.0.31:3030
HiddenServicePort 3031 172.28.0.32:3031
HiddenServicePort 3032 172.28.0.33:3032
```

**サービス名で書かないこと** (tor が `Unparseable address` で起動失敗する)。

- [ ] **Step 3: tor だけ起動して onion を取得**

```bash
docker compose up -d tor && sleep 30
docker compose exec tor cat /var/lib/tor/anarchy-chain/hostname
docker compose exec tor cat /var/lib/tor/anarchy-storage/hostname
```
Expected: 56 文字 + `.onion` が 2 つ。**両方控える**

- [ ] **Step 4: chain-1 を起動して peer ID を取得**

```bash
docker compose up -d chain-1 && sleep 30
docker compose logs chain-1 | grep -E "Chain specification|Local node identity"
```
Expected:
- `Chain specification: Anarchy Portfolio`
- `Local node identity is: 12D3Koo...` ← 控える

- [ ] **Step 5: 残りを起動**

peer ID を chain-2 / chain-3 の bootnode に反映し、storage の `--public-url` に
storage onion を設定してから全部起動する。

- [ ] **Step 6: チェーン 3 台が互いに繋がっていることを確認**

```bash
for p in 9944 9945 9946; do
  docker compose exec tor wget -qO- --post-data='{"jsonrpc":"2.0","id":1,"method":"system_health","params":[]}' \
    --header='Content-Type: application/json' http://172.28.0.2X:$p
done
```
Expected: `peers` が 1 以上、ブロック番号の差が 1-2 以内

- [ ] **Step 7: ストレージ 3 台が .onion で登録されたことを確認**

```bash
docker compose exec tor wget -qO- --post-data='{"jsonrpc":"2.0","id":1,"method":"storage_getNodes","params":[]}' \
  --header='Content-Type: application/json' http://172.28.0.21:9944
```
Expected: 3 件すべて `http://<storage-onion>:303X`。
**`127.0.0.1` が 1 件でもあれば `--public-url` が効いていない** (古いイメージの可能性)

- [ ] **Step 8: 採掘の CPU を確認**

Expected: 30% 前後 (ローカル実測 29.7%)。100% 近い場合は `d0c6978` を含まない
古いイメージ

- [ ] **Step 9: コミット**

```bash
git add infra/deploy/sakura/
git commit -m "chore(deploy): add sakura compose (3 chain + 3 storage)

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: GCP — chain×1 + 公開 wss

**Files:**
- Create: `infra/deploy/gcp/compose.yml`
- Create: `infra/deploy/gcp/torrc`
- Create: `infra/deploy/gcp/nginx-anarchy.conf`

**Interfaces:**
- Consumes: さくらの chain onion と peer ID
- Produces: `wss://<domain>/rpc`

- [ ] **Step 1: インスタンス作成と swap**

```bash
gcloud compute instances create anarchy-gcp \
  --machine-type=e2-micro --zone=us-west1-b \
  --image-family=ubuntu-2404-lts-amd64 --image-project=ubuntu-os-cloud \
  --boot-disk-size=30GB --boot-disk-type=pd-standard
```

**Always Free の条件**: `e2-micro` かつ us-west1 / us-central1 / us-east1、
標準永続ディスク 30GB まで。

swap 2GB を必ず入れる (1GB に 462MB のノード + 断片の一時展開が乗るため):
```bash
sudo fallocate -l 2G /swapfile && sudo chmod 600 /swapfile
sudo mkswap /swapfile && sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```
Expected: `free -h` の Swap 行に 2.0Gi

- [ ] **Step 2: torrc は SOCKS のみ**

さくらへは outbound だけなので Hidden Service は不要。

- [ ] **Step 3: chain を torsocks 込みで起動**

bootnode は **socat トンネル経由**で指定する。`infra/deploy/gcp/compose.yml` に
`onion-proxy` サービスとして組み込み済み。

`/dns4/<onion>/` は **動かないことを実測で確認済み** (§5.6)。

- [ ] **Step 4: さくらと同期しているか確認**

Expected: `peers` が 1 以上、`isSyncing` が false に落ち着く。
`free -h` で swap を食い潰していないことも確認

- [ ] **Step 5: 同期しない場合のフォールバック**

`peers: 0` が続くなら socat トンネルを立てる:
```bash
socat TCP-LISTEN:30333,bind=127.0.0.2,reuseaddr,fork \
      SOCKS4A:172.28.0.10:<さくらのchain onion>:30333,socksport=9050
```
bootnode を `/ip4/127.0.0.2/tcp/30333/p2p/<peer-id>` に差し替える。
torsocks が localhost 宛を遮断する場合は `AllowOutboundLocalhost 1` を追加。

- [ ] **Step 6: さくら/AWS のストレージに到達できることを確認**

**ここが構成の核心。通らなければ先に進んでも無意味。**

```bash
docker compose exec chain sh -c 'torsocks wget -qO- --timeout=90 http://<storage-onion>:3030/metrics' | head -3
```
Expected: メトリクスが返る

- [ ] **Step 7: nginx + Let's Encrypt**

```nginx
location /rpc {
    proxy_pass http://127.0.0.1:9944;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_read_timeout  3600s;   # 必須: 60s では PoW のブロック間隔で切れる
    proxy_send_timeout  3600s;
}
```

**Cloudflare Tunnel は使わないこと。** CF はアイドル WebSocket を閉じ、その
タイムアウトは Enterprise でしか変更できない。PAPI は ping を送らないため
ブロック間隔の隙間で切断される。

- [ ] **Step 8: WS が張れることを確認**

```bash
curl -s -i -N -H "Connection: Upgrade" -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  https://<domain>/rpc | head -5
```
Expected: `HTTP/1.1 101 Switching Protocols`

---

### Task 4: AWS — storage×1

3 プロバイダ目。ストレージは実測 20MB なので t3.micro に余裕で収まる。

**Files:**
- Create: `infra/deploy/aws/compose.yml`
- Create: `infra/deploy/aws/torrc`

- [ ] **Step 1: インスタンス作成**

`t3.micro` / Ubuntu 24.04 / 10GB gp3。セキュリティグループは **22 番のみ**
(ストレージは Tor 経由でのみ公開する)。

**無料枠は 12 ヶ月で失効する。** 期限をカレンダーに登録すること。

- [ ] **Step 2: torrc に storage の Hidden Service**

- [ ] **Step 3: storage を torsocks 込みで起動**

**さくらとは逆に、ここでは torsocks で包む。** チェーンが別ホストなので
`--chain-url=ws://<さくらのchain onion>:9944` を指定する。

**subxt (jsonrpsee WS) が torsocks 経由で `.onion` に繋がるかは未検証。**
失敗する場合は `--chain-url` を GCP のチェーンに向けるか、socat トンネルを検討する。

- [ ] **Step 4: 登録が全チェーンに伝播したか確認**

さくら (9944/9945/9946) と GCP (9944) の **4 ノードすべて**で
`http://<aws-onion>:3030` が見えること
Expected: オンチェーン経由なので、同期さえしていれば全ノードに現れる

---

### Task 5: Cloudflare Workers — フロント

**Files:** 変更なし (`apps/frontend/wrangler.jsonc` は作成済み)

- [ ] **Step 1: wasm を再ビルドして依存を入れ直す**

```bash
cd packages/wasm-engine && wasm-pack build --target web --out-dir pkg
cd ../.. && pnpm install
```
**`file:` 依存はコピーなので `wasm-pack` の後に `pnpm install` が必須。**
忘れると古い wasm がバンドルされる。

- [ ] **Step 2: 接続先を焼き込んでビルド**

```bash
NEXT_PUBLIC_CHAIN_RPC_URL=wss://<GCPのドメイン>/rpc pnpm --filter @anarchy/frontend build
```

- [ ] **Step 3: 焼き込まれた URL を検証**

```bash
grep -ro "wss://[^\"']*" apps/frontend/out/_next/static/chunks/ | head -3
```
Expected: GCP のドメインが現れる。**ここを飛ばさないこと** — 環境変数を
付け忘れると `ws://127.0.0.1:9944` のままデプロイされる

- [ ] **Step 4: デプロイ**

```bash
cd apps/frontend && npx wrangler deploy
```
Expected: `https://anarchy-frontend.<subdomain>.workers.dev` が払い出される

- [ ] **Step 5: CORS を通す**

GCP のチェーンの `--rpc-cors` に払い出された URL を設定して再起動する
(フロントと RPC が別オリジンのため)。

---

### Task 6: ブラウザからの E2E 検証

**この計画で唯一「全部繋がったこと」を証明するタスク。**

- [ ] **Step 1: フロントを開く**

Expected: DevTools の Network で `wss://<GCPドメイン>/rpc` が `101` で確立

- [ ] **Step 2: ブロックが進むことを確認**

Expected: ブロック番号が増える (さくらの chain-1 が採掘している)

- [ ] **Step 3: ウォレット作成と faucet**

Expected: 残高が反映される (純オンチェーンなのでストレージ不要)

- [ ] **Step 4: テキスト投稿**

Expected: エラーなく完了。`No Storage Nodes connected` が出る場合は Task 3 Step 6 に戻る

- [ ] **Step 5: サーバー側で fan-out を確認**

GCP: `Fragment uploaded to Storage Node`
さくら/AWS: 断片受信のログ (配置は merkle_root 依存なのでどれか 1 台)

- [ ] **Step 6: リロードして読み戻せることを確認**

Expected: 投稿本文が表示される
(= GCP のチェーンが別プロバイダのストレージから断片を取得して復元できている)

- [ ] **Step 7: メモリと OOM を確認 (GCP)**

```bash
free -h && sudo dmesg | grep -i "killed process"
```
Expected: swap を食い潰していない、OOM killer が動いていない

- [ ] **Step 8: 画像添付を試す**

1-2MB の画像を添付して投稿・表示し、Step 7 を再確認する。
断片は base64 で全量メモリに載るため、ここが 1GB インスタンスの実質的な上限テスト。

- [ ] **Step 9: 手順書に実測値を反映**

`docs/operations/deployment-multi-provider.md` の onion アドレス、peer ID、
ブロック間隔、メモリ使用量を実際の値で更新する。**プレースホルダを残さないこと。**

---

## Self-Review

**1. Requirements coverage**

| 要件 | 対応 |
|---|---|
| さくら: chain×3 + storage×3 + 採掘、公開ポートなし | Task 2 |
| GCP: chain×1 + 公開 wss | Task 3 |
| AWS: storage×1 | Task 4 |
| Cloudflare: フロント | Task 5 |
| コンテナ配布 (ghcr) | Task 1 |
| Tor はサーバー間のみ | Task 2/3/4 (torsocks の適用範囲を各所で明示) |
| ローカル接続 | `docs/operations/deployment-multi-provider.md` §7 |

**2. 未検証として残っているもの**

- Task 4 Step 3: subxt が torsocks 経由で `.onion` の WS に繋がるか
  (AWS のストレージ単独ノードのみ該当。さくら/GCP は同一ホストのチェーンを使うので無関係)

**解決済み** (当初は未検証だったもの):
- ~~`/dns4/<onion>/` による libp2p dial~~ → **動かないことを実測で確認**。socat 必須
- ~~GitHub Actions 上での Docker ビルド~~ → CI 成功 (21m13s / 4m29s)

**3. スコープ外 (意図的)**

- reqwest への `socks` feature 追加 (torsocks 方式を採用)
- `X-Chain-Auth` の session-token 化 (既知の制約として記載)
- 採掘のハッシュレート上限設定 (29.7% で許容)
- マルチアーキビルド (全ホスト x86_64)

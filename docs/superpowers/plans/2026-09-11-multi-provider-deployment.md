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
| AWS t3.micro (Free Tier, 1GB) | storage×1。常時稼働。**無料枠は 12 ヶ月で失効** |
| Cloudflare Workers | フロント (静的 export)。Static Assets は課金対象外 |
| Tor | 有効。サーバ間は全て hidden service 経由 |
| Tor 実現方式 | torsocks で包む (reqwest の `socks` feature は使わない) |
| フロント公開形態 | clearnet のみ (Onion-Location / フロント用 hidden service は作らない) |
| ネットワーク | プライベートネットワーク不使用。全てグローバル到達可能なアドレス |

## 実測値 (2026-09-11、本計画の前提)

| 項目 | 実測 | 出典 |
|---|---|---|
| chain-node RSS (採掘なし) | **525 MB** | `--dev` 起動 120 秒安定 |
| storage-node RSS | **20 MB** | 同 90 秒安定 |
| 採掘スレッド CPU (修正後) | **29.7% of 1 core** | 90 秒計測、6 blocks |
| フロント静的 export | **4.8 MB** (`out/`) | `next build` 成功、全ルート prerender |

## Global Constraints

- **チェーンスペック id に `"mainnet"` を含めない。** [command.rs:290](../../../apps/blockchain/node/src/command.rs) が id の部分文字列一致で `--tor-mode` を `Forced` に強制上書きする
- **`--tor-mode=forced` は listen を `/ip4/127.0.0.1/tcp/30333` に決め打ちする** ([command.rs:156](../../../apps/blockchain/node/src/command.rs))。**同一ホストで複数チェーンノードを Forced にすると bind が衝突する**。さくらの 3 台は `outbound-only` + 明示的な `--listen-addr /ip4/127.0.0.1/tcp/<port>` で同等の効果を得る。GCP の単一ノードのみ `forced` を使う
- **採掘は 1 ノードのみ** (`chain-s1`)。複数ノードの同時採掘は reorg を招く。`--randomx-mode light` (256MB) を使う
- **全ホストを同一ディストリ (Ubuntu 24.04 LTS) で揃える。** ローカルでビルドしたバイナリを配るため glibc を一致させる必要がある。GCP e2-micro / AWS t3.micro / さくら はいずれも **x86_64** なのでクロスコンパイルは不要
- **GCP e2-micro に swap 2GB を割り当てる。** 1GB に 525MB のノードが乗る上、断片は base64 `String` として `response.text()` で全量メモリに載る ([storage.rs:48](../../../apps/blockchain/node/src/rpc/storage.rs) の `MAX_FRAGMENT_SIZE` は 128MB = base64 で約 171MB)
- **既存データとの互換性は考慮しない** (CLAUDE.md Compatibility Policy)
- コメント・ドキュメントは日本語
- ポート割り当て:
  - さくら chain: p2p `30333/30334/30335`, rpc `9944/9945/9946`, prometheus `9615/9616/9617`
  - さくら storage: http rpc `3030-3032`, libp2p `4001-4003`
  - GCP chain: p2p `30333`, rpc `9944`, nginx `80/443`
  - AWS storage: http rpc `3030`, libp2p `4001`
  - Tor SOCKS `9050` (全ホスト)

## 完了済み (本計画の前提となる実装)

| 内容 | コミット |
|---|---|
| storage-node `--public-url` (跨ホストのエンドポイント広告) | `7fc943c` |
| チェーンノードの stale 掘り直し修正 (100% → 29.7%) | `d0c6978` |
| フロント静的 export 対応 | `6b3b947` |

## File Structure

| ファイル | 責務 |
|---|---|
| `infra/deploy/anarchy-portfolio-raw.json` | 全ノードが共有する raw chainspec |
| `infra/deploy/build-and-ship.sh` | ローカルビルド → 各ホストへ配布 |
| `infra/deploy/sakura/torrc` | chain×3 + storage×3 の hidden service |
| `infra/deploy/sakura/anarchy-chain@.service` | チェーンノード systemd テンプレート |
| `infra/deploy/sakura/anarchy-storage@.service` | ストレージノード systemd テンプレート |
| `infra/deploy/gcp/torrc` | SOCKS のみ (hidden service なし) |
| `infra/deploy/gcp/anarchy-chain.service` | チェーンノード systemd ユニット (forced) |
| `infra/deploy/gcp/nginx-anarchy.conf` | 公開 HTTPS + `/rpc` の WS プロキシ |
| `infra/deploy/aws/torrc` | storage 用 hidden service |
| `infra/deploy/aws/anarchy-storage.service` | ストレージノード systemd ユニット |
| `apps/frontend/wrangler.jsonc` | Workers Static Assets 設定 |
| `docs/operations/deployment-multi-provider.md` | 手順書とローカル接続 runbook |
| `docs/operations/tor-connection-patterns.md` | §2.4 として本構成を追記 |

---

### Task 1: チェーンスペックの生成と固定

4 ノード全てが同一 genesis を共有する必要がある。id に `"mainnet"` が入らないことを保証する。

**Files:**
- Create: `infra/deploy/anarchy-portfolio-raw.json`

**Interfaces:**
- Produces: `anarchy-portfolio-raw.json` — 全ノードが `--chain` に指定する raw chainspec。id は `anarchy_portfolio`

- [ ] **Step 1: チェーンノードをビルド**

Run: `cd apps/blockchain && cargo build --release --bin anarchy-node`
Expected: `target/release/anarchy-node` が生成される

- [ ] **Step 2: プレーンな chainspec を生成**

Run:
```bash
cd apps/blockchain && ./target/release/anarchy-node build-spec \
  --chain local --disable-default-bootnode > /tmp/anarchy-plain.json
```
Expected: JSON が生成される

- [ ] **Step 3: name と id を書き換える**

```bash
python3 - <<'PY'
import json
p = "/tmp/anarchy-plain.json"
spec = json.load(open(p))
spec["name"] = "Anarchy Portfolio"
spec["id"] = "anarchy_portfolio"   # "mainnet" を含めないこと
spec["chainType"] = "Local"
json.dump(spec, open(p, "w"), indent=2)
print(spec["id"], spec["chainType"])
PY
```
Expected: `anarchy_portfolio Local`

- [ ] **Step 4: id に mainnet が含まれないことを検証**

Run: `python3 -c "import json;s=json.load(open('/tmp/anarchy-plain.json'));assert 'mainnet' not in s['id'], s['id'];print('OK', s['id'])"`
Expected: `OK anarchy_portfolio`

- [ ] **Step 5: raw chainspec に変換して保存**

Run:
```bash
mkdir -p infra/deploy
cd apps/blockchain && ./target/release/anarchy-node build-spec \
  --chain /tmp/anarchy-plain.json --raw --disable-default-bootnode \
  > ../../infra/deploy/anarchy-portfolio-raw.json
```

- [ ] **Step 6: raw spec で起動することを確認**

Run:
```bash
cd apps/blockchain && timeout 30 ./target/release/anarchy-node \
  --chain ../../infra/deploy/anarchy-portfolio-raw.json --tmp 2>&1 | head -30
```
Expected: `Chain specification: Anarchy Portfolio` が出てパニックしない

- [ ] **Step 7: コミット**

```bash
git add infra/deploy/anarchy-portfolio-raw.json
git commit -m "chore(deploy): add anarchy-portfolio raw chainspec

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: バイナリのビルドと配布スクリプト

全ホスト x86_64 / Ubuntu 24.04 LTS で揃えるためクロスコンパイルは不要。ただし glibc を合わせるためビルド環境も Ubuntu 24.04 にする。

**Files:**
- Create: `infra/deploy/build-and-ship.sh`

**Interfaces:**
- Produces: `build-and-ship.sh <user@host> <sakura|gcp|aws>` — 役割に応じたバイナリと chainspec を `/opt/anarchy/` に配置する

- [ ] **Step 1: スクリプトを作成**

```bash
cat > infra/deploy/build-and-ship.sh <<'EOF'
#!/usr/bin/env bash
# ローカルでビルドしたバイナリと chainspec を各ホストに配布する。
#
#   ./build-and-ship.sh user@sakura sakura   # chain + storage
#   ./build-and-ship.sh user@gcp    gcp      # chain のみ
#   ./build-and-ship.sh user@aws    aws      # storage のみ
#
# VPS 上でのビルドは避けること (Substrate の release ビルドは 16GB 級の
# メモリを要求し、GCP e2-micro / AWS t3.micro (1GB) では不可能)。
# glibc を合わせるため Ubuntu 24.04 LTS でビルドすること。
set -euo pipefail

HOST="${1:?usage: $0 <user@host> <sakura|gcp|aws>}"
ROLE="${2:?usage: $0 <user@host> <sakura|gcp|aws>}"
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

need_chain=0; need_storage=0
case "$ROLE" in
    sakura) need_chain=1; need_storage=1 ;;
    gcp)    need_chain=1 ;;
    aws)    need_storage=1 ;;
    *) echo "unknown role: $ROLE (expected sakura|gcp|aws)" >&2; exit 1 ;;
esac

if [ "$need_chain" = 1 ]; then
    echo "==> building anarchy-node"
    ( cd "$REPO_ROOT/apps/blockchain" && cargo build --release --bin anarchy-node )
fi
if [ "$need_storage" = 1 ]; then
    echo "==> building anarchy-storage-node"
    ( cd "$REPO_ROOT/apps/storage-node" && cargo build --release --bin anarchy-storage-node )
fi

echo "==> preparing /opt/anarchy on $HOST"
ssh "$HOST" 'sudo mkdir -p /opt/anarchy/bin /opt/anarchy/env && sudo chown -R "$USER" /opt/anarchy'

if [ "$need_chain" = 1 ]; then
    scp "$REPO_ROOT/apps/blockchain/target/release/anarchy-node" "$HOST:/opt/anarchy/bin/"
fi
if [ "$need_storage" = 1 ]; then
    scp "$REPO_ROOT/apps/storage-node/target/release/anarchy-storage-node" "$HOST:/opt/anarchy/bin/"
fi

echo "==> shipping chainspec"
scp "$REPO_ROOT/infra/deploy/anarchy-portfolio-raw.json" "$HOST:/opt/anarchy/"

echo "==> verifying"
ssh "$HOST" 'ls -la /opt/anarchy /opt/anarchy/bin && /opt/anarchy/bin/anarchy-node --version 2>/dev/null || true'
EOF
chmod +x infra/deploy/build-and-ship.sh
```

- [ ] **Step 2: 構文検証**

Run: `bash -n infra/deploy/build-and-ship.sh && echo OK`
Expected: `OK`

- [ ] **Step 3: 引数チェックの動作確認**

Run: `./infra/deploy/build-and-ship.sh host bogus 2>&1 | head -2; echo "exit=$?"`
Expected: `unknown role: bogus (expected sakura|gcp|aws)` が出て非ゼロ終了

- [ ] **Step 4: コミット**

```bash
git add infra/deploy/build-and-ship.sh
git commit -m "chore(deploy): add build-and-ship script for 3-host distribution

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: さくら — Tor 設定

chain 3 台と storage 3 台を hidden service で公開する。onion は用途ごとに 2 本にまとめ、仮想ポートで分ける。

**Files:**
- Create: `infra/deploy/sakura/torrc`

**Interfaces:**
- Produces:
  - `/var/lib/tor/anarchy-chain/hostname` — 仮想ポート `30333/30334/30335` (p2p), `9944/9945/9946` (RPC)
  - `/var/lib/tor/anarchy-storage/hostname` — 仮想ポート `3030/3031/3032`
  - SOCKS5 `127.0.0.1:9050`

- [ ] **Step 1: torrc を作成**

```bash
mkdir -p infra/deploy/sakura
cat > infra/deploy/sakura/torrc <<'EOF'
# Anarchy さくら用 torrc (/etc/tor/torrc に配置)
#
# さくらは公開ポートを一切開けない。外部との通信は全てこの hidden service 経由。
# onion は用途ごとに 2 本にまとめ、仮想ポートでノードを分ける
# (同一ホストに同居している事実は隠せないので 6 本に分ける利点が無い)。

DataDirectory /var/lib/tor
RunAsDaemon 0
Log notice syslog

# ---- SOCKS ----
# チェーン / ストレージノードが torsocks 経由で外に出るために使う。
SocksPort 127.0.0.1:9050 IsolateClientAddr IsolateSOCKSAuth

# ---- Hidden Service: チェーンノード 3 台 ----
# p2p は GCP のノードからの bootnode 接続を受ける。RPC は運用確認用。
HiddenServiceDir /var/lib/tor/anarchy-chain
HiddenServiceVersion 3
HiddenServicePort 30333 127.0.0.1:30333
HiddenServicePort 30334 127.0.0.1:30334
HiddenServicePort 30335 127.0.0.1:30335
HiddenServicePort 9944  127.0.0.1:9944
HiddenServicePort 9945  127.0.0.1:9945
HiddenServicePort 9946  127.0.0.1:9946

# ---- Hidden Service: ストレージノード 3 台 ----
# GCP のチェーンノードがここに直接 fan-out する
# (オンチェーン/gossip でエンドポイントを知り、さくらのチェーンを中継しない)。
HiddenServiceDir /var/lib/tor/anarchy-storage
HiddenServiceVersion 3
HiddenServicePort 3030 127.0.0.1:3030
HiddenServicePort 3031 127.0.0.1:3031
HiddenServicePort 3032 127.0.0.1:3032

ClientUseIPv6 1
ClientPreferIPv6ORPort 0
SafeLogging 1
EOF
```

- [ ] **Step 2: 構文検証** (さくら上)

```bash
sudo cp infra/deploy/sakura/torrc /etc/tor/torrc
sudo -u debian-tor tor --verify-config -f /etc/tor/torrc
```
Expected: `Configuration was valid`

- [ ] **Step 3: 起動して onion を取得** (さくら上)

```bash
sudo systemctl restart tor && sleep 15
sudo cat /var/lib/tor/anarchy-chain/hostname
sudo cat /var/lib/tor/anarchy-storage/hostname
```
Expected: 56 文字 + `.onion` が 2 つ。**両方を控える** (以降のタスクで使う)

- [ ] **Step 4: SOCKS 確認** (さくら上)

Run: `ss -tlnp | grep 9050`
Expected: `127.0.0.1:9050` で listen

- [ ] **Step 5: コミット**

```bash
git add infra/deploy/sakura/torrc
git commit -m "chore(deploy): add sakura torrc (3 chain + 3 storage hidden services)

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: さくら — チェーンノード 3 台

`--tor-mode=forced` は listen を `127.0.0.1:30333` に決め打ちして衝突するため、`outbound-only` + 明示的な `--listen-addr` で同じ効果を得る。

**Files:**
- Create: `infra/deploy/sakura/anarchy-chain@.service`

**Interfaces:**
- Consumes: `/opt/anarchy/bin/anarchy-node`, `/opt/anarchy/anarchy-portfolio-raw.json`
- Produces: `anarchy-chain@1..3`。`@1` のみ採掘。peer ID は `--node-key-file` で固定

- [ ] **Step 1: systemd テンプレートを作成**

```bash
cat > infra/deploy/sakura/anarchy-chain@.service <<'EOF'
[Unit]
Description=Anarchy chain node %i (sakura)
After=network-online.target tor.service
Wants=network-online.target
Requires=tor.service

[Service]
Type=simple
User=anarchy
Group=anarchy
WorkingDirectory=/opt/anarchy
EnvironmentFile=/opt/anarchy/env/chain-%i.env

# torsocks 配下で起動する。
#   outbound: 全て Tor 経由 (ANARCHY_RUNNING_UNDER_TORSOCKS=1)
#   inbound:  --listen-addr を 127.0.0.1 に明示し hidden service 経由のみにする
#
# --tor-mode=forced を使わない理由: forced は listen を
# /ip4/127.0.0.1/tcp/30333 に決め打ちするため、同一ホストで 3 台動かすと
# bind が衝突する。
Environment=ANARCHY_RUNNING_UNDER_TORSOCKS=1
ExecStart=/usr/bin/torsocks /opt/anarchy/bin/anarchy-node \
  --chain /opt/anarchy/anarchy-portfolio-raw.json \
  --base-path /var/lib/anarchy/chain-%i \
  --node-key-file /var/lib/anarchy/chain-%i/node-key \
  --listen-addr /ip4/127.0.0.1/tcp/${P2P_PORT} \
  --rpc-port ${RPC_PORT} \
  --prometheus-port ${PROM_PORT} \
  --rpc-cors all \
  --tor-mode outbound-only \
  $MINING_ARGS \
  $BOOTNODE_ARGS

Restart=on-failure
RestartSec=10
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
```

- [ ] **Step 2: 環境ファイルを配置** (さくら上)

```bash
sudo useradd -r -s /usr/sbin/nologin anarchy 2>/dev/null || true
sudo mkdir -p /opt/anarchy/env /var/lib/anarchy/chain-{1,2,3}
for i in 1 2 3; do
  head -c 32 /dev/urandom | xxd -p -c 32 | sudo tee /var/lib/anarchy/chain-$i/node-key > /dev/null
done

# chain-1: 唯一の採掘ノード。--coinbase は自分の SS58 に差し替える
sudo tee /opt/anarchy/env/chain-1.env > /dev/null <<'EOF'
P2P_PORT=30333
RPC_PORT=9944
PROM_PORT=9615
MINING_ARGS=--mine --coinbase 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY --randomx-mode light
BOOTNODE_ARGS=
EOF

sudo tee /opt/anarchy/env/chain-2.env > /dev/null <<'EOF'
P2P_PORT=30334
RPC_PORT=9945
PROM_PORT=9616
MINING_ARGS=
BOOTNODE_ARGS=--bootnodes /ip4/127.0.0.1/tcp/30333/p2p/PLACEHOLDER_PEER_ID_1
EOF

sudo tee /opt/anarchy/env/chain-3.env > /dev/null <<'EOF'
P2P_PORT=30335
RPC_PORT=9946
PROM_PORT=9617
MINING_ARGS=
BOOTNODE_ARGS=--bootnodes /ip4/127.0.0.1/tcp/30333/p2p/PLACEHOLDER_PEER_ID_1
EOF

sudo chown -R anarchy:anarchy /var/lib/anarchy /opt/anarchy/env
```

- [ ] **Step 3: chain-1 を起動して peer ID を取得** (さくら上)

```bash
sudo cp infra/deploy/sakura/anarchy-chain@.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl start anarchy-chain@1
sleep 20
sudo journalctl -u anarchy-chain@1 -n 60 --no-pager | grep -E "Local node identity|Tor mode|Chain specification"
```
Expected:
- `Chain specification: Anarchy Portfolio`
- `⚠️  Tor mode: OUTBOUND-ONLY` (意図通り。inbound は `--listen-addr` で閉じている)
- `Local node identity is: 12D3Koo...` — **控える**

- [ ] **Step 4: chain-2 / chain-3 を起動** (さくら上)

```bash
PEER1=<Step 3 の peer ID>
sudo sed -i "s/PLACEHOLDER_PEER_ID_1/$PEER1/" /opt/anarchy/env/chain-2.env /opt/anarchy/env/chain-3.env
sudo systemctl start anarchy-chain@2 anarchy-chain@3 && sleep 25
```

- [ ] **Step 5: 3 台が接続していることを確認** (さくら上)

```bash
for p in 9944 9945 9946; do
  echo -n "rpc $p: "
  curl -s -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"system_health","params":[]}' \
    http://127.0.0.1:$p | python3 -c 'import sys,json;print(json.load(sys.stdin)["result"])'
done
```
Expected: 3 つとも応答し `peers` が 1 以上

- [ ] **Step 6: 採掘とブロック追従を確認** (さくら上)

```bash
sudo journalctl -u anarchy-chain@1 -n 100 --no-pager | grep -cE "🏆|Submitted valid seal"
sudo journalctl -u anarchy-chain@1 -n 200 --no-pager | grep -c "rejected seal"
for p in 9944 9945 9946; do
  echo -n "rpc $p best: "
  curl -s -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader","params":[]}' \
    http://127.0.0.1:$p | python3 -c 'import sys,json;print(int(json.load(sys.stdin)["result"]["number"],16))'
done
```
Expected: seal 提出が 1 件以上、**`rejected seal` は 0 件** (`d0c6978` の修正が効いている)、3 台のブロック番号の差が 1-2 以内

- [ ] **Step 7: 採掘スレッドの CPU を確認** (さくら上)

```bash
PID=$(systemctl show -p MainPID --value anarchy-chain@1)
TID=$(for t in /proc/$PID/task/*; do grep -q pow-nonce-loop $t/comm 2>/dev/null && basename $t; done)
A=$(awk '{print $14+$15}' /proc/$PID/task/$TID/stat); sleep 60
B=$(awk '{print $14+$15}' /proc/$PID/task/$TID/stat)
echo "pow thread: $(echo "scale=1; ($B-$A)*100/$(getconf CLK_TCK)/60" | bc)% of 1 core"
```
Expected: 30% 前後 (ローカル実測 29.7%)。100% 近い場合は修正が入っていないバイナリの可能性

- [ ] **Step 8: enable してコミット**

```bash
# さくら上
sudo systemctl enable anarchy-chain@{1,2,3}
# ローカル
git add infra/deploy/sakura/anarchy-chain@.service
git commit -m "chore(deploy): add sakura chain node systemd template (3 nodes)

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: さくら — ストレージノード 3 台

`--public-url` (`7fc943c`) で **onion アドレスを広告する**。これで GCP のチェーンノードからも同じ URL で到達できる。

**Files:**
- Create: `infra/deploy/sakura/anarchy-storage@.service`

**Interfaces:**
- Consumes: `/opt/anarchy/bin/anarchy-storage-node`, `Config::public_url`, さくらの storage onion
- Produces: 3 台が `http://<sakura-storage-onion>:303X` として自己登録

- [ ] **Step 1: systemd テンプレートを作成**

```bash
cat > infra/deploy/sakura/anarchy-storage@.service <<'EOF'
[Unit]
Description=Anarchy storage node %i (sakura)
After=network-online.target tor.service anarchy-chain@1.service
Wants=network-online.target
Requires=tor.service

[Service]
Type=simple
User=anarchy
Group=anarchy
WorkingDirectory=/opt/anarchy
EnvironmentFile=/opt/anarchy/env/storage-%i.env

# torsocks 必須: storage -> chain の登録は subxt (jsonrpsee WS) を使っており
# SOCKS プロキシを差す口が無い。
Environment=ANARCHY_RUNNING_UNDER_TORSOCKS=1
ExecStart=/usr/bin/torsocks /opt/anarchy/bin/anarchy-storage-node \
  --config /opt/anarchy/env/storage-%i.toml \
  --data-dir /var/lib/anarchy/storage-%i \
  --rpc-port ${RPC_PORT} \
  --public-url ${PUBLIC_URL} \
  --chain-url ${CHAIN_URL}

Restart=on-failure
RestartSec=10
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
```

- [ ] **Step 2: 設定を配置** (さくら上)

```bash
SAKURA_STORAGE_ONION=<Task 3 Step 3 で控えた storage onion>
SEED=$(openssl rand -hex 32)

for i in 1 2 3; do
  port=$((3029 + i))    # 3030..3032
  p2p=$((4000 + i))     # 4001..4003
  sudo mkdir -p /var/lib/anarchy/storage-$i
  sudo tee /opt/anarchy/env/storage-$i.toml > /dev/null <<EOF
capacity = 10737418240
declare_rate_limit = 10
auth_enabled = true
dev_mode = false
listen_addr = "/ip4/127.0.0.1/tcp/$p2p"
signer_seed = "$SEED"
EOF
  sudo tee /opt/anarchy/env/storage-$i.env > /dev/null <<EOF
RPC_PORT=$port
PUBLIC_URL=http://$SAKURA_STORAGE_ONION:$port
CHAIN_URL=ws://127.0.0.1:9944
EOF
done
sudo chown -R anarchy:anarchy /var/lib/anarchy /opt/anarchy/env
sudo chmod 600 /opt/anarchy/env/storage-*.toml
```

**注意:** `signer_seed` はノードごとに別の値にするのが望ましい。上記は簡略化のため共有している。

- [ ] **Step 3: 起動して広告 URL を確認** (さくら上)

```bash
sudo cp infra/deploy/sakura/anarchy-storage@.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl start anarchy-storage@{1,2,3}
sleep 20
sudo journalctl -u anarchy-storage@1 -n 40 --no-pager | grep -E "Registering|Registered|HTTP RPC server"
```
Expected:
- `HTTP RPC server started addr=0.0.0.0:3030`
- `Registering with blockchain node url=http://<onion>:3030`

**`url=http://127.0.0.1:3030` と出る場合は `7fc943c` が入っていない古いバイナリ。** Task 2 の配布をやり直す

- [ ] **Step 4: チェーン側のレジストリを確認** (さくら上)

```bash
curl -s -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"rpc_methods","params":[]}' \
  http://127.0.0.1:9944 | python3 -c 'import sys,json;print([m for m in json.load(sys.stdin)["result"]["methods"] if m.startswith("storage_")])'
```
Expected: `storage_` 系メソッドが列挙される。レジストリ照会用のメソッド名を確認し、それで 3 件が `http://<onion>:303X` として登録されていること (`127.0.0.1` が 1 件も無いこと) を確認する

- [ ] **Step 5: gossip 伝播を確認** (さくら上)

Step 4 と同じ照会を `9945` / `9946` に対して実行
Expected: 同じ 3 件が返る

- [ ] **Step 6: enable してコミット**

```bash
sudo systemctl enable anarchy-storage@{1,2,3}
git add infra/deploy/sakura/anarchy-storage@.service
git commit -m "chore(deploy): add sakura storage node systemd template (3 nodes)

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: さくら内での fan-out 疎通確認

GCP を作る前に、さくら単体でチェーン → ストレージの fan-out が通ることを確認する。ここで失敗するなら GCP からは絶対に通らない。

**Files:** なし (検証のみ)

- [ ] **Step 1: torsocks 経由で onion に到達できるか** (さくら上)

```bash
sudo -u anarchy torsocks curl -s -m 30 -o /dev/null -w '%{http_code}\n' \
  http://$SAKURA_STORAGE_ONION:3030/metrics
```
Expected: `200`

**失敗時:** `000`/タイムアウトなら hidden service が publish しきっていない (`sudo journalctl -u tor -n 50` を確認し 2-3 分待つ)。torsocks エラーなら `/etc/tor/torrc` の `SocksPort` と `/etc/tor/torsocks.conf` の `TorPort` が一致しているか確認

- [ ] **Step 2: 対照実験 (localhost 直)** (さくら上)

Run: `curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:3030/metrics`
Expected: `200` (これが失敗するならストレージノード自体が死んでいる)

- [ ] **Step 3: 3 台全てに到達できるか** (さくら上)

```bash
for p in 3030 3031 3032; do
  echo -n "$p: "
  sudo -u anarchy torsocks curl -s -m 30 -o /dev/null -w '%{http_code}\n' \
    http://$SAKURA_STORAGE_ONION:$p/metrics
done
```
Expected: 3 つとも `200`

- [ ] **Step 4: 記録**

`storage_listNodes` 相当の出力と各チェーンノードの peer ID を控える (以降のタスクで使う)

---

### Task 7: GCP — インスタンス作成と Tor

Always Free の e2-micro (1GB) にチェーンノードを載せる。RAM がギリギリなので swap を必ず入れる。

**Files:**
- Create: `infra/deploy/gcp/torrc`

**Interfaces:**
- Produces: GCP インスタンス (Ubuntu 24.04, swap 2GB, SOCKS5 `127.0.0.1:9050`)

- [ ] **Step 1: インスタンスを作成**

```bash
gcloud compute instances create anarchy-gcp \
  --machine-type=e2-micro \
  --zone=us-west1-b \
  --image-family=ubuntu-2404-lts-amd64 \
  --image-project=ubuntu-os-cloud \
  --boot-disk-size=30GB \
  --boot-disk-type=pd-standard
```
Expected: インスタンスが作成される

**Always Free の条件:** `e2-micro` かつ **us-west1 / us-central1 / us-east1** のいずれか、標準永続ディスク 30GB まで。それ以外は課金される

- [ ] **Step 2: swap を 2GB 作成** (GCP 上)

```bash
sudo fallocate -l 2G /swapfile && sudo chmod 600 /swapfile
sudo mkswap /swapfile && sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
free -h
```
Expected: `Swap:` の行に 2.0Gi が出る

- [ ] **Step 3: tor と torsocks を導入** (GCP 上)

Run: `sudo apt-get update && sudo apt-get install -y tor torsocks nginx`

- [ ] **Step 4: torrc を作成**

```bash
mkdir -p infra/deploy/gcp
cat > infra/deploy/gcp/torrc <<'EOF'
# Anarchy GCP 用 torrc
#
# GCP はチェーン 1 台と公開 nginx のみ。さくらへは全て outbound で繋ぐので
# hidden service は不要 (さくらから GCP を dial する必要が無い)。

DataDirectory /var/lib/tor
RunAsDaemon 0
Log notice syslog

SocksPort 127.0.0.1:9050 IsolateClientAddr IsolateSOCKSAuth

ClientUseIPv6 1
ClientPreferIPv6ORPort 0
SafeLogging 1
EOF
```

- [ ] **Step 5: 適用して確認** (GCP 上)

```bash
sudo cp infra/deploy/gcp/torrc /etc/tor/torrc
sudo -u debian-tor tor --verify-config -f /etc/tor/torrc
sudo systemctl restart tor && sleep 10
ss -tlnp | grep 9050
```
Expected: `Configuration was valid` と `127.0.0.1:9050` の listen

- [ ] **Step 6: さくらの onion に到達できるか確認** (GCP 上)

Run: `torsocks curl -s -m 45 -o /dev/null -w '%{http_code}\n' http://$SAKURA_STORAGE_ONION:3030/metrics`
Expected: `200`

**これが通らなければ以降は無意味なので、ここで止めて原因を潰すこと**

- [ ] **Step 7: コミット**

```bash
git add infra/deploy/gcp/torrc
git commit -m "chore(deploy): add GCP torrc (SOCKS only)

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: GCP — チェーンノード

1 台なので `--tor-mode=forced` の enforcement をそのまま使える。

**Files:**
- Create: `infra/deploy/gcp/anarchy-chain.service`

**Interfaces:**
- Consumes: さくらの chain onion、chain-1 の peer ID
- Produces: `127.0.0.1:9944` で RPC を提供し、さくらと同期しているノード

- [ ] **Step 1: systemd ユニットを作成**

```bash
cat > infra/deploy/gcp/anarchy-chain.service <<'EOF'
[Unit]
Description=Anarchy chain node (GCP)
After=network-online.target tor.service
Wants=network-online.target
Requires=tor.service

[Service]
Type=simple
User=anarchy
Group=anarchy
WorkingDirectory=/opt/anarchy
EnvironmentFile=/opt/anarchy/env/chain.env

# 1 台のみなので forced の enforcement をそのまま使える。
#   ① Outbound Lock: torsocks 配下でないと起動を拒否
#   ② Inbound Lock:  listen を /ip4/127.0.0.1/tcp/30333 に強制
Environment=ANARCHY_RUNNING_UNDER_TORSOCKS=1
ExecStart=/usr/bin/torsocks /opt/anarchy/bin/anarchy-node \
  --chain /opt/anarchy/anarchy-portfolio-raw.json \
  --base-path /var/lib/anarchy/chain \
  --node-key-file /var/lib/anarchy/chain/node-key \
  --rpc-port 9944 \
  --rpc-cors ${RPC_CORS} \
  --tor-mode forced \
  --bootnodes ${BOOTNODE}

Restart=on-failure
RestartSec=10
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
```

- [ ] **Step 2: 環境ファイルを配置** (GCP 上)

```bash
sudo useradd -r -s /usr/sbin/nologin anarchy 2>/dev/null || true
sudo mkdir -p /opt/anarchy/env /var/lib/anarchy/chain
head -c 32 /dev/urandom | xxd -p -c 32 | sudo tee /var/lib/anarchy/chain/node-key > /dev/null

sudo tee /opt/anarchy/env/chain.env > /dev/null <<'EOF'
RPC_CORS=https://<フロントのドメイン>
BOOTNODE=/dns4/<SAKURA_CHAIN_ONION>/tcp/30333/p2p/<PEER1>
EOF
sudo chown -R anarchy:anarchy /var/lib/anarchy /opt/anarchy/env
```

`/dns4/<onion>/` を使う理由: sc-network の transport は `/onion3/` を dial できない (`build_transport` は DNS(TCP)+WS のみ) が、torsocks が `getaddrinfo` を乗っ取って `.onion` を仮想 IP にマップするため `/dns4/` 経由なら到達できる見込み。**未検証なので Step 4 で確認し、駄目なら Step 5 のフォールバックを使う**

- [ ] **Step 3: 起動して forced のログを確認** (GCP 上)

```bash
sudo cp infra/deploy/gcp/anarchy-chain.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl start anarchy-chain
sleep 40
sudo journalctl -u anarchy-chain -n 80 --no-pager | grep -E "Tor mode|Inbound Lock|Outbound Lock|Local node identity|Syncing|Imported"
```
Expected:
- `🔒 Tor mode: FORCED - Full anonymity enabled`
- `🔒 ① Outbound Lock: All traffic via Tor (torsocks detected)`
- `🔒 ② Inbound Lock: Listening on 127.0.0.1:30333 only`

`Tor mode FORCED requires running under torsocks` で失敗する場合は `Environment=ANARCHY_RUNNING_UNDER_TORSOCKS=1` が効いていない

- [ ] **Step 4: さくらと同期しているか確認** (GCP 上)

```bash
curl -s -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"system_health","params":[]}' \
  http://127.0.0.1:9944 | python3 -m json.tool
free -h
```
Expected: `peers` が 1 以上、`isSyncing` が false に落ち着く。**メモリも確認する** (swap を食い潰していないこと)

- [ ] **Step 5: 同期しない場合のフォールバック** (GCP 上)

`peers: 0` のままなら `/dns4/<onion>/` が dial できていない。socat でローカル TCP に落とす:

```bash
sudo apt-get install -y socat
socat TCP-LISTEN:30333,bind=127.0.0.2,reuseaddr,fork \
      SOCKS4A:127.0.0.1:$SAKURA_CHAIN_ONION:30333,socksport=9050 &
sudo sed -i 's|BOOTNODE=.*|BOOTNODE=/ip4/127.0.0.2/tcp/30333/p2p/<PEER1>|' /opt/anarchy/env/chain.env
sudo systemctl restart anarchy-chain
```

torsocks が localhost 宛を遮断する場合は `/etc/tor/torsocks.conf` に `AllowOutboundLocalhost 1` を追加。socat も systemd 化すること

- [ ] **Step 6: GCP からさくらのストレージへの到達を確認** (GCP 上)

```bash
for p in 3030 3031 3032; do
  echo -n "$p: "
  sudo -u anarchy torsocks curl -s -m 45 -o /dev/null -w '%{http_code}\n' \
    http://$SAKURA_STORAGE_ONION:$p/metrics
done
```
Expected: 3 つとも `200`。**これがこの構成の核心** (GCP のチェーンがさくらのストレージに直接 fan-out する)

- [ ] **Step 7: enable してコミット**

```bash
sudo systemctl enable anarchy-chain
git add infra/deploy/gcp/anarchy-chain.service
git commit -m "chore(deploy): add GCP chain node (forced tor mode, sakura as bootnode)

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 9: GCP — nginx で公開 wss エンドポイント

フロントの接続先。**Cloudflare Tunnel は使わない** — CF はアイドルの WebSocket を閉じ、そのタイムアウトは非公開かつ Enterprise でしか変更できない。PAPI の ws-provider は受信側のウォッチドッグだけで **ping を送らない** ([default-provider.mjs](../../../apps/frontend/src/lib/chain-client.ts) のコメント参照) ため、ブロック間隔の隙間で接続が本当にアイドルになり切断される。

**Files:**
- Create: `infra/deploy/gcp/nginx-anarchy.conf`

**Interfaces:**
- Consumes: GCP のチェーンノード `127.0.0.1:9944`
- Produces: `wss://<domain>/rpc`

- [ ] **Step 1: nginx 設定を作成**

```bash
cat > infra/deploy/gcp/nginx-anarchy.conf <<'EOF'
# Anarchy GCP — 公開 wss エンドポイント
#
# フロントは Cloudflare Workers 上の別オリジンなので cross-origin になる。
# チェーンノード側に --rpc-cors https://<フロントのドメイン> が必要。

server {
    listen 80;
    server_name <DOMAIN>;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name <DOMAIN>;

    ssl_certificate     /etc/letsencrypt/live/<DOMAIN>/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/<DOMAIN>/privkey.pem;

    location /rpc {
        proxy_pass http://127.0.0.1:9944;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;

        # 必須: デフォルト 60s では PoW のブロック間隔で WS が切れる。
        # chain-client.ts が heartbeatTimeout=300s にしているのと同じ理由
        # (ブロック間隔は指数分布で 40s 超のギャップが日常的に発生する)。
        proxy_read_timeout  3600s;
        proxy_send_timeout  3600s;
    }

    location / {
        return 404;
    }
}
EOF
```

- [ ] **Step 2: ファイアウォールを開ける**

```bash
gcloud compute firewall-rules create anarchy-https \
  --allow tcp:80,tcp:443 --target-tags=anarchy-web
gcloud compute instances add-tags anarchy-gcp --tags=anarchy-web --zone=us-west1-b
```

- [ ] **Step 3: 証明書を取得** (GCP 上)

```bash
sudo apt-get install -y certbot python3-certbot-nginx
sudo certbot --nginx -d <DOMAIN>
```
Expected: 証明書が発行される。事前に DNS の A レコードを GCP の外部 IP に向けておくこと

- [ ] **Step 4: 設定を適用** (GCP 上)

```bash
sudo cp infra/deploy/gcp/nginx-anarchy.conf /etc/nginx/sites-available/anarchy
sudo sed -i "s/<DOMAIN>/$DOMAIN/g" /etc/nginx/sites-available/anarchy
sudo ln -sf /etc/nginx/sites-available/anarchy /etc/nginx/sites-enabled/
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t && sudo systemctl reload nginx
```
Expected: `syntax is ok` / `test is successful`

- [ ] **Step 5: WS が張れることを確認**

```bash
curl -s -i -N -H "Connection: Upgrade" -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  https://<DOMAIN>/rpc | head -5
```
Expected: `HTTP/1.1 101 Switching Protocols`

- [ ] **Step 6: コミット**

```bash
git add infra/deploy/gcp/nginx-anarchy.conf
git commit -m "chore(deploy): add GCP nginx config for public wss endpoint

proxy_read_timeout 3600s は PoW のブロック間隔で WS が切れるのを防ぐ。
Cloudflare Tunnel を使わない理由: CF はアイドル WebSocket を閉じ、その
タイムアウトは Enterprise でしか変更できない。PAPI は ping を送らないため
ブロック間隔の隙間で接続がアイドルになる。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 10: AWS — ストレージノード

3 プロバイダ目。ストレージノードは実測 20MB なので t3.micro (1GB) に余裕で収まる。

**Files:**
- Create: `infra/deploy/aws/torrc`
- Create: `infra/deploy/aws/anarchy-storage.service`

**Interfaces:**
- Produces: `http://<aws-storage-onion>:3030` として自己登録するストレージノード

- [ ] **Step 1: インスタンスを作成**

EC2 で `t3.micro` / Ubuntu 24.04 LTS / 10GB gp3 を起動する。セキュリティグループは **22 番のみ** (ストレージノードは Tor 経由でのみ公開する)。

**無料枠は 12 ヶ月で失効する。** 期限をカレンダーに登録すること

- [ ] **Step 2: torrc を作成**

```bash
mkdir -p infra/deploy/aws
cat > infra/deploy/aws/torrc <<'EOF'
# Anarchy AWS 用 torrc (ストレージノード 1 台)

DataDirectory /var/lib/tor
RunAsDaemon 0
Log notice syslog

SocksPort 127.0.0.1:9050 IsolateClientAddr IsolateSOCKSAuth

# チェーンノード (さくら / GCP) がここに fan-out する
HiddenServiceDir /var/lib/tor/anarchy-storage
HiddenServiceVersion 3
HiddenServicePort 3030 127.0.0.1:3030

ClientUseIPv6 1
ClientPreferIPv6ORPort 0
SafeLogging 1
EOF
```

- [ ] **Step 3: systemd ユニットを作成**

```bash
cat > infra/deploy/aws/anarchy-storage.service <<'EOF'
[Unit]
Description=Anarchy storage node (AWS)
After=network-online.target tor.service
Wants=network-online.target
Requires=tor.service

[Service]
Type=simple
User=anarchy
Group=anarchy
WorkingDirectory=/opt/anarchy
EnvironmentFile=/opt/anarchy/env/storage.env

Environment=ANARCHY_RUNNING_UNDER_TORSOCKS=1
ExecStart=/usr/bin/torsocks /opt/anarchy/bin/anarchy-storage-node \
  --config /opt/anarchy/env/storage.toml \
  --data-dir /var/lib/anarchy/storage \
  --rpc-port 3030 \
  --public-url ${PUBLIC_URL} \
  --chain-url ${CHAIN_URL}

Restart=on-failure
RestartSec=10
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
```

- [ ] **Step 4: 設定を配置して起動** (AWS 上)

```bash
sudo apt-get update && sudo apt-get install -y tor torsocks
sudo cp infra/deploy/aws/torrc /etc/tor/torrc
sudo systemctl restart tor && sleep 15
AWS_STORAGE_ONION=$(sudo cat /var/lib/tor/anarchy-storage/hostname)
echo "AWS storage onion: $AWS_STORAGE_ONION"

sudo useradd -r -s /usr/sbin/nologin anarchy 2>/dev/null || true
sudo mkdir -p /opt/anarchy/env /var/lib/anarchy/storage
sudo tee /opt/anarchy/env/storage.toml > /dev/null <<EOF
capacity = 5368709120
declare_rate_limit = 10
auth_enabled = true
dev_mode = false
listen_addr = "/ip4/127.0.0.1/tcp/4001"
signer_seed = "$(openssl rand -hex 32)"
EOF
sudo tee /opt/anarchy/env/storage.env > /dev/null <<EOF
PUBLIC_URL=http://$AWS_STORAGE_ONION:3030
CHAIN_URL=ws://<GCP のチェーンに torsocks で届く経路、または さくら chain onion>
EOF
sudo chown -R anarchy:anarchy /var/lib/anarchy /opt/anarchy/env
sudo chmod 600 /opt/anarchy/env/storage.toml

sudo cp infra/deploy/aws/anarchy-storage.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl start anarchy-storage
sleep 20
sudo journalctl -u anarchy-storage -n 40 --no-pager | grep -E "Registering|Registered|HTTP RPC"
```
Expected: `Registering with blockchain node url=http://<aws-onion>:3030`

**`CHAIN_URL` について:** storage-node は subxt (WS) でチェーンに繋ぐ。torsocks 配下なので `ws://<sakura-chain-onion>:9944` を指定すれば到達できるはず。ここも未検証なので、繋がらなければログを見て切り分けること

- [ ] **Step 5: 登録が全チェーンノードに伝播したか確認**

さくら (9944/9945/9946) と GCP (9944) の全てで AWS のエンドポイントが見えること
Expected: `http://<aws-onion>:3030` が 4 ノードすべてのレジストリに現れる

- [ ] **Step 6: enable してコミット**

```bash
sudo systemctl enable anarchy-storage
git add infra/deploy/aws/torrc infra/deploy/aws/anarchy-storage.service
git commit -m "chore(deploy): add AWS storage node (3rd provider)

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 11: Cloudflare Workers — フロントのデプロイ

静的 export は `6b3b947` で対応済み。Workers Static Assets に載せるだけ。

**Files:**
- Create: `apps/frontend/wrangler.jsonc`

**Interfaces:**
- Consumes: `apps/frontend/out/` (静的 export の出力)、GCP の `wss://<domain>/rpc`
- Produces: 公開フロント URL

- [ ] **Step 1: wrangler 設定を作成**

```bash
cat > apps/frontend/wrangler.jsonc <<'EOF'
{
  // Anarchy フロント — Workers Static Assets
  //
  // このアプリは src/app/ が 3 ファイルだけの実質シングルページ SPA で、
  // サーバランタイムを必要としない。よって vinext / OpenNext は不要で、
  // next build (output: 'export') の出力をそのまま配信する。
  // 静的アセット配信は課金対象外。
  "name": "anarchy-frontend",
  "compatibility_date": "2026-09-11",
  "assets": {
    "directory": "./out",
    "not_found_handling": "single-page-application"
  }
}
EOF
```

- [ ] **Step 2: wasm を再ビルドして依存を入れ直す**

```bash
cd packages/wasm-engine && wasm-pack build --target web --out-dir pkg
cd ../.. && pnpm install
```

**必須:** `anarchy-wasm-engine` は `file:` 依存でコピーされるため、`wasm-pack` の後に `pnpm install` しないと古い wasm が使われる

- [ ] **Step 3: 接続先を焼き込んでビルド**

```bash
NEXT_PUBLIC_CHAIN_RPC_URL=wss://<GCP のドメイン>/rpc \
  pnpm --filter @anarchy/frontend build
```
Expected: `out/` が生成され、全ルートが `○ (Static)` になる

**`NEXT_PUBLIC_*` はビルド時に焼き込まれる。** 付け忘れると `ws://127.0.0.1:9944` のままになる

- [ ] **Step 4: 焼き込まれた URL を検証**

Run: `grep -ro "wss://[^\"']*" apps/frontend/out/_next/static/chunks/ | head -3`
Expected: GCP のドメインが現れる。`127.0.0.1` が出る場合は Step 3 をやり直す

- [ ] **Step 5: デプロイ**

```bash
cd apps/frontend && npx wrangler deploy
```
Expected: `https://anarchy-frontend.<subdomain>.workers.dev` が払い出される

- [ ] **Step 6: CORS を通す**

GCP のチェーンノードの `--rpc-cors` に払い出された URL を設定する:

```bash
# GCP 上
sudo sed -i 's|RPC_CORS=.*|RPC_CORS=https://anarchy-frontend.<subdomain>.workers.dev|' /opt/anarchy/env/chain.env
sudo systemctl restart anarchy-chain
```

- [ ] **Step 7: コミット**

```bash
git add apps/frontend/wrangler.jsonc
git commit -m "chore(deploy): add wrangler config for Workers Static Assets

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 12: ブラウザからの E2E 検証

投稿が GCP のチェーンを経由して さくら/AWS のストレージに格納され、読み戻せることを確認する。この計画で唯一「全部繋がったこと」を証明するタスク。

**Files:** なし (検証のみ)

- [ ] **Step 1: フロントを開く**

払い出された Workers URL にアクセスする
Expected: ページが表示され、DevTools の Network で `wss://<GCPドメイン>/rpc` が `101` で確立している

- [ ] **Step 2: ブロックが進むことを確認**

Expected: ブロック番号が増えていく (さくらの chain-1 が採掘している)

- [ ] **Step 3: ウォレット作成と faucet**

シードフレーズを生成し、faucet で MORAL を受け取る
Expected: 残高が反映される (純オンチェーン処理なのでストレージ不要)

- [ ] **Step 4: テキスト投稿**

Expected: エラーなく完了する

**`No Storage Nodes connected` が出る場合:** GCP のチェーンがストレージを認識していない。Task 8 Step 6 に戻る

- [ ] **Step 5: サーバ側で fan-out を確認**

```bash
# GCP 上
sudo journalctl -u anarchy-chain -n 50 --no-pager | grep -E "Fragment uploaded|Failed to upload|No Storage Nodes"
# さくら / AWS 上
sudo journalctl -u anarchy-storage@1 -n 50 --no-pager | grep -iE "store|fragment"
```
Expected: GCP 側に `Fragment uploaded to Storage Node`。断片の配置は merkle_root 依存なので、さくら 3 台と AWS 1 台のいずれかに現れる

- [ ] **Step 6: リロードして読み戻せることを確認**

Expected: 投稿本文が表示される (= GCP のチェーンが別プロバイダのストレージから断片を取得して復元できている)

- [ ] **Step 7: メモリを確認** (GCP 上)

Run: `free -h && sudo journalctl -u anarchy-chain -n 20 --no-pager | grep -i "oom\|killed"`
Expected: swap を食い潰していない、OOM killer が動いていない

- [ ] **Step 8: 画像添付を試す**

小さめの画像 (1-2MB) を添付して投稿し、表示できることを確認する。Step 7 のメモリ確認を再度実行する
Expected: 投稿・表示ともに成功し、OOM が起きない

---

### Task 13: ドキュメント

**Files:**
- Create: `docs/operations/deployment-multi-provider.md`
- Modify: `docs/operations/tor-connection-patterns.md`

- [ ] **Step 1: 手順書を作成**

`docs/operations/deployment-multi-provider.md` に以下を書く:

1. トポロジ図 (4 プロバイダと通信経路)
2. 前提 (ドメイン、証明書、各社アカウント、Ubuntu 24.04)
3. 各ホストの構築手順 (Task 3-11 の要約)
4. ローカル接続 (下記 Step 2)
5. トラブルシューティング (Task 6/8/10/12 で実際に踏んだ事象)
6. 既知の制約 (下記 Step 3)
7. 運用メモ (AWS 無料枠の失効日、GCP Always Free の条件)

- [ ] **Step 2: ローカル接続の 3 パターンを書く**

```markdown
## ローカルからの接続

### パターン A: フルノード型 (フロント + チェーン)

    ANARCHY_RUNNING_UNDER_TORSOCKS=1 torsocks ./target/release/anarchy-node \
      --chain infra/deploy/anarchy-portfolio-raw.json \
      --base-path ~/.anarchy/chain \
      --tor-mode forced \
      --bootnodes /dns4/<SAKURA_CHAIN_ONION>/tcp/30333/p2p/<PEER1>

    NEXT_PUBLIC_CHAIN_RPC_URL=ws://127.0.0.1:9944 pnpm dev:frontend

NAT の内側でも hidden service 不要 (こちらから dial するだけ)。
`peers: 0` が続く場合は socat トンネル経由に切り替える。

### パターン B: フロントのみローカル

    NEXT_PUBLIC_CHAIN_RPC_URL=wss://<GCPドメイン>/rpc pnpm dev:frontend

### パターン C: チェーンのみローカル

パターン A のチェーン部分のみ。
```

- [ ] **Step 3: 既知の制約を書く**

```markdown
## 既知の制約

- **`--tor-mode=forced` は listen を `/ip4/127.0.0.1/tcp/30333` に決め打ちする。**
  同一ホストで複数チェーンノードを forced にすると衝突するため、さくらの 3 台は
  `outbound-only` + 明示的な `--listen-addr` を使っている
- **`X-Chain-Auth` は timestamp とメソッド名しか署名していない** (nonce も body
  ハッシュも無い)。窓の間は replay 可能。チェーン↔ストレージが Tor 経由なので
  下回りで緩和されているが、session-token 方式への移行が宿題
- **reqwest に `socks` feature が無い**ため torsocks (LD_PRELOAD) に依存している
- **GCP e2-micro は 1GB。** chain-node が 525MB を使い、断片は base64 `String` と
  して全量メモリに載る (`MAX_FRAGMENT_SIZE` = 128MB → base64 で約 171MB)。
  swap 2GB で緩和しているが、大きな添付で OOM する可能性がある
- **AWS 無料枠は 12 ヶ月で失効する**
- **GCP Always Free は us-west1 / us-central1 / us-east1 限定**なので、日本からは
  RPC 往復に 120-150ms 乗る
- **フロントは clearnet のみ。** GCP のノードは全訪問者の生 IP を見る。これは
  匿名 SNS 本来の脅威モデルではなく、公開デモとしての割り切り
```

- [ ] **Step 4: 実測値を埋める**

onion アドレス、peer ID、ブロック間隔、メモリ使用量を反映する。**プレースホルダを残さないこと**

- [ ] **Step 5: tor-connection-patterns.md に §2.4 を追記**

```markdown
### 2.4 マルチプロバイダ分散型 (本番デプロイ)

    ユーザー ──HTTPS──▶ Cloudflare Workers (フロント/静的)
                              │ wss
                              ▼
                         GCP e2-micro ──────Tor──────▶ さくら
                         chain×1                        chain×3 + storage×3
                         nginx                     └──▶ AWS t3.micro
                                                        storage×1

**特徴**:
- GCP のチェーンが さくら/AWS のストレージに **直接** fan-out する
  (さくらのチェーンを中継しない)。エンドポイントはオンチェーンと gossip の
  二重経路で伝播する
- サーバ間は全て torsocks + hidden service。公開ポートは GCP の 443 のみ
- さくらは公開ポートを一切開けない

**手順**: [deployment-multi-provider.md](deployment-multi-provider.md)
```

- [ ] **Step 6: §2.3 に訂正を入れる**

```markdown
> **注記 (2026-09)**: 上記の Next.js API Routes + SocksProxyAgent 案は未実装。
> App Router の route handler は WebSocket upgrade をサポートせず、PAPI は WS
> 前提なので `fetch` + agent の例は流用できない。また静的 SPA が「サーバ必須」
> になり静的ホスティングができなくなる。中継が必要な場合はアプリの外
> (nginx + socat) に置くこと。
```

- [ ] **Step 7: 未使用依存を削除**

```bash
pnpm --filter @anarchy/frontend remove socks-proxy-agent
pnpm --filter @anarchy/frontend build   # 影響が無いことを確認
pnpm --filter @anarchy/frontend test
```
Expected: ビルドとテストが通る (どこからも import されていない)

- [ ] **Step 8: コミット**

```bash
git add docs/operations/ apps/frontend/package.json pnpm-lock.yaml
git commit -m "docs(operations): add multi-provider deployment runbook

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**1. Requirements coverage**

| 要件 | 対応タスク |
|---|---|
| さくら: chain×3 + storage×3 + 採掘、公開ポートなし | Task 3, 4, 5 |
| GCP: chain×1 + 公開 wss | Task 7, 8, 9 |
| AWS: storage×1 | Task 10 |
| Cloudflare: フロント | Task 11 |
| Tor (torsocks 方式) | Task 3, 4, 5, 7, 8, 10 (全 ExecStart が torsocks 経由) |
| フロント clearnet のみ | Task 9, 11 (Onion-Location なし) |
| プライベートネットワーク不使用 | Task 5, 10 (onion で広告)、Task 8 Step 6 (到達確認) |
| ローカル接続 (任意の構成) | Task 13 Step 2 |

**2. 未検証として明示したもの**

- Task 8 Step 2: `/dns4/<onion>/` による libp2p dial は未検証。Step 5 に socat フォールバックを用意
- Task 10 Step 4: storage-node の subxt が torsocks 経由で `.onion` の WS に繋がるかは未検証
- Task 5 Step 4: レジストリ照会の RPC 名は `rpc_methods` で確認する手順を併記

**3. 型・値の一貫性**

- `--public-url` / `Config::advertised_url()` は `7fc943c` の実装と一致
- ポート割り当ては Global Constraints と Task 4/5/8/9/10 で一致
- onion アドレスの受け渡し (Task 3 → 5, 6, 8; Task 10 → 全ノード) が一貫している

**4. スコープ外 (意図的)**

- reqwest への `socks` feature 追加 (torsocks 方式を採用)
- `--tor-mode=forced` のポート決め打ち修正 (既知の制約として記載、別 PR 相当)
- `X-Chain-Auth` の session-token 化 (同上)
- 採掘のハッシュレート上限設定 (現状 29.7% で許容)
- コンテナ化 (全ホスト x86_64 で揃うためバイナリ配布で足りる)

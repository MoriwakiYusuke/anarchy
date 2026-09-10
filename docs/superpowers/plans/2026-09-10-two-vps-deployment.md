# Anarchy 2-VPS デプロイ実装計画

> ⚠️ **SUPERSEDED (2026-09-11)** — この計画は
> [2026-09-11-multi-provider-deployment.md](2026-09-11-multi-provider-deployment.md)
> に置き換えられた。事業者選定・無料枠調査・Cloudflare フロントの検討を経て、
> 「VPS 2 台」から「さくら + GCP + AWS + Cloudflare の 4 プロバイダ」構成に変わった。
> 本計画の Task 1 (storage-node `--public-url`) は実装済み (`7fc943c`)。
> 履歴として残すが、これを実行しないこと。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** VPS 2 台に Anarchy を本番デプロイし、VPS B のフロント経由とローカル経由の 2 経路からアクセスできる状態にする。

**Architecture:** VPS A がチェーンノード 3 台 + ストレージノード 5 台を持ち、VPS B がフロントとチェーンノード 1 台を持つ。B のチェーンはオンチェーン/gossip の二重ディスカバリで A のストレージノードを知り、**直接 HTTP で fan-out** する（A のチェーンを中継しない）。サーバ間通信は全て Tor hidden service 経由で、各プロセスは torsocks 配下で起動する。フロントの入口のみ clearnet HTTPS。

**Tech Stack:** Polkadot SDK stable2503 (PoW/RandomX), Rust storage-node (libp2p + axum), Next.js 16 + PAPI, Tor (hidden service + SOCKS5), nginx, systemd

**Spec:** 本計画内の「Requirements」節（独立した spec ファイルは無く、要件はここに内包する）

---

## Requirements

| 項目 | 決定 |
|---|---|
| VPS A | チェーンノード 3 台 + ストレージノード 5 台 |
| VPS B | フロント + チェーンノード 1 台 |
| アクセス経路 | ① VPS B のフロント（clearnet HTTPS） ② ローカル（任意の構成） |
| Tor | 有効。サーバ間は全て hidden service 経由 |
| Tor 実現方式 | torsocks で包む（reqwest の socks feature は使わない） |
| フロント公開形態 | clearnet のみ（Onion-Location / フロント用 hidden service は作らない） |
| ネットワーク | プライベートネットワークは使わない。全てグローバル到達可能なアドレスで通信 |

## Global Constraints

- **チェーンスペック id に `"mainnet"` を含めない。** [command.rs:290](../../../apps/blockchain/node/src/command.rs) が id の部分文字列一致で `--tor-mode` を `Forced` に強制上書きする。A の 3 ノードが Forced になると次項の理由で衝突する
- **`--tor-mode=forced` は listen を `/ip4/127.0.0.1/tcp/30333` に決め打ちする**（[command.rs:156](../../../apps/blockchain/node/src/command.rs)）。ポートも固定なので、**同一ホストで複数チェーンノードを Forced で動かすと bind が衝突する**。A の 3 ノードは `outbound-only` + 明示的な `--listen-addr /ip4/127.0.0.1/tcp/<port>` で同等の効果を得る。B の単一ノードのみ `forced` を使う
- **採掘は 1 ノードのみ**（`chain-a1`）。低難易度で複数ノードが同時採掘すると reorg が頻発する。`--randomx-mode light`（256MB）を使う
- **Substrate の release ビルドを VPS 上で行わない。** ローカルでビルドしてバイナリを転送する
- **既存データとの互換性は考慮しない**（CLAUDE.md Compatibility Policy）。スキーマ変更時は破棄して再生成する
- コメント・ドキュメントは日本語。コードは Rust / TypeScript
- ポート割り当て:
  - VPS A chain: p2p `30333/30334/30335`, rpc `9944/9945/9946`, prometheus `9615/9616/9617`
  - VPS A storage: http rpc `3030-3034`, libp2p `4001-4005`
  - VPS B chain: p2p `30333`, rpc `9944`
  - VPS B frontend `3000`, nginx `80/443`
  - Tor SOCKS `9050`（両ホスト）

## File Structure

| ファイル | 責務 |
|---|---|
| `apps/storage-node/src/config/mod.rs` | `public_url` 設定フィールドの追加（外部広告 URL） |
| `apps/storage-node/src/main.rs` | `--public-url` CLI 引数と登録時の URL 決定ロジック |
| `infra/deploy/vps-a/torrc` | A 側 Tor 設定（chain 3 + storage 5 の hidden service） |
| `infra/deploy/vps-a/anarchy-chain@.service` | A のチェーンノード systemd テンプレートユニット |
| `infra/deploy/vps-a/anarchy-storage@.service` | A のストレージノード systemd テンプレートユニット |
| `infra/deploy/vps-a/storage-N.toml` | ストレージノード個別設定（5 本） |
| `infra/deploy/vps-b/torrc` | B 側 Tor 設定（SOCKS のみ、hidden service なし） |
| `infra/deploy/vps-b/anarchy-chain.service` | B のチェーンノード systemd ユニット |
| `infra/deploy/vps-b/anarchy-frontend.service` | Next.js の systemd ユニット |
| `infra/deploy/vps-b/nginx-anarchy.conf` | clearnet HTTPS + `/rpc` の WS プロキシ |
| `docs/operations/deployment-2vps.md` | デプロイ手順書とローカル接続 runbook |
| `docs/operations/tor-connection-patterns.md` | §2.4「1 台の VPS に全部載せて外部公開」パターンの追記 |

---

### Task 1: storage-node の広告 URL を設定可能にする

現状 `register_with_blockchain` に渡す URL が `http://127.0.0.1:<port>` にハードコードされており、他ホストのチェーンノードから到達できない。この構成の前提が崩れるので最優先で直す。

**Files:**
- Modify: `apps/storage-node/src/config/mod.rs`
- Modify: `apps/storage-node/src/main.rs:172-180`, `apps/storage-node/src/main.rs:265-275`
- Test: `apps/storage-node/src/config/mod.rs`（既存の `#[cfg(test)] mod tests` に追加）

**Interfaces:**
- Consumes: 既存の `Config` 構造体と `ConfigOverrides`
- Produces:
  - `Config::public_url: Option<String>` — 外部に広告する URL。`None` なら `http://127.0.0.1:{rpc_port}` を使う
  - `Config::advertised_url(&self) -> String` — 広告 URL を解決して返す
  - `ConfigOverrides::public_url: Option<String>`
  - CLI: `--public-url <URL>`

- [ ] **Step 1: 失敗するテストを書く**

`apps/storage-node/src/config/mod.rs` の `mod tests` に追加:

```rust
#[test]
fn advertised_url_defaults_to_loopback() {
    let mut config = Config::default();
    config.rpc_port = 3030;
    config.public_url = None;
    assert_eq!(config.advertised_url(), "http://127.0.0.1:3030");
}

#[test]
fn advertised_url_uses_public_url_when_set() {
    let mut config = Config::default();
    config.rpc_port = 3030;
    config.public_url = Some("http://abc123.onion:3030".to_string());
    assert_eq!(config.advertised_url(), "http://abc123.onion:3030");
}

#[test]
fn public_url_override_wins_over_config_file() {
    let mut config = Config::default();
    config.public_url = Some("http://from-file.onion:3030".to_string());
    let overrides = ConfigOverrides {
        data_dir: None,
        chain_url: None,
        listen_addr: None,
        public_url: Some("http://from-cli.onion:3030".to_string()),
    };
    config.apply_overrides(overrides);
    assert_eq!(config.advertised_url(), "http://from-cli.onion:3030");
}
```

- [ ] **Step 2: テストが失敗することを確認**

Run: `cd apps/storage-node && cargo test --lib config:: 2>&1 | tail -20`
Expected: FAIL — `no field 'public_url' on type 'Config'` でコンパイルエラー

- [ ] **Step 3: Config に public_url と advertised_url を実装**

`apps/storage-node/src/config/mod.rs` の `Config` 構造体に追加:

```rust
    /// 他ホストのチェーンノードに広告する外部到達可能な URL。
    /// 未設定なら `http://127.0.0.1:{rpc_port}` にフォールバックする。
    /// 例: `http://<onion>:3030` / `https://s1.example.com:3030`
    #[serde(default)]
    pub public_url: Option<String>,
```

`impl Config` に追加:

```rust
    /// チェーンノードへの登録時に広告する URL を解決する。
    pub fn advertised_url(&self) -> String {
        self.public_url
            .clone()
            .unwrap_or_else(|| format!("http://127.0.0.1:{}", self.rpc_port))
    }
```

`ConfigOverrides` に追加:

```rust
    pub public_url: Option<String>,
```

`apply_overrides`（既存の `if let Some(chain_url) = overrides.chain_url` の並び）に追加:

```rust
        if let Some(public_url) = overrides.public_url {
            config.public_url = Some(public_url);
        }
```

`Config::default()` の初期化に `public_url: None,` を追加。

- [ ] **Step 4: テストが通ることを確認**

Run: `cd apps/storage-node && cargo test --lib config:: 2>&1 | tail -20`
Expected: PASS（3 テストとも）

- [ ] **Step 5: CLI 引数と登録処理を配線**

`apps/storage-node/src/main.rs` の `Args` に追加:

```rust
    /// 外部に広告する URL (overrides config)。他ホストのチェーンノードから
    /// 到達できるアドレスを指定する。例: http://<onion>:3030
    #[arg(long)]
    pub public_url: Option<String>,
```

`ConfigOverrides` の構築に追加:

```rust
        public_url: args.public_url.clone(),
```

登録処理（`main.rs:172` 付近）を差し替え:

```rust
    // Register with blockchain node (auto-connection)
    // 他ホストのチェーンノードも同じ URL で fan-out するため、
    // loopback ではなく外部到達可能なアドレスを広告する必要がある。
    let our_rpc_url = config.advertised_url();
    info!(url = %our_rpc_url, "Registering with blockchain node");
    match chain_client.register_with_blockchain(&our_rpc_url).await {
```

ハートビート再登録（`main.rs:265` 付近の `heartbeat_url`）も同じ値を使うよう修正する。`let heartbeat_url = config.advertised_url();` を spawn の外で定義して move する。

- [ ] **Step 6: ビルドと全テスト**

Run: `cd apps/storage-node && cargo build --release && cargo test --lib 2>&1 | tail -20`
Expected: ビルド成功、既存テストが 1 件も壊れていない

- [ ] **Step 7: 起動ログで広告 URL を確認**

Run:
```bash
cd apps/storage-node && ./target/release/anarchy-storage-node \
  --data-dir /tmp/anarchy-test-storage --rpc-port 3030 \
  --public-url http://example.onion:3030 2>&1 | head -30
```
Expected: `Registering with blockchain node url=http://example.onion:3030` が出る（チェーン未起動なので登録自体は失敗して warn になるが、URL が正しく解決されていればよい）

- [ ] **Step 8: コミット**

```bash
git add apps/storage-node/src/config/mod.rs apps/storage-node/src/main.rs
git commit -m "feat(storage-node): add --public-url for cross-host endpoint advertisement

登録時の URL が http://127.0.0.1:<port> にハードコードされており、
他ホストのチェーンノードから fan-out できなかった。
public_url 設定と --public-url CLI 引数を追加し、未設定時のみ
loopback にフォールバックする。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: チェーンスペックの生成と固定

4 ノード全てが同一 genesis を共有する必要がある。`--chain local` はコードから決定的に生成されるが、「本番」として名前を付け、id に `"mainnet"` が入らないことを保証するため raw chainspec を作って配布する。

**Files:**
- Create: `infra/deploy/anarchy-portfolio-raw.json`（生成物、git 管理する）

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
spec["id"] = "anarchy_portfolio"   # "mainnet" を含めないこと (Global Constraints 参照)
spec["chainType"] = "Local"        # allow_private=true になるが今回は使わない。Live との差は endpoint_policy のみ
json.dump(spec, open(p, "w"), indent=2)
print(spec["id"], spec["chainType"])
PY
```
Expected: `anarchy_portfolio Local` と出力される

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
Expected: raw JSON が生成される

- [ ] **Step 6: raw spec でノードが起動することを確認**

Run:
```bash
cd apps/blockchain && timeout 30 ./target/release/anarchy-node \
  --chain ../../infra/deploy/anarchy-portfolio-raw.json \
  --base-path /tmp/anarchy-spec-test --tmp 2>&1 | head -30
```
Expected: `Chain specification: Anarchy Portfolio` が出て、パニックせずに起動する

- [ ] **Step 7: コミット**

```bash
git add infra/deploy/anarchy-portfolio-raw.json
git commit -m "chore(deploy): add anarchy-portfolio raw chainspec

4 ノードが共有する genesis を固定する。id は anarchy_portfolio
("mainnet" を含めると --tor-mode が forced に強制上書きされ、
VPS A の複数チェーンノードが 30333 で衝突する)。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: バイナリのビルドと転送スクリプト

Substrate の release ビルドは VPS のメモリでは厳しいので、ローカルでビルドして転送する。glibc の差異を避けるため VPS と同じディストリでビルドする。

**Files:**
- Create: `infra/deploy/build-and-ship.sh`

**Interfaces:**
- Produces: `build-and-ship.sh <host> <role>` — `role` は `a` または `b`。必要なバイナリと chainspec を対象ホストの `/opt/anarchy/` に配置する

- [ ] **Step 1: スクリプトを作成**

```bash
cat > infra/deploy/build-and-ship.sh <<'EOF'
#!/usr/bin/env bash
# ローカルでビルドしたバイナリと chainspec を VPS に転送する。
#
# 使い方:
#   ./build-and-ship.sh user@vps-a a
#   ./build-and-ship.sh user@vps-b b
#
# VPS 上でのビルドは避けること (Substrate の release ビルドは 16GB 級の
# メモリを要求する)。glibc の差異を避けるため、VPS と同じディストリで
# ビルドすること。
set -euo pipefail

HOST="${1:?usage: $0 <user@host> <a|b>}"
ROLE="${2:?usage: $0 <user@host> <a|b>}"
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

echo "==> building anarchy-node"
( cd "$REPO_ROOT/apps/blockchain" && cargo build --release --bin anarchy-node )

if [ "$ROLE" = "a" ]; then
    echo "==> building anarchy-storage-node"
    ( cd "$REPO_ROOT/apps/storage-node" && cargo build --release --bin anarchy-storage-node )
fi

echo "==> creating /opt/anarchy on $HOST"
ssh "$HOST" 'sudo mkdir -p /opt/anarchy/bin && sudo chown -R "$USER" /opt/anarchy'

echo "==> shipping anarchy-node"
scp "$REPO_ROOT/apps/blockchain/target/release/anarchy-node" "$HOST:/opt/anarchy/bin/"

if [ "$ROLE" = "a" ]; then
    echo "==> shipping anarchy-storage-node"
    scp "$REPO_ROOT/apps/storage-node/target/release/anarchy-storage-node" "$HOST:/opt/anarchy/bin/"
fi

echo "==> shipping chainspec"
scp "$REPO_ROOT/infra/deploy/anarchy-portfolio-raw.json" "$HOST:/opt/anarchy/"

echo "==> done"
ssh "$HOST" 'ls -la /opt/anarchy /opt/anarchy/bin'
EOF
chmod +x infra/deploy/build-and-ship.sh
```

- [ ] **Step 2: shellcheck で構文を検証**

Run: `shellcheck infra/deploy/build-and-ship.sh || bash -n infra/deploy/build-and-ship.sh`
Expected: エラーなし（shellcheck 未インストールなら `bash -n` の構文チェックのみ）

- [ ] **Step 3: 引数なしで実行してエラーメッセージを確認**

Run: `./infra/deploy/build-and-ship.sh 2>&1 | head -3; echo "exit=$?"`
Expected: `usage: ... <user@host> <a|b>` が表示され、非ゼロ終了する

- [ ] **Step 4: コミット**

```bash
git add infra/deploy/build-and-ship.sh
git commit -m "chore(deploy): add build-and-ship script for VPS binary distribution

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: VPS A の Tor 設定

A のチェーン 3 台とストレージ 5 台を hidden service で公開する。同一ホストなので onion は 2 本（chain 用 / storage 用）にまとめ、仮想ポートで分ける。

**Files:**
- Create: `infra/deploy/vps-a/torrc`

**Interfaces:**
- Produces:
  - `/var/lib/tor/anarchy-a-chain/hostname` — チェーン用 onion。仮想ポート `30333/30334/30335`（p2p）と `9944/9945/9946`（RPC）
  - `/var/lib/tor/anarchy-a-storage/hostname` — ストレージ用 onion。仮想ポート `3030-3034`
  - SOCKS5 `127.0.0.1:9050`

- [ ] **Step 1: torrc を作成**

```bash
mkdir -p infra/deploy/vps-a
cat > infra/deploy/vps-a/torrc <<'EOF'
# Anarchy VPS A 用 torrc (systemd の tor.service が読む /etc/tor/torrc に配置)
#
# A はチェーンノード 3 台 + ストレージノード 5 台を持つ。
# onion は用途ごとに 2 本にまとめ、仮想ポートでノードを分ける。
# (同一ホストに同居している事実は隠せないので、5 本に分ける利点が無い)

DataDirectory /var/lib/tor
RunAsDaemon 0
Log notice syslog

# ---- SOCKS ----
# チェーンノード / ストレージノードが torsocks 経由で外に出るために使う。
SocksPort 127.0.0.1:9050 IsolateClientAddr IsolateSOCKSAuth

# ---- Hidden Service: チェーンノード 3 台 ----
# p2p は VPS B / ローカルからの bootnode 接続を受ける。
# RPC は運用確認用 (フロントは B 側の RPC を使うので必須ではない)。
HiddenServiceDir /var/lib/tor/anarchy-a-chain
HiddenServiceVersion 3
HiddenServicePort 30333 127.0.0.1:30333
HiddenServicePort 30334 127.0.0.1:30334
HiddenServicePort 30335 127.0.0.1:30335
HiddenServicePort 9944  127.0.0.1:9944
HiddenServicePort 9945  127.0.0.1:9945
HiddenServicePort 9946  127.0.0.1:9946

# ---- Hidden Service: ストレージノード 5 台 ----
# VPS B のチェーンノードがここに直接 fan-out する (gossip/オンチェーンで
# エンドポイントを知り、チェーン A を中継せず直接叩く)。
HiddenServiceDir /var/lib/tor/anarchy-a-storage
HiddenServiceVersion 3
HiddenServicePort 3030 127.0.0.1:3030
HiddenServicePort 3031 127.0.0.1:3031
HiddenServicePort 3032 127.0.0.1:3032
HiddenServicePort 3033 127.0.0.1:3033
HiddenServicePort 3034 127.0.0.1:3034

# ---- 安全側のチューニング ----
ClientUseIPv6 1
ClientPreferIPv6ORPort 0
SafeLogging 1
EOF
```

- [ ] **Step 2: 設定ファイルの構文を検証**

Run（VPS A 上で実行）:
```bash
sudo cp infra/deploy/vps-a/torrc /etc/tor/torrc
sudo -u debian-tor tor --verify-config -f /etc/tor/torrc
```
Expected: `Configuration was valid` と出る

- [ ] **Step 3: tor を起動して onion アドレスを取得**

Run（VPS A 上）:
```bash
sudo systemctl restart tor && sleep 15
sudo cat /var/lib/tor/anarchy-a-chain/hostname
sudo cat /var/lib/tor/anarchy-a-storage/hostname
```
Expected: 56 文字 + `.onion` の文字列が 2 つ表示される。**両方を控えること**（以降のタスクで使う）

- [ ] **Step 4: SOCKS が listen していることを確認**

Run（VPS A 上）: `ss -tlnp | grep 9050`
Expected: `127.0.0.1:9050` で listen している行が出る

- [ ] **Step 5: コミット**

```bash
git add infra/deploy/vps-a/torrc
git commit -m "chore(deploy): add VPS A torrc (3 chain + 5 storage hidden services)

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: VPS A のチェーンノード 3 台

`--tor-mode=forced` は listen を `/ip4/127.0.0.1/tcp/30333` に決め打ちするため、同一ホストで 3 台動かすと衝突する。`outbound-only` + 明示的な `--listen-addr` で同じ効果（localhost のみ listen + onion 経由 inbound）を得る。

**Files:**
- Create: `infra/deploy/vps-a/anarchy-chain@.service`

**Interfaces:**
- Consumes: `/opt/anarchy/bin/anarchy-node`, `/opt/anarchy/anarchy-portfolio-raw.json`（Task 2, 3）
- Produces:
  - systemd テンプレートユニット。`anarchy-chain@1` `@2` `@3` として起動
  - `chain-a1` のみ採掘する（`ANARCHY_MINE=1` を環境ファイルで指定）
  - 各ノードの peer ID は `--node-key-file` で固定される

- [ ] **Step 1: systemd テンプレートユニットを作成**

```bash
cat > infra/deploy/vps-a/anarchy-chain@.service <<'EOF'
[Unit]
Description=Anarchy chain node %i (VPS A)
After=network-online.target tor.service
Wants=network-online.target
Requires=tor.service

[Service]
Type=simple
User=anarchy
Group=anarchy
WorkingDirectory=/opt/anarchy

# ノードごとの設定 (ポート・採掘有無・coinbase) を読む
EnvironmentFile=/opt/anarchy/env/chain-%i.env

# torsocks 配下で起動する。
#   - outbound: 全て Tor 経由 (ANARCHY_RUNNING_UNDER_TORSOCKS=1 を立てる)
#   - inbound: --listen-addr を 127.0.0.1 に明示して hidden service 経由のみにする
#
# --tor-mode=forced を使わない理由: forced は listen を
# /ip4/127.0.0.1/tcp/30333 に決め打ちするため、同一ホストで 3 台動かすと
# bind が衝突する (docs/superpowers/plans/2026-09-10-two-vps-deployment.md
# の Global Constraints 参照)。
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

- [ ] **Step 2: ノードごとの環境ファイルを作成（VPS A 上）**

```bash
sudo mkdir -p /opt/anarchy/env /var/lib/anarchy/chain-{1,2,3}
sudo useradd -r -s /usr/sbin/nologin anarchy 2>/dev/null || true

# node-key を生成して peer ID を固定する
for i in 1 2 3; do
  head -c 32 /dev/urandom | xxd -p -c 32 | sudo tee /var/lib/anarchy/chain-$i/node-key > /dev/null
done

# chain-1: 唯一の採掘ノード
sudo tee /opt/anarchy/env/chain-1.env > /dev/null <<'EOF'
P2P_PORT=30333
RPC_PORT=9944
PROM_PORT=9615
MINING_ARGS=--mine --coinbase 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY --randomx-mode light
BOOTNODE_ARGS=
EOF

# chain-2 / chain-3: 同期のみ (採掘は 1 台に絞る。複数採掘は reorg を招く)
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

sudo chown -R anarchy:anarchy /var/lib/anarchy
```

`--coinbase` の SS58 は自分のアカウントに差し替えること。

- [ ] **Step 3: chain-1 を起動して peer ID を取得**

Run（VPS A 上）:
```bash
sudo cp infra/deploy/vps-a/anarchy-chain@.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl start anarchy-chain@1
sleep 20
sudo journalctl -u anarchy-chain@1 -n 60 --no-pager | grep -E "Local node identity|Tor mode|Chain specification"
```
Expected:
- `Chain specification: Anarchy Portfolio`
- `⚠️  Tor mode: OUTBOUND-ONLY` の警告（意図通り。inbound は `--listen-addr` で localhost に閉じている）
- `Local node identity is: 12D3Koo...` — **この peer ID を控える**

- [ ] **Step 4: chain-2 / chain-3 の環境ファイルに peer ID を反映して起動**

```bash
PEER1=<Step 3 で得た peer ID>
sudo sed -i "s/PLACEHOLDER_PEER_ID_1/$PEER1/" /opt/anarchy/env/chain-2.env /opt/anarchy/env/chain-3.env
sudo systemctl start anarchy-chain@2 anarchy-chain@3
sleep 25
```

- [ ] **Step 5: 3 台が互いに繋がっていることを確認**

Run（VPS A 上）:
```bash
for p in 9944 9945 9946; do
  echo -n "rpc $p peers: "
  curl -s -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"system_health","params":[]}' \
    http://127.0.0.1:$p | python3 -c 'import sys,json;print(json.load(sys.stdin)["result"])'
done
```
Expected: 3 つとも応答し、`peers` が 1 以上（chain-1 は 2、chain-2/3 は 1 以上）

- [ ] **Step 6: chain-1 がブロックを生成していることを確認**

Run（VPS A 上）: `sudo journalctl -u anarchy-chain@1 -n 100 --no-pager | grep -E "🏆|Submitted valid seal|Imported"`
Expected: `🏆 Submitted valid seal at nonce ...` が出ており、ブロック番号が増えている

- [ ] **Step 7: 3 台が同じブロックに追従していることを確認**

Run（VPS A 上）:
```bash
for p in 9944 9945 9946; do
  echo -n "rpc $p best: "
  curl -s -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"chain_getHeader","params":[]}' \
    http://127.0.0.1:$p | python3 -c 'import sys,json;print(int(json.load(sys.stdin)["result"]["number"],16))'
done
```
Expected: 3 つの番号が一致するか、差が 1-2 以内

- [ ] **Step 8: enable してコミット**

```bash
# VPS A 上
sudo systemctl enable anarchy-chain@1 anarchy-chain@2 anarchy-chain@3

# ローカル
git add infra/deploy/vps-a/anarchy-chain@.service
git commit -m "chore(deploy): add VPS A chain node systemd template (3 nodes)

forced モードは listen を 127.0.0.1:30333 に決め打ちして同一ホストの
複数ノードで衝突するため、outbound-only + 明示的 --listen-addr で
同等の効果を得る。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: VPS A のストレージノード 5 台

Task 1 で追加した `--public-url` を使い、**onion アドレスで登録する**。これで VPS B のチェーンノードからも同じ URL で到達できる。

**Files:**
- Create: `infra/deploy/vps-a/anarchy-storage@.service`
- Create: `infra/deploy/vps-a/storage-template.toml`

**Interfaces:**
- Consumes: `/opt/anarchy/bin/anarchy-storage-node`（Task 3）、`Config::public_url`（Task 1）、A の storage onion（Task 4）
- Produces: 5 台のストレージノードが `http://<a-storage-onion>:303X` として自己登録する

- [ ] **Step 1: systemd テンプレートユニットを作成**

```bash
cat > infra/deploy/vps-a/anarchy-storage@.service <<'EOF'
[Unit]
Description=Anarchy storage node %i (VPS A)
After=network-online.target tor.service anarchy-chain@1.service
Wants=network-online.target
Requires=tor.service

[Service]
Type=simple
User=anarchy
Group=anarchy
WorkingDirectory=/opt/anarchy

EnvironmentFile=/opt/anarchy/env/storage-%i.env

# torsocks 配下で起動する。
# storage-node -> chain の登録は subxt (jsonrpsee WS) を使っており、
# SOCKS プロキシを差す口が無いため torsocks が必須。
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

- [ ] **Step 2: config テンプレートを作成**

```bash
cat > infra/deploy/vps-a/storage-template.toml <<'EOF'
# Anarchy ストレージノード設定テンプレート (VPS A)
# storage-1.toml .. storage-5.toml として rpc_port / listen_addr を変えて配置する。
#
# public_url は systemd の --public-url が上書きするのでここでは書かない。
# chain_url も同様に systemd 側で指定する。

capacity = 10737418240          # 10 GiB
declare_rate_limit = 10
auth_enabled = true
dev_mode = false
EOF
```

- [ ] **Step 3: ノードごとの設定を VPS A に配置**

```bash
# VPS A 上。<A_STORAGE_ONION> は Task 4 Step 3 で控えた値
A_STORAGE_ONION=<xxxxx.onion>

for i in 1 2 3 4 5; do
  port=$((3029 + i))       # 3030..3034
  p2p=$((4000 + i))        # 4001..4005
  sudo mkdir -p /var/lib/anarchy/storage-$i

  sudo tee /opt/anarchy/env/storage-$i.toml > /dev/null <<EOF
capacity = 10737418240
declare_rate_limit = 10
auth_enabled = true
dev_mode = false
listen_addr = "/ip4/127.0.0.1/tcp/$p2p"
EOF

  sudo tee /opt/anarchy/env/storage-$i.env > /dev/null <<EOF
RPC_PORT=$port
PUBLIC_URL=http://$A_STORAGE_ONION:$port
CHAIN_URL=ws://127.0.0.1:9944
EOF
done

sudo chown -R anarchy:anarchy /var/lib/anarchy /opt/anarchy/env
```

- [ ] **Step 4: 起動して登録ログを確認**

Run（VPS A 上）:
```bash
sudo cp infra/deploy/vps-a/anarchy-storage@.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl start anarchy-storage@{1,2,3,4,5}
sleep 20
sudo journalctl -u anarchy-storage@1 -n 40 --no-pager | grep -E "Registering|Registered|HTTP RPC server"
```
Expected:
- `HTTP RPC server started addr=0.0.0.0:3030`
- `Registering with blockchain node url=http://<onion>:3030`
- `Registered with blockchain node`

**重要:** `url=http://127.0.0.1:3030` と出る場合は Task 1 の変更が効いていない。バイナリが古い可能性があるので Task 3 の転送をやり直す。

- [ ] **Step 5: チェーン側のレジストリに 5 台が載っていることを確認**

Run（VPS A 上）:
```bash
curl -s -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"storage_listNodes","params":[]}' \
  http://127.0.0.1:9944 | python3 -m json.tool
```
Expected: 5 件のエントリが返り、全ての `endpoint` が `http://<onion>:303X` 形式（`127.0.0.1` が 1 件も無いこと）

RPC 名が異なる場合は `curl -s -d '{"jsonrpc":"2.0","id":1,"method":"rpc_methods","params":[]}' http://127.0.0.1:9944` で `storage_` 系のメソッド名を確認する。

- [ ] **Step 6: gossip で chain-2 / chain-3 にも伝播していることを確認**

Run（VPS A 上）: Step 5 と同じ curl を `9945` と `9946` に対して実行
Expected: 同じ 5 件が返る（gossip 伝播、あるいはオンチェーン経由のフォールバック）

- [ ] **Step 7: enable してコミット**

```bash
# VPS A 上
sudo systemctl enable anarchy-storage@{1,2,3,4,5}

# ローカル
git add infra/deploy/vps-a/anarchy-storage@.service infra/deploy/vps-a/storage-template.toml
git commit -m "chore(deploy): add VPS A storage node systemd template (5 nodes)

--public-url で onion アドレスを広告し、VPS B のチェーンノードからも
同じ URL で fan-out できるようにする。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: VPS A 内での投稿フロー疎通確認

B を作る前に、A 単体でチェーン→ストレージの fan-out が通ることを確認する。ここで失敗するなら B からは絶対に通らない。

**Files:** なし（検証のみ）

**Interfaces:**
- Consumes: Task 5, 6 の稼働中サービス

- [ ] **Step 1: chain-1 から storage への到達を確認**

Run（VPS A 上）:
```bash
sudo -u anarchy torsocks curl -s -m 30 -o /dev/null -w '%{http_code}\n' \
  http://$A_STORAGE_ONION:3030/metrics
```
Expected: `200`

**失敗する場合の切り分け:**
- `000` かタイムアウト → tor の hidden service が publish しきっていない。`sudo journalctl -u tor -n 50` を確認し、2-3 分待って再試行
- torsocks のエラー → `/etc/tor/torrc` の `SocksPort 127.0.0.1:9050` と `/etc/tor/torsocks.conf` の `TorPort 9050` が一致しているか確認

- [ ] **Step 2: 直接 localhost でも到達することを確認（対照実験）**

Run（VPS A 上）: `curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:3030/metrics`
Expected: `200`（これが失敗するならストレージノード自体が死んでいる）

- [ ] **Step 3: チェーン RPC 経由でフラグメント往復を試す**

Run（VPS A 上）:
```bash
curl -s -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"rpc_methods","params":[]}' \
  http://127.0.0.1:9944 | python3 -c 'import sys,json;print([m for m in json.load(sys.stdin)["result"]["methods"] if m.startswith("storage_")])'
```
Expected: `storage_uploadFragment` / `storage_getFragment` / `storage_registerEndpoint` などが列挙される

- [ ] **Step 4: ノード 0 台エラーが出ないことを確認**

Run（VPS A 上）: 既存の統合テストがあれば流す。
```bash
ls apps/blockchain/tests/integration/
```
Expected: 該当するシェルテストがあれば実行し、`No Storage Nodes connected` が出ないことを確認する

- [ ] **Step 5: 記録**

この時点の `storage_listNodes` の出力と各ノードの peer ID を手元にメモしておく（Task 8 以降で使う）。

---

### Task 8: VPS B のチェーンノード

B のチェーンは 1 台なので `--tor-mode=forced` の enforcement をそのまま使える。A の chain onion を bootnode に指定して同期する。

**Files:**
- Create: `infra/deploy/vps-b/torrc`
- Create: `infra/deploy/vps-b/anarchy-chain.service`

**Interfaces:**
- Consumes: A の chain onion（Task 4）、chain-1 の peer ID（Task 5 Step 3）
- Produces: B のチェーンノードが `127.0.0.1:9944` で RPC を提供し、A と同期している

- [ ] **Step 1: B の torrc を作成**

```bash
mkdir -p infra/deploy/vps-b
cat > infra/deploy/vps-b/torrc <<'EOF'
# Anarchy VPS B 用 torrc
#
# B はチェーン 1 台とフロントのみ。A へは全て outbound で繋ぐので
# hidden service は不要 (A から B を dial する必要が無い)。

DataDirectory /var/lib/tor
RunAsDaemon 0
Log notice syslog

SocksPort 127.0.0.1:9050 IsolateClientAddr IsolateSOCKSAuth

ClientUseIPv6 1
ClientPreferIPv6ORPort 0
SafeLogging 1
EOF
```

- [ ] **Step 2: systemd ユニットを作成**

```bash
cat > infra/deploy/vps-b/anarchy-chain.service <<'EOF'
[Unit]
Description=Anarchy chain node (VPS B)
After=network-online.target tor.service
Wants=network-online.target
Requires=tor.service

[Service]
Type=simple
User=anarchy
Group=anarchy
WorkingDirectory=/opt/anarchy

EnvironmentFile=/opt/anarchy/env/chain.env

# B はチェーン 1 台なので forced の enforcement をそのまま使える。
#   ① Outbound Lock: torsocks 配下でないと起動を拒否する
#   ② Inbound Lock: listen を /ip4/127.0.0.1/tcp/30333 に強制する
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

- [ ] **Step 3: 環境ファイルを配置（VPS B 上）**

```bash
sudo useradd -r -s /usr/sbin/nologin anarchy 2>/dev/null || true
sudo mkdir -p /opt/anarchy/env /var/lib/anarchy/chain
head -c 32 /dev/urandom | xxd -p -c 32 | sudo tee /var/lib/anarchy/chain/node-key > /dev/null

# <A_CHAIN_ONION> と <PEER1> は Task 4 / Task 5 で控えた値
sudo tee /opt/anarchy/env/chain.env > /dev/null <<'EOF'
RPC_CORS=https://<フロントのドメイン>
BOOTNODE=/dns4/<A_CHAIN_ONION>/tcp/30333/p2p/<PEER1>
EOF

sudo chown -R anarchy:anarchy /var/lib/anarchy /opt/anarchy/env
```

`/dns4/<onion>/tcp/...` を使う理由: sc-network の transport は `/onion3/` を dial できない（`build_transport` は DNS(TCP) + WS のみ）が、torsocks が `getaddrinfo` を乗っ取って `.onion` を仮想 IP にマップするため、`/dns4/` 経由なら到達できる可能性が高い。**これは未検証なので次のステップで確認し、駄目なら Step 6 のフォールバックを使う。**

- [ ] **Step 4: 起動して forced モードのログを確認**

Run（VPS B 上）:
```bash
sudo cp infra/deploy/vps-b/torrc /etc/tor/torrc
sudo cp infra/deploy/vps-b/anarchy-chain.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl restart tor && sleep 10
sudo systemctl start anarchy-chain
sleep 30
sudo journalctl -u anarchy-chain -n 80 --no-pager | grep -E "Tor mode|Inbound Lock|Outbound Lock|Local node identity|Syncing|Imported"
```
Expected:
- `🔒 Tor mode: FORCED - Full anonymity enabled`
- `🔒 ① Outbound Lock: All traffic via Tor (torsocks detected)`
- `🔒 ② Inbound Lock: Listening on 127.0.0.1:30333 only`

`Tor mode FORCED requires running under torsocks` で起動失敗する場合は `Environment=ANARCHY_RUNNING_UNDER_TORSOCKS=1` が効いていない。

- [ ] **Step 5: A と同期しているか確認**

Run（VPS B 上）:
```bash
curl -s -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"system_health","params":[]}' \
  http://127.0.0.1:9944 | python3 -m json.tool
```
Expected: `"peers": 1` 以上。`"isSyncing": false` になり、ブロック番号が A と一致していく

- [ ] **Step 6: 同期しない場合のフォールバック（socat トンネル）**

Step 5 で `peers: 0` のままなら、`/dns4/<onion>/` が dial できていない。socat でローカル TCP に落とす:

```bash
# VPS B 上。既存スクリプトを使う
/opt/anarchy/onion-proxy.sh <A_CHAIN_ONION> 30333 30333 &
# → 127.0.0.2:30333 で listen する

# bootnode を差し替え
sudo sed -i 's|BOOTNODE=.*|BOOTNODE=/ip4/127.0.0.2/tcp/30333/p2p/<PEER1>|' /opt/anarchy/env/chain.env
sudo systemctl restart anarchy-chain
```

この場合 torsocks が localhost 宛の接続を遮断する可能性があるので、`/etc/tor/torsocks.conf` に `AllowOutboundLocalhost 1` を追加する。socat 自体も systemd 化すること。

- [ ] **Step 7: enable してコミット**

```bash
# VPS B 上
sudo systemctl enable anarchy-chain

# ローカル
git add infra/deploy/vps-b/torrc infra/deploy/vps-b/anarchy-chain.service
git commit -m "chore(deploy): add VPS B chain node (forced tor mode, A as bootnode)

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 9: VPS B から A のストレージへの到達確認

この計画の核心。B のチェーンが A のストレージに**直接** fan-out できることを確認する。

**Files:** なし（検証のみ）

**Interfaces:**
- Consumes: Task 6（A のストレージが onion で登録済み）、Task 8（B がチェーン同期済み）

- [ ] **Step 1: B のチェーンがストレージノードを認識しているか確認**

Run（VPS B 上）:
```bash
curl -s -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"storage_listNodes","params":[]}' \
  http://127.0.0.1:9944 | python3 -m json.tool
```
Expected: A の 5 台が `http://<a-storage-onion>:303X` として列挙される

**0 件の場合:** チェーン同期が完了していないか、gossip が届いていない。`system_health` でブロック番号が A に追いついているか確認する。オンチェーンにエンドポイントが載っていれば同期だけで見えるはず。

- [ ] **Step 2: B から A のストレージに torsocks で直接到達できるか確認**

Run（VPS B 上）:
```bash
sudo -u anarchy torsocks curl -s -m 30 -o /dev/null -w '%{http_code}\n' \
  http://<A_STORAGE_ONION>:3030/metrics
```
Expected: `200`

これが通れば、B のチェーンノード（同じく torsocks 配下）からも到達できる。

- [ ] **Step 3: 5 台全てに到達できることを確認**

Run（VPS B 上）:
```bash
for p in 3030 3031 3032 3033 3034; do
  echo -n "$p: "
  sudo -u anarchy torsocks curl -s -m 30 -o /dev/null -w '%{http_code}\n' \
    http://<A_STORAGE_ONION>:$p/metrics
done
```
Expected: 5 つとも `200`

- [ ] **Step 4: fan-out が実際に動くことをログで確認**

Run（VPS B 上、Task 11 の投稿後に再確認）:
```bash
sudo journalctl -u anarchy-chain -n 100 --no-pager | grep -E "Fragment uploaded|Failed to upload|No Storage Nodes"
```
Expected: `No Storage Nodes connected` が出ていないこと

- [ ] **Step 5: 記録**

到達確認の結果を `docs/operations/deployment-2vps.md`（Task 12）に転記するためメモしておく。

---

### Task 10: VPS B のフロントと nginx

clearnet HTTPS で公開する。フロントは静的 SPA だが `output: 'export'` の検証を避けるため `next start` を nginx の背後に置く。

**Files:**
- Create: `infra/deploy/vps-b/anarchy-frontend.service`
- Create: `infra/deploy/vps-b/nginx-anarchy.conf`

**Interfaces:**
- Consumes: Task 8（B のチェーン RPC が `127.0.0.1:9944`）
- Produces: `https://<domain>/` でフロント、`wss://<domain>/rpc` でチェーン RPC

- [ ] **Step 1: フロントの systemd ユニットを作成**

```bash
cat > infra/deploy/vps-b/anarchy-frontend.service <<'EOF'
[Unit]
Description=Anarchy frontend (Next.js)
After=network-online.target anarchy-chain.service
Wants=network-online.target

[Service]
Type=simple
User=anarchy
Group=anarchy
WorkingDirectory=/opt/anarchy/frontend
Environment=NODE_ENV=production
Environment=PORT=3000
ExecStart=/usr/bin/node_modules/.bin/next start
# pnpm で入れている場合は ExecStart=/usr/bin/pnpm start に置き換える
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF
```

- [ ] **Step 2: nginx 設定を作成**

```bash
cat > infra/deploy/vps-b/nginx-anarchy.conf <<'EOF'
# Anarchy VPS B — clearnet HTTPS 入口
#
# / は Next.js、/rpc は同一ホストのチェーンノード RPC に WS プロキシする。
# same-origin にすることで CORS の設定が不要になる。

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

    # チェーンノード RPC (WebSocket)
    location /rpc {
        proxy_pass http://127.0.0.1:9944;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;

        # 必須: デフォルトの 60s では PoW のブロック間隔で WS が切れる。
        # chain-client.ts が heartbeatTimeout=300s にしているのと同じ理由
        # (ブロック間隔は指数分布で 40s 超のギャップが日常的に発生する)。
        proxy_read_timeout  3600s;
        proxy_send_timeout  3600s;
    }

    # フロント
    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
EOF
```

- [ ] **Step 3: フロントをビルドして転送**

Run（ローカル）:
```bash
cd packages/wasm-engine && wasm-pack build --target web --out-dir pkg
cd ../.. && pnpm install    # file: 依存はコピーなので wasm-pack 後は必須
NEXT_PUBLIC_CHAIN_RPC_URL=wss://<DOMAIN>/rpc pnpm --filter @anarchy/frontend build
```
Expected: `.next/` が生成される。`NEXT_PUBLIC_*` はビルド時に焼き込まれるので、この環境変数を付け忘れると `ws://127.0.0.1:9944` のままになる

- [ ] **Step 4: 転送して起動**

```bash
# ローカル
rsync -az --exclude node_modules apps/frontend/ <user>@<vps-b>:/opt/anarchy/frontend/
ssh <user>@<vps-b> 'cd /opt/anarchy/frontend && pnpm install --prod'

# VPS B 上
sudo cp infra/deploy/vps-b/anarchy-frontend.service /etc/systemd/system/
sudo cp infra/deploy/vps-b/nginx-anarchy.conf /etc/nginx/sites-available/anarchy
sudo sed -i "s/<DOMAIN>/$DOMAIN/g" /etc/nginx/sites-available/anarchy
sudo ln -sf /etc/nginx/sites-available/anarchy /etc/nginx/sites-enabled/
sudo systemctl daemon-reload
sudo systemctl start anarchy-frontend
sudo nginx -t && sudo systemctl reload nginx
```

- [ ] **Step 5: nginx の設定検証**

Run（VPS B 上）: `sudo nginx -t`
Expected: `syntax is ok` / `test is successful`

- [ ] **Step 6: HTTPS でフロントが返ることを確認**

Run: `curl -s -o /dev/null -w '%{http_code}\n' https://<DOMAIN>/`
Expected: `200`

- [ ] **Step 7: WS が張れることを確認**

Run:
```bash
curl -s -i -N -H "Connection: Upgrade" -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  https://<DOMAIN>/rpc | head -5
```
Expected: `HTTP/1.1 101 Switching Protocols`

- [ ] **Step 8: enable してコミット**

```bash
# VPS B 上
sudo systemctl enable anarchy-frontend

# ローカル
git add infra/deploy/vps-b/anarchy-frontend.service infra/deploy/vps-b/nginx-anarchy.conf
git commit -m "chore(deploy): add VPS B frontend systemd unit and nginx config

/rpc を same-origin で WS プロキシし CORS を回避する。
proxy_read_timeout 3600s は PoW のブロック間隔で WS が切れるのを防ぐため。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 11: ブラウザからの E2E 検証

投稿がチェーン B を経由してストレージ A に格納され、読み戻せることをブラウザで確認する。この計画で唯一「全部繋がったこと」を証明するタスク。

**Files:** なし（検証のみ）

**Interfaces:**
- Consumes: Task 1-10 の全成果物

- [ ] **Step 1: ブラウザでフロントを開く**

`https://<DOMAIN>/` にアクセスする。
Expected: ページが表示され、DevTools の Network タブで `wss://<DOMAIN>/rpc` の WebSocket が `101` で確立している

- [ ] **Step 2: ウォレットを作成してブロック進行を確認**

シードフレーズを生成しアカウントを作る。
Expected: ブロック番号が UI 上で増えていく（chain-a1 が採掘している）

- [ ] **Step 3: faucet で MORAL を受け取る**

Expected: 残高が反映される（純オンチェーン処理なのでストレージ不要）

- [ ] **Step 4: 投稿する**

短いテキストを投稿する。
Expected: エラーなく完了する

**`No Storage Nodes connected` が出る場合:** Task 9 Step 1 に戻る。B のチェーンがストレージを認識していない。

- [ ] **Step 5: 投稿がストレージに届いたことをサーバ側で確認**

Run（VPS B 上）: `sudo journalctl -u anarchy-chain -n 50 --no-pager | grep "Fragment uploaded"`
Expected: `Fragment uploaded to Storage Node: root=..., index=..., size=...` が出る

Run（VPS A 上）: `sudo journalctl -u anarchy-storage@1 -n 50 --no-pager | grep -iE "store|fragment"`
Expected: 断片受信のログが出る（どのノードに配置されるかは merkle_root 依存なので 1-5 のいずれか）

- [ ] **Step 6: リロードして投稿が読み戻せることを確認**

ブラウザをリロードしてタイムラインを表示する。
Expected: 投稿本文が表示される（= B のチェーンが A のストレージから断片を取得して復元できている）

- [ ] **Step 7: 失敗時の切り分け表を記録**

Step 1-6 のどこで落ちたかと、その時のログを Task 12 の runbook に記載する。

---

### Task 12: デプロイ手順書とローカル接続 runbook

「ローカル」は任意の構成を許すので、3 パターンを手順として書く。

**Files:**
- Create: `docs/operations/deployment-2vps.md`

**Interfaces:**
- Consumes: Task 1-11 の実測値（onion アドレス、peer ID、ポート）

- [ ] **Step 1: 手順書の骨子を作成**

`docs/operations/deployment-2vps.md` に以下の節を作る:

1. トポロジ図（VPS A / VPS B / ローカルの 3 者と通信経路）
2. 前提（ドメイン、証明書、VPS スペック、torsocks インストール）
3. VPS A 構築手順（Task 4-7 の要約）
4. VPS B 構築手順（Task 8, 10 の要約）
5. ローカル接続（3 パターン、下記 Step 2）
6. トラブルシューティング（Task 7 / 9 / 11 で実際に踏んだ事象）
7. 既知の制約

- [ ] **Step 2: ローカル接続の 3 パターンを書く**

```markdown
## ローカルからの接続

### パターン A: フルノード型（フロント + チェーン）

自分のマシンで frontend と chain-node を動かし、VPS A と P2P 同期する。
NAT の内側でも hidden service 不要（こちらから dial するだけ）。

    # A のチェーン onion へ同期
    ANARCHY_RUNNING_UNDER_TORSOCKS=1 torsocks ./target/release/anarchy-node \
      --chain infra/deploy/anarchy-portfolio-raw.json \
      --base-path ~/.anarchy/chain \
      --tor-mode forced \
      --bootnodes /dns4/<A_CHAIN_ONION>/tcp/30333/p2p/<PEER1>

    # フロントは自分のチェーンを向く
    NEXT_PUBLIC_CHAIN_RPC_URL=ws://127.0.0.1:9944 pnpm dev:frontend

`peers: 0` のままなら onion-proxy.sh で socat トンネルを立てて
`/ip4/127.0.0.2/tcp/30333/p2p/<PEER1>` を bootnode にする。

### パターン B: フロントのみローカル

チェーンは VPS B のものを使う。UI をいじりたいだけならこれが最速。

    NEXT_PUBLIC_CHAIN_RPC_URL=wss://<DOMAIN>/rpc pnpm dev:frontend

### パターン C: チェーンのみローカル

同期状態や RPC を確認したいだけの場合。パターン A のチェーン部分のみ。
```

- [ ] **Step 3: 既知の制約を書く**

```markdown
## 既知の制約

- **`--tor-mode=forced` は listen を `/ip4/127.0.0.1/tcp/30333` に決め打ちする。**
  同一ホストで複数チェーンノードを forced で動かすと衝突するため、VPS A の
  3 台は `outbound-only` + 明示的な `--listen-addr` を使っている。
  forced が `--port` を尊重するようになれば解消できる
- **`X-Chain-Auth` は timestamp とメソッド名しか署名していない**（nonce も
  body ハッシュも無い）。窓の間は replay 可能。今回はチェーン↔ストレージが
  Tor 経由なので下回りで緩和されているが、session-token 方式への移行が宿題
- **reqwest に `socks` feature が無い**ため、torsocks に依存している。
  明示的なプロキシ指定に移行すれば LD_PRELOAD 依存を外せる
- **フロントは clearnet のみ。** ゲートウェイである VPS B は全訪問者の
  生 IP を見る。これは匿名 SNS としての本来の脅威モデルではなく、
  公開デモとしての割り切り
```

- [ ] **Step 4: 実測値を埋める**

Task 4-11 で得た onion アドレス、peer ID、ブロック時間などを手順書に反映する。
**プレースホルダのまま残さないこと。**

- [ ] **Step 5: コミット**

```bash
git add docs/operations/deployment-2vps.md
git commit -m "docs(operations): add 2-VPS deployment runbook

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 13: tor-connection-patterns.md の更新

既存ドキュメントは §2.2「全部ローカル」と §2.3「フロント公開・ノードは隠しサーバ」の 2 択で、今回の構成が抜けている。また §2.3 の SocksProxyAgent 案は実装されておらず、実際には採らない方針なので訂正する。

**Files:**
- Modify: `docs/operations/tor-connection-patterns.md`

**Interfaces:**
- Consumes: Task 12 の手順書（相互リンクする）

- [ ] **Step 1: §2.4 として今回の構成を追記**

`docs/operations/tor-connection-patterns.md` の §2.3 の後に追加:

```markdown
### 2.4 2-VPS 分散型（本番デプロイ）

    ユーザー ──HTTPS──▶ VPS B ──────Tor──────▶ VPS A
                        フロント              チェーン×3
                        チェーン×1            ストレージ×5

**用途**: 実運用 / ポートフォリオ公開

**特徴**:
- VPS B のチェーンは A のストレージに **直接** fan-out する
  （A のチェーンを中継しない）。エンドポイントはオンチェーンと gossip の
  二重経路で伝播する
- サーバ間は全て torsocks + hidden service。プライベートネットワーク不要
- フロントの入口のみ clearnet。VPS B は訪問者の生 IP を見る

**手順**: [deployment-2vps.md](deployment-2vps.md)
```

- [ ] **Step 2: §2.3 の SocksProxyAgent 案に訂正を入れる**

§2.3 の設定例の直後に追記:

```markdown
> **注記 (2026-09)**: 上記の Next.js API Routes + SocksProxyAgent 案は
> 実装されていない。App Router の route handler は WebSocket upgrade を
> サポートせず、PAPI は WS 前提なので `fetch` + agent の例は流用できない。
> また静的 SPA が「サーバ必須」になり静的ホスティングができなくなる。
> 中継が必要な場合はアプリの外（nginx + socat）に置くこと。
> `socks-proxy-agent` は package.json に残っているが未使用。
```

- [ ] **Step 3: 未使用依存を削除**

```bash
pnpm --filter @anarchy/frontend remove socks-proxy-agent
```

§2.3 の注記から `socks-proxy-agent` は package.json に残っている旨の一文を削る。

- [ ] **Step 4: フロントのビルドが壊れていないことを確認**

Run: `pnpm --filter @anarchy/frontend build 2>&1 | tail -20`
Expected: ビルド成功（`socks-proxy-agent` はどこからも import されていないため影響なし）

- [ ] **Step 5: テストが通ることを確認**

Run: `pnpm --filter @anarchy/frontend test 2>&1 | tail -20`
Expected: 既存テストが全て通る

- [ ] **Step 6: コミット**

```bash
git add docs/operations/tor-connection-patterns.md apps/frontend/package.json pnpm-lock.yaml
git commit -m "docs(operations): add 2-VPS pattern, correct unimplemented gateway design

§2.3 の SocksProxyAgent 案は未実装であることを明記し、未使用の
socks-proxy-agent 依存を削除する。

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**1. Requirements coverage**

| 要件 | 対応タスク |
|---|---|
| VPS A: チェーン 3 + ストレージ 5 | Task 5, 6 |
| VPS B: フロント + チェーン 1 | Task 8, 10 |
| アクセス経路① B のフロント | Task 10, 11 |
| アクセス経路② ローカル（任意の構成） | Task 12 Step 2（3 パターン） |
| Tor 有効・torsocks 方式 | Task 4, 5, 6, 8（全 ExecStart が torsocks 経由） |
| フロントは clearnet のみ | Task 10（Onion-Location なし、フロント用 hidden service なし） |
| プライベートネットワーク不使用 | Task 6（onion で広告）、Task 9（B→A 到達確認） |

**2. 未解決事項として明示したもの**

- Task 8 Step 3 の `/dns4/<onion>/` による libp2p dial は**未検証**。Step 6 に socat フォールバックを用意した
- Task 6 Step 5 の `storage_listNodes` は RPC 名が異なる可能性があるため `rpc_methods` での確認手順を併記した

**3. 型・シグネチャの一貫性**

- Task 1 で定義した `Config::public_url` / `Config::advertised_url()` / `ConfigOverrides::public_url` / `--public-url` を Task 6 で一貫して使用している
- ポート割り当ては Global Constraints と Task 5/6/8/10 で一致している
- onion アドレスの受け渡し（Task 4 → 6, 8, 9, 12）が全て「Task 4 Step 3 で控えた値」を参照している

**4. スコープ外（意図的に含めていない）**

- reqwest への `socks` feature 追加（ユーザー判断で torsocks 方式を採用）
- `--tor-mode=forced` のポート決め打ち修正（Task 12 Step 3 に既知の制約として記載、別 PR 相当）
- `X-Chain-Auth` の session-token 化（同上）
- VPS の選定・契約・OS 初期設定（前提条件）

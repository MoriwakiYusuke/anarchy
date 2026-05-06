# PoW Mining Setup Guide

> **Status**: Phase B 実装ベース
> **対象**: マイニング参加者・運用者

Anarchy mainnet で RandomX マイニングノードを起動する手順。

## ハードウェア要件

| 構成 | 推奨 | 必須最低 |
|---|---|---|
| CPU | 8 コア以上 (AMD Ryzen 9 / Intel Core i9 等) | 4 コア |
| RAM | 16GB 以上 | 8GB |
| Storage | NVMe SSD 100GB+ | SATA SSD 50GB |
| Network | 10 Mbps 以上 | 1 Mbps |
| OS | Linux x86_64 (Ubuntu 22.04+ / Fedora 40+) | Linux x86_64 |

注: ARM64 は randomx-rs の対応状況により制約あり。x86_64 推奨。

## RandomX のメモリモード

| Mode | RAM 使用 | hashrate 倍率 | 用途 |
|---|---|---|---|
| `fast` (Full dataset) | 2GB scratchpad + 256MB cache | ~3-10x | mainnet マイニング |
| `light` (Cache only) | 256MB | 1x ベースライン | 軽量ノード / 検証用 |

ガチで採掘するなら **fast 一択**。light は CI / dev 検証用。

## Linux: Large Pages (Hugepages) 設定

RandomX は 2MB hugepage を使うと hash rate が 2-3 倍になる。

### 永続設定 (`/etc/sysctl.d/99-randomx.conf`)

```
vm.nr_hugepages = 1280
```

`1280` = 1280 × 2MB = 2.5GB (RandomX dataset 2GB + 余裕)

反映:
```bash
sudo sysctl -p /etc/sysctl.d/99-randomx.conf
cat /proc/sys/vm/nr_hugepages   # 1280 であること
```

### transparent hugepage (THP) は無効推奨

THP は RandomX の性能を不安定にする:
```bash
echo never | sudo tee /sys/kernel/mm/transparent_hugepage/enabled
echo never | sudo tee /sys/kernel/mm/transparent_hugepage/defrag
```

永続化は `/etc/rc.local` か systemd service で。

## Coinbase アカウント生成

マイナー報酬 (5 MORAL/block × halving) を受け取る SS58 アドレスを準備。

### subkey で新規生成 (推奨)

```bash
# subkey は polkadot-sdk に同梱
cargo install --git https://github.com/paritytech/polkadot-sdk subkey
subkey generate --scheme sr25519 --network anarchy
# 出力例: Secret seed: 0x..., Public key (SS58): 5G...
```

シードフレーズは安全に保管 (オフラインマシン推奨)。

### 既存アカウント使用

`5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY` のような既存 SS58 を `--coinbase` に指定。

## ノード起動

### dev (smoke / 検証)

```bash
cd apps/blockchain
cargo build --release -p anarchy-node

./target/release/anarchy-node --dev --mine \
    --coinbase 5G... \
    --randomx-mode light \
    --base-path /var/lib/anarchy-dev \
    --rpc-port 9944
```

### mainnet (本番採掘)

```bash
./target/release/anarchy-node \
    --chain production \
    --mine \
    --coinbase 5G... \
    --randomx-mode fast \
    --base-path /var/lib/anarchy \
    --bootnodes /ip4/<seed-node-ip>/tcp/30333/p2p/<peer-id> \
    --validator \
    --tor-mode forced
```

`--tor-mode forced` で Tor anonymity 強制 (Anarchy Principle #1)。Tor daemon が同一マシンで
起動済みである必要あり (`/etc/tor/torrc`, ControlPort 9051)。

## systemd unit サンプル

`/etc/systemd/system/anarchy-miner.service`:

```ini
[Unit]
Description=Anarchy PoW Miner
After=network-online.target tor.service
Wants=network-online.target tor.service

[Service]
Type=simple
User=anarchy
Group=anarchy
ExecStart=/usr/local/bin/anarchy-node \
    --chain production --mine \
    --coinbase 5G... \
    --randomx-mode fast \
    --base-path /var/lib/anarchy \
    --tor-mode forced \
    --bootnodes /ip4/.../p2p/...
Restart=on-failure
RestartSec=30
LimitMEMLOCK=infinity

# Hugepage 確保のため
LimitMEMLOCK=infinity
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

`anarchy` ユーザを `vm.nr_hugepages` を予約できる group (例: `kvm`) に追加するか、
`LimitMEMLOCK=infinity` で hugepage lock 権限を与える。

```bash
sudo systemctl daemon-reload
sudo systemctl enable anarchy-miner
sudo systemctl start anarchy-miner
sudo journalctl -u anarchy-miner -f
```

## モニタリング

### ノード health

```bash
curl -s -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"system_health","params":[]}' \
    http://127.0.0.1:9944
```

### マイニング状況

ログ:
```bash
journalctl -u anarchy-miner -f | grep -E "🏆|✅|Imported|Finalized|hashrate"
```

期待形式:
```
🏆 Submitted valid seal at nonce 12345 (difficulty 50000)
✅ Successfully mined block on top of: 0xabc...
🏆 Imported #100 (0xprev → 0xnext)
👴 Finalized #98 (0x...)
```

## トラブルシューティング

| 症状 | 原因 | 対処 |
|---|---|---|
| ブロック生成が始まらない | difficulty が高すぎる | `bench-randomx.sh` で hashrate 計測。chain spec の initial_difficulty が hardware 想定より高い |
| `RandomX init failed: hugepages` | hugepages 未確保 | `vm.nr_hugepages=1280`、`LimitMEMLOCK=infinity` 確認 |
| `seal is invalid` 連発 | RandomX seed mismatch | `--purge-chain` で chain reset、最新 chain spec で再起動 |
| メモリ不足 panic | 2GB scratchpad + 他プロセスで OOM | `--randomx-mode light` で再起動、または RAM 増設 |
| Tor 接続失敗 | tor daemon 未起動 | `sudo systemctl start tor`、`/etc/tor/torrc` の SOCKSPort 確認 |

## bench: ハードウェア性能測定

```bash
./scripts/bench-randomx.sh 60 30
# 出力例:
#   Measured hashrate: 547.2 H/s
#   Recommended initial_difficulty: 16416
```

mainnet 投入時はリファレンス HW (e.g., 8-core CPU) で計測した値を chain spec の
`initial_difficulty` に使用 (mainnet runbook 参照)。

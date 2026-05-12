# Contract: Network Configuration API

**Feature**: 006-libp2p-tor  
**Date**: 2026-02-08  
**Version**: 1.0.0

## 概要

Anarchyノードのネットワーク設定に関するCLIオプションと設定ファイル形式を定義する。

---

## CLI Options

### `--tor-mode`

Torネットワーク使用モードを指定する。

| 項目 | 値 |
|------|-----|
| **型** | enum: `off`, `outbound-only`, `forced` |
| **デフォルト** | `off` |
| **必須** | No |

**動作説明**:

| 値 | 送信 | 受信 | ブートノード制限 | リスク |
|----|------|------|-----------------|------|
| `off` | 通常TCP | 通常TCP | なし | - |
| `outbound-only` | Tor経由（torsocks前提） | 通常TCP | なし | **受信側IP露出** |
| `forced` | Tor経由 | Onion Service | Onionアドレスのみ | - |

**使用例**:
```bash
# 開発環境（Torなし）
./anarchy-node --tor-mode=off

# torsocksと併用（**注意: 受信IPは露出します**）
torsocks ./anarchy-node --tor-mode=outbound-only

# 完全匿名モード
./anarchy-node --tor-mode=forced \
  --reserved-only \
  --reserved-nodes="/onion3/xyz...onion:30333/p2p/12D3KooW..."
```

**エラー条件**:
- `forced`モードでOnionブートノードが設定されていない場合: `Error: --tor-mode=forced requires at least one Onion bootstrap node`

---

### `--public-addr`（既存オプション拡張）

外部から見えるアドレスを広告する。Onionアドレスを含む。

| 項目 | 値 |
|------|-----|
| **型** | libp2p Multiaddress |
| **デフォルト** | 自動検出 |
| **必須** | Onion Service使用時は必須 |

**Onionアドレス形式**:
```
/onion3/<56文字のbase32>:<port>
```

**使用例**:
```bash
./anarchy-node \
  --public-addr=/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333
```

---

### `--reserved-nodes`（既存オプション）

優先接続するノードを指定する。`--reserved-only`と併用でそれ以外を拒否。

| 項目 | 値 |
|------|-----|
| **型** | libp2p Multiaddress（複数指定可） |
| **デフォルト** | なし |
| **必須** | `--tor-mode=forced`時 |

**使用例**:
```bash
./anarchy-node \
  --reserved-nodes="/onion3/boot1...:30333/p2p/12D3KooW..." \
  --reserved-nodes="/onion3/boot2...:30333/p2p/12D3KooW..." \
  --reserved-only
```

---

## 設定ファイル形式

### chain_spec.json（ブートノード設定）

**パス**: `apps/blockchain/chain_spec.json` または生成されたスペックファイル

```json
{
  "name": "Anarchy Testnet",
  "id": "anarchy_testnet",
  "bootNodes": [
    "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp",
    "/onion3/anothernodeaddress...:30333/p2p/12D3KooW...",
    "/ip4/203.0.113.10/tcp/30333/p2p/12D3KooW..."
  ]
}
```

**制約**:
- `--tor-mode=forced`では`/onion3/`プレフィックスのアドレスのみ使用される
- PeerIdは必須（`/p2p/`サフィックス）

---

### torrc（Tor設定）

**パス**: `/etc/tor/torrc` または `~/.tor/torrc`

```
# === Anarchy Node Onion Service ===
HiddenServiceDir /var/lib/tor/anarchy-node/
HiddenServicePort 30333 127.0.0.1:30333

# オプション: v3 Onionのみ（デフォルト）
HiddenServiceVersion 3
```

**生成ファイル**（`HiddenServiceDir`配下）:
| ファイル | 説明 |
|----------|------|
| `hostname` | `.onion`アドレス（公開可） |
| `hs_ed25519_public_key` | 公開鍵 |
| `hs_ed25519_secret_key` | 秘密鍵（厳重管理） |

---

## 環境変数

| 変数名 | 説明 | デフォルト |
|--------|------|-----------|
| `ANARCHY_TOR_MODE` | CLIの`--tor-mode`と同等（off/outbound-only/forced） | `off` |
| `TOR_SOCKS_PORT` | torsocks用（通常変更不要） | `9050` |

**優先順位**: CLI引数 > 環境変数 > デフォルト

---

## レスポンス/ログ形式

### 起動時ログ

```
[INFO] Tor mode: outbound-only (**WARNING: Inbound IP exposed**)
[INFO] Public address: /onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333
[INFO] Connecting to bootstrap node: /onion3/boot1...:30333/p2p/12D3KooW...
[INFO] Peer discovered: 12D3KooW... (via Onion)
```

### エラーログ

```
[ERROR] Tor mode 'forced' requires Onion bootstrap nodes. 
        Use --reserved-nodes with /onion3/ addresses.
[ERROR] Tor mode 'outbound-only' requires torsocks wrapper. 
        Run with: torsocks ./anarchy-node --tor-mode=outbound-only
```

---

## 互換性

| Substrate バージョン | サポート |
|---------------------|---------|
| stable2503 | ✅ |
| stable2407 | ✅ (未検証) |

| Tor バージョン | サポート |
|---------------|---------|
| 0.4.7+ | ✅ |
| 0.3.x | ⚠️ (v3 Onion Service必須) |

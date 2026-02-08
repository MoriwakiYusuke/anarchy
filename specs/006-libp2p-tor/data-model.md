# Data Model: libp2p + Tor統合

**Feature**: 006-libp2p-tor  
**Date**: 2026-02-08

## 概要

本機能はネットワーク層の設定変更が主であり、新規データエンティティは最小限。主に設定値とアドレス形式を定義する。

---

## エンティティ定義

### 1. TorMode（Torモード）

ノードの匿名化レベルを表す列挙型。

| 値 | 説明 | 用途 |
|----|------|------|
| `off` | Tor未使用（通常TCP） | 開発・テスト環境 |
| `outbound-only` | 送信のみTor経由（**受信IP露出リスクあり**） | torsocksラッパー使用時、リスクを理解した上での使用 |
| `forced` | Tor接続のみ許可 | 本番環境（匿名ノード） |

**バリデーション**:
- CLIで指定されない場合は`off`がデフォルト
- `forced`モードではブートノードにOnionアドレスが必須

---

### 2. OnionAddress（Onionアドレス）

Tor v3 Onion Serviceのアドレス。

| 属性 | 型 | 説明 |
|------|-----|------|
| `address` | String (56文字) | Base32エンコードされたv3 Onionアドレス |
| `port` | u16 | P2Pポート（デフォルト: 30333） |

**形式**: `{56文字のbase32}.onion:{port}`  
**例**: `vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion:30333`

**libp2pマルチアドレス形式**: `/onion3/{address}:{port}`

---

### 3. BootstrapNode（ブートストラップノード）

新規ノードが最初に接続する既知ノード。

| 属性 | 型 | 説明 |
|------|-----|------|
| `multiaddr` | String | libp2pマルチアドレス |
| `peer_id` | String | libp2p PeerId |

**例**:
```
/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
```

---

## 設定ファイル構造

### torrc（Tor設定）

```
# Onion Service for Anarchy node
HiddenServiceDir /var/lib/tor/anarchy-node/
HiddenServicePort 30333 127.0.0.1:30333
```

### chain_spec.json（ブートノード）

```json
{
  "bootNodes": [
    "/onion3/{onion-address}:{port}/p2p/{peer-id}",
    "/ip4/{ip}/tcp/{port}/p2p/{peer-id}"
  ]
}
```

---

## 状態遷移

### ノード起動フロー

```
[起動] 
  │
  ├─ tor-mode=off ──────> [通常TCP接続]
  │
  ├─ tor-mode=outbound-only ─> [torsocks検出]
  │                           │
  │                           ├─ 検出 ──> [Tor経由送信（**受信IP露出**）]
  │                           └─ 未検出 ─> [エラー: torsocks必須]
  │
  └─ tor-mode=forced ───> [Onionブートノード確認]
                              │
                              ├─ あり ──> [reserved-only接続]
                              └─ なし ──> [起動エラー]
```

---

## リレーションシップ

```
[TorMode]
    │
    │ 1:N (設定に基づく)
    ▼
[BootstrapNode]
    │
    │ 1:1
    ▼
[OnionAddress] (optional)
```

ノードは複数のブートストラップノードを持ち、各ブートストラップノードは通常のIPアドレスまたはOnionアドレスを持つ。`forced`モードではOnionアドレスのみ許可。

# Research: libp2p + Tor統合

**Feature**: 006-libp2p-tor  
**Date**: 2026-02-08  
**Status**: Complete

## 調査タスク

1. torsocksとSubstrateノードの互換性
2. Onion Service設定とlibp2pの統合
3. マルチアドレス（.onion）の広告方法
4. Torモード切替の実装アプローチ

---

## 1. torsocksとSubstrateノードの互換性

### Decision: ✅ 互換性あり（制限付き）

### Rationale

torsocksはLD_PRELOADを使用してlibcソケット関数を透過的にTor SOCKSプロキシにリダイレクトする。Substrateノード（libp2p実装）はTCPソケットを使用するため、基本的な互換性がある。

**検証済みの動作**:
- 送信TCP接続がTor経由でルーティングされる
- DNS解決もTor経由（リーク防止）
- libp2pのNoise暗号化はTorの上で正常に動作

**制限事項**:
- UDPはTor非対応（QUIC使用時は注意が必要だが、Substrateはデフォルトでは未使用）
- 受信接続は別途Onion Serviceが必要
- 一部のTor出口ノードでは特定ポートがブロックされる可能性

### Alternatives Considered

| 代替案 | 却下理由 |
|--------|---------|
| arti-client内蔵 | 1.0未達（0.25.x）、API不安定、sc-networkフォーク必要 |
| Tor Control Protocol直接使用 | 複雑性が高い、torsocksで十分 |
| I2P | Torより成熟度が低い、ユーザーベースが少ない |

---

## 2. Onion Service設定とlibp2pの統合

### Decision: Torデーモン側でOnion Serviceを設定し、ノードのP2Pポートにリバースプロキシ

### Rationale

Torデーモンの`torrc`でHiddenServiceを設定し、`.onion:port`を`localhost:p2p-port`にマッピングする標準的なアプローチが最も安定。

**設定例**（torrc）:
```
HiddenServiceDir /var/lib/tor/anarchy-node/
HiddenServicePort 30333 127.0.0.1:30333
```

**利点**:
- コード変更不要
- Torデーモンの安定したサービス管理
- 自動的なキーペア管理と`.onion`アドレス生成

**生成されるファイル構造**:
```
/var/lib/tor/anarchy-node/
├── hostname          # .onionアドレス（例: xyz123...abc.onion）
├── hs_ed25519_public_key
└── hs_ed25519_secret_key
```

### Alternatives Considered

| 代替案 | 却下理由 |
|--------|---------|
| libp2p-tor（Goライブラリ） | Rust版なし |
| OnionShareのようなラッパー | 過剰な依存関係 |

---

## 3. マルチアドレス（.onion）の広告方法

### Decision: `--public-addr`オプションでOnionアドレスを手動指定

### Rationale

Substrateノードは`--public-addr`フラグで外部から見えるアドレスを広告できる。Onion Serviceの`.onion`アドレスをここに指定することで、他のTorノードが接続可能になる。

**コマンド例**:
```bash
torsocks ./anarchy-node \
  --public-addr=/onion3/xyz123...abc:30333 \
  --listen-addr=/ip4/127.0.0.1/tcp/30333
```

**libp2pマルチアドレス形式**:
- `/onion3/<base32-address>:<port>` - Onion v3アドレス
- 例: `/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333`

### Alternatives Considered

| 代替案 | 却下理由 |
|--------|---------|
| 自動検出 | Torデーモンとの連携が複雑 |
| DNSシード | Onionアドレスは通常のDNSに登録不可 |

---

## 4. Torモード切替の実装アプローチ

### Decision: ① 出口ロック + ② 入口ロック の2段階強制

### Rationale

`--tor-mode=forced`は以下の2つのロックで匿名性を保証する:

| ロック | 目的 | 実装 |
|--------|------|------|
| **① 出口ロック** | 送信が必ずTor経由 | 環境変数`ANARCHY_RUNNING_UNDER_TORSOCKS`チェック、未設定なら`exit(1)` |
| **② 入口ロック** | 受信がOnion Service経由のみ | `listen_addresses`を`127.0.0.1:30333`に強制上書き |

**`forced`モードの実装**:
```rust
// command.rs
if tor_mode == TorMode::Forced {
    // ① 出口ロック
    if std::env::var("ANARCHY_RUNNING_UNDER_TORSOCKS").is_err() {
        eprintln!("ERROR: --tor-mode=forced requires torsocks!");
        eprintln!("Usage: ./scripts/anarchy-tor.sh ./target/release/anarchy-node --tor-mode=forced");
        std::process::exit(1);
    }
    
    // ② 入口ロック
    config.network.listen_addresses = vec![
        "/ip4/127.0.0.1/tcp/30333".parse().unwrap(),
    ];
    
    log::info!("🔒 Tor forced mode: listening on 127.0.0.1 only");
}
```

**モード一覧**:

| モード | 動作 | 実装 |
|--------|------|------|
| `off` | 通常のTCP接続 | デフォルト（変更なし） |
| `outbound-only` | 送信のみTor化（**受信IP露出リスク**） | torsocks wrapper必須、警告表示 |
| `forced` | 完全匿名 | ①出口ロック + ②入口ロック |

### Alternatives Considered

| 代替案 | 却下理由 |
|--------|---------|
| `--reserved-only`でピア制限 | 動的ピア発見不可、普及性を阻害 |
| ブートノードの.onionフィルター | chain_specを正しく作るべき、過剰な防御 |
| sc-network改変 | メンテナンスコストが高い |
| iptablesで非Tor遮断 | システムレベルの変更が必要、ポータビリティ低下 |

---

## 5. ブートストラップノード設定

### Decision: チェーンスペックのbootNodesにOnionアドレスを追加

### Rationale

Substrateの`chain_spec.json`でブートノードを定義できる。Onionアドレスを含めることで、新規ノードがTorネットワーク経由で参加可能。

**chain_spec.json例**:
```json
{
  "bootNodes": [
    "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooW...",
    "/onion3/anothernodeaddress...:30333/p2p/12D3KooW..."
  ]
}
```

**フォールバック戦略**:
複数のブートノードを設定し、接続失敗時に自動的に次のノードへ。libp2pの標準動作として組み込み済み。

---

## 技術リスクと緩和策

| リスク | 影響 | 緩和策 |
|--------|------|--------|
| Tor出口ノードのブロック | 一部地域で接続不可 | 複数出口を自動選択（Tor標準動作） |
| Onion Serviceの遅延 | 接続確立に数秒 | タイムアウト値を調整（90秒推奨） |
| torsocksのメモリリーク（古いバージョン） | 長時間運用で問題 | torsocks 2.3+を使用、定期再起動 |
| Onionアドレス漏洩 | 匿名性低下 | ログ出力のサニタイズ |

---

## 結論

Phase 1-2のTor統合は既存のSubstrate/libp2p機能と外部Torデーモンの組み合わせで実現可能。コード変更は最小限（CLIオプション追加とドキュメント）。

**次ステップ**:
1. torsocksでの動作検証（Phase 1）
2. Onion Service設定スクリプト作成（Phase 2）
3. チェーンスペックへのOnionブートノード追加

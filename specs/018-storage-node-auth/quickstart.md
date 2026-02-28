# Quickstart: ストレージノードアクセス制限（セッショントークン認証）

**Spec**: [spec.md](spec.md)  
**Implementation Plan**: [plan.md](plan.md)

## 概要

ストレージノードの書き込み・削除操作をP2P接続済みブロックチェーンノードに限定する認証機構。
libp2p経由でセッショントークンを取得し、HTTP API呼び出し時にトークンで認証する。

## 前提条件

- Rust 1.81+ (stable2503互換)
- Docker (オプション)
- ブロックチェーンノードとストレージノードがlibp2pで接続済み

## クイックセットアップ

### 1. ストレージノードのビルド

```bash
cd apps/storage-node
cargo build --release
```

### 2. 設定ファイル

`config.toml`にセッション設定を追加:

```toml
[session]
# トークン有効期間（秒）、デフォルト: 86400 (24時間)
token_ttl_secs = 86400

# アイドルタイムアウト（秒）、デフォルト: 3600 (1時間)
idle_timeout_secs = 3600

# クリーンアップ間隔（秒）、デフォルト: 300 (5分)
cleanup_interval_secs = 300
```

### 3. ノードの起動

```bash
# ストレージノード起動
./target/release/anarchy-storage-node --config config.toml

# 別ターミナルでブロックチェーンノード起動
cd ../../apps/blockchain
./target/release/anarchy-node --dev
```

## 認証フロー

### Step 1: セッショントークンの取得

ブロックチェーンノードからlibp2p経由でセッションをリクエスト:

```rust
use anarchy_storage_client::StorageClient;

// ストレージノードへのP2P接続
let client = StorageClient::connect(storage_peer_id).await?;

// セッショントークンを取得
let session = client.request_session(&keypair).await?;
println!("Token: {}", session.token);
println!("Expires at: {}", session.expires_at);
```

### Step 2: HTTP APIでフラグメント書き込み

```bash
curl -X POST http://localhost:3030/fragments \
  -H "Content-Type: application/json" \
  -H "X-Session-Token: a1b2c3d4e5f6..." \
  -d '{
    "id": "fragment_001",
    "data": "SGVsbG8gV29ybGQ=",
    "commitment": "0x...",
    "metadata": {
      "post_id": "post_xyz",
      "shard_index": 0,
      "total_shards": 5
    }
  }'
```

### Step 3: 読み取り（認証不要）

```bash
curl http://localhost:3030/fragments/fragment_001
```

### Step 4: トークン更新（有効期限1時間前から可能）

```rust
// 自動更新（クライアント側で実装）
let new_session = client.renew_session(&session.token).await?;
```

## テスト

### ユニットテスト

```bash
cd apps/storage-node
cargo test session
```

### 統合テスト

```bash
cd apps/blockchain/tests/integration
./storage_auth_test.sh
```

## トラブルシューティング

### 401 Unauthorized: missing_token

書き込み・削除リクエストに`X-Session-Token`ヘッダーがない:

```bash
# 正しい形式
curl -H "X-Session-Token: your_token_here" ...
```

### 403 Forbidden: invalid_token

1. トークンの有効期限が切れている → 再度`storage_requestSession`を呼び出し
2. トークンがアイドルタイムアウトした → 再度`storage_requestSession`を呼び出し
3. 他のセッションが取得され旧トークンが無効化された → 新トークンを使用

### -32001: Not connected via P2P

ストレージノードとブロックチェーンノードがlibp2pで接続されていない:

```bash
# bootstrap_peersを確認
grep bootstrap_peers config.toml
```

### 署名検証エラー (-32002)

1. タイムスタンプが±30秒以内か確認
2. 署名ペイロードが`"anarchy-session-request:{timestamp}"`形式か確認
3. 公開鍵と秘密鍵のペアが正しいか確認

## 複数ノード同時セッションテスト (Concurrent Session Test)

複数のブロックチェーンノードが同時にセッションを確立し、それぞれ独立して動作することを確認するシナリオ。

### テスト環境

```bash
# ストレージノード1台 + ブロックチェーンノード2台
./target/release/anarchy-storage-node --config config.toml
./target/release/anarchy-node --dev --base-path /tmp/node1 --ws-port 9944
./target/release/anarchy-node --dev --base-path /tmp/node2 --ws-port 9955
```

### テストステップ

1. **両ノードがP2P接続を確立**

```bash
# node1のPeerIdを取得
curl -s http://localhost:9944 -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"system_localPeerId","params":[]}' \
  | jq -r '.result'

# node2も同様
```

2. **両ノードがそれぞれセッションを取得**

```rust
// Node 1
let session1 = client1.request_session(&keypair1).await?;

// Node 2 (同時実行)
let session2 = client2.request_session(&keypair2).await?;

// 両方のトークンが異なることを確認
assert_ne!(session1.token, session2.token);
```

3. **両ノードが独立してフラグメント書き込み**

```bash
# Node 1からの書き込み
curl -X POST http://localhost:3030/fragments \
  -H "X-Session-Token: $SESSION1_TOKEN" \
  -d '{"id": "frag_from_node1", ...}'

# Node 2からの書き込み
curl -X POST http://localhost:3030/fragments \
  -H "X-Session-Token: $SESSION2_TOKEN" \
  -d '{"id": "frag_from_node2", ...}'
```

4. **一方のセッションを失効させても他方は有効**

```rust
// Node 1のセッションを失効
client1.revoke_session().await?;

// Node 2はまだ有効
let result = client2.upload_fragment(fragment).await;
assert!(result.is_ok());
```

### 期待される結果

- ✅ 両ノードが独立したセッショントークンを取得
- ✅ 両ノードのトークンが同時に有効
- ✅ 片方のセッション終了が他方に影響しない
- ✅ 各ノードが24時間以上連続稼働可能（自動更新あり）

### 検証コマンド

```bash
cd apps/storage-node
cargo test test_multiple_peers -- --nocapture
```

## API リファレンス

- [JSON-RPC API](contracts/json-rpc.md)
- [HTTP API](contracts/http-api.md)
- [Data Model](data-model.md)

## 関連ドキュメント

- [Research Notes](research.md)
- [Implementation Tasks](tasks.md)

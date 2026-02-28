# Research: ストレージノードアクセス制限（セッショントークン認証）

**Date**: 2026-03-01  
**Spec**: [spec.md](spec.md)

## Research Tasks

### 1. libp2p connected_peers 取得方法

**Question**: libp2pのSwarmからconnected_peersを取得し、署名者のpeer_idが含まれるか検証する方法は？

**Finding**: 
- `Swarm::connected_peers()` でイテレータを取得可能
- `HashSet<PeerId>` をリアルタイムで維持する場合、`SwarmEvent::ConnectionEstablished` / `ConnectionClosed` をハンドリング
- 既存実装: `apps/storage-node/src/p2p/` にSwarmハンドラあり

**Decision**: SwarmEventハンドラで`connected_peers: HashSet<PeerId>`を維持。セッションリクエスト時にO(1)で検証可能。

**Alternatives Rejected**:
- `Swarm::connected_peers()` を毎回呼び出し → イテレーション必要、O(n)
- Kademliaルーティングテーブル参照 → 接続状態と一致しない可能性

---

### 2. セッショントークン生成（256ビットランダム）

**Question**: セキュアな256ビットランダムトークンの生成方法は？

**Finding**:
- `rand` crate の `OsRng` + `rand::Rng::gen::<[u8; 32]>()` が推奨
- `getrandom` crateは低レベルすぎる
- hex encodingで64文字の文字列トークンに変換

**Decision**: `rand::rngs::OsRng` + `hex::encode()` で256ビットトークンを生成。

```rust
use rand::Rng;
let token_bytes: [u8; 32] = rand::rngs::OsRng.gen();
let token = hex::encode(token_bytes);
```

**Alternatives Rejected**:
- UUIDv4 → 128ビット、不十分
- SHA256ハッシュ → 入力が必要、ランダム性が入力依存

---

### 3. axum ミドルウェアでのトークン検証

**Question**: axumでHTTPヘッダー (`X-Session-Token`) を検証するベストプラクティスは？

**Finding**:
- `tower::ServiceBuilder` + カスタムレイヤーが推奨
- `axum::extract::FromRequestParts` でヘッダー抽出
- 既存実装: `apps/storage-node/src/rpc/auth.rs` に `X-Anarchy-Auth` 検証あり

**Decision**: 既存の `auth.rs` パターンを流用。`X-Session-Token` ヘッダーをExtractorで取得し、SessionRegistryで照合。

```rust
pub async fn extract_session_token(
    headers: &HeaderMap,
    registry: &SessionRegistry,
) -> Result<PeerId, AuthError> {
    let token = headers.get("X-Session-Token")
        .ok_or(AuthError::MissingToken)?
        .to_str()
        .map_err(|_| AuthError::InvalidToken)?;
    registry.validate(token).ok_or(AuthError::InvalidToken)
}
```

**Alternatives Rejected**:
- ミドルウェアで全エンドポイント検証 → `/health`、読み取りエンドポイントは除外必要
- Bearerトークン形式 → シンプルな`X-Session-Token`で十分

---

### 4. Ed25519署名検証（libp2p peer_id）

**Question**: libp2pのpeer_id（Ed25519公開鍵）で署名を検証する方法は？

**Finding**:
- `libp2p::identity::Keypair::Ed25519` から公開鍵取得
- `ed25519_dalek::Signature::from_bytes()` + `PublicKey::verify_strict()`
- peer_idはマルチハッシュ形式、公開鍵は `PeerId::as_ref()` で取得不可
- `libp2p::identity::PublicKey::try_decode_protobuf()` で復元

**Decision**: `storage_requestSession` リクエストに公開鍵バイト列を含め、peer_idと照合してから署名検証。

```rust
// リクエスト: { public_key: Vec<u8>, signature: Vec<u8>, timestamp: u64 }
let pubkey = PublicKey::try_decode_protobuf(&req.public_key)?;
let peer_id = PeerId::from(pubkey);
if !connected_peers.contains(&peer_id) {
    return Err(AuthError::NotConnected);
}
// 署名検証
ed25519_dalek::VerifyingKey::from_bytes(&pubkey.to_ed25519().unwrap())?
    .verify_strict(&req.message_bytes(), &signature)?;
```

**Alternatives Rejected**:
- peer_idのみで検証 → 公開鍵復元が複雑、プロトコル非対応
- TLS証明書で検証 → libp2pはNoiseプロトコル使用

---

### 5. HTTP repair コード削除の影響

**Question**: HTTPベースのリカバリコードを削除した場合の影響範囲は？

**Finding**:
- `apps/storage-node/src/rpc/client.rs`: `request_fragment_from_peer()` 等のHTTPクライアント
- `apps/storage-node/src/sync/repair.rs`: リペアロジック
- 既にlibp2p P2P経由の `request_fragment()` が存在（`apps/storage-node/src/p2p/handler.rs`）

**Decision**: HTTP RPC経由のフラグメント取得を削除し、全てlibp2p P2Pに統一。影響範囲は限定的。

| ファイル | 変更 |
|---------|------|
| `src/rpc/client.rs` | `request_fragment_from_peer()` 削除 |
| `src/sync/repair.rs` | libp2p呼び出しに置換 |
| `src/p2p/handler.rs` | 変更なし（既存機能） |

**Alternatives Rejected**:
- HTTPを認証対象に追加 → 複雑化、P2Pで十分
- 両方維持 → 冗長、メンテナンスコスト増

---

### 6. セッショントークン自動更新のタイミング

**Question**: ブロックチェーンノード側でトークン自動更新を実装する方法は？

**Finding**:
- `tokio::time::interval` でバックグラウンドタスク実行
- トークン発行時に有効期限を受信、ローカルで監視
- 残り1時間（23時間経過）で新トークン取得

**Decision**: ブロックチェーンノード起動時に `spawn` でセッション管理タスクを開始。有効期限の1時間前に自動更新。

```rust
async fn session_manager(storage_client: StorageClient) {
    let mut session = storage_client.request_session().await?;
    loop {
        let remaining = session.expires_at - Instant::now();
        if remaining < Duration::from_secs(3600) {
            // 自動更新
            session = storage_client.request_session().await?;
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
```

**Alternatives Rejected**:
- トークン期限切れ時に再取得 → 一時的にリクエスト失敗する可能性
- ストレージノード側からプッシュ → 複雑化

---

## Summary

全ての研究課題が解決済み。設計に必要な技術的決定が完了。

| 課題 | 決定 |
|------|------|
| connected_peers取得 | SwarmEventハンドラで`HashSet<PeerId>`を維持 |
| トークン生成 | `rand::rngs::OsRng` + `hex::encode()` |
| axumトークン検証 | 既存auth.rsパターンを流用 |
| Ed25519署名検証 | 公開鍵バイト列を含むリクエスト |
| HTTP repair削除 | libp2p P2Pに統一、影響限定的 |
| 自動更新 | tokioバックグラウンドタスク、1時間前更新 |

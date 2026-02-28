# Data Model: ストレージノードアクセス制限（セッショントークン認証）

**Date**: 2026-03-01  
**Spec**: [spec.md](spec.md)

## Entities

### SessionToken

256ビットランダム値。ブロックチェーンノードを識別するための一時的なトークン。

```rust
/// 256-bit session token (hex-encoded string, 64 characters)
pub type SessionToken = String;

impl SessionToken {
    /// Generate a new cryptographically secure random token
    pub fn generate() -> Self {
        let bytes: [u8; 32] = rand::rngs::OsRng.gen();
        hex::encode(bytes)
    }
}
```

| Field | Type | Description |
|-------|------|-------------|
| value | `String` | 64文字のhexエンコード文字列 |

**Validation Rules**:
- 長さ: 64文字（256ビット = 32バイト × 2）
- 文字: `[0-9a-f]` のみ

---

### SessionInfo

セッション情報。トークンに紐づくメタデータ。

```rust
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Associated peer ID
    pub peer_id: PeerId,
    /// Token issuance timestamp
    pub issued_at: Instant,
    /// Token expiration timestamp
    pub expires_at: Instant,
    /// Last access timestamp (for idle timeout)
    pub last_access: Instant,
}
```

| Field | Type | Description |
|-------|------|-------------|
| peer_id | `PeerId` | セッション所有者のlibp2p peer ID |
| issued_at | `Instant` | トークン発行時刻 |
| expires_at | `Instant` | トークン有効期限（発行から24時間後） |
| last_access | `Instant` | 最終アクセス時刻（アイドルタイムアウト用） |

**Validation Rules**:
- `expires_at` > `issued_at`
- `expires_at` - `issued_at` = 24時間
- `last_access` >= `issued_at`

---

### SessionRegistry

ストレージノードが保持するセッションレジストリ。

```rust
use std::collections::HashMap;
use std::sync::RwLock;

pub struct SessionRegistry {
    /// Token -> SessionInfo mapping
    sessions: RwLock<HashMap<SessionToken, SessionInfo>>,
    /// Token TTL (default: 24 hours)
    ttl: Duration,
    /// Idle timeout (default: 1 hour)
    idle_timeout: Duration,
}

impl SessionRegistry {
    /// Create new session for a peer
    pub fn create_session(&self, peer_id: PeerId) -> SessionToken;
    
    /// Validate token and return peer_id if valid
    pub fn validate(&self, token: &str) -> Option<PeerId>;
    
    /// Revoke existing token for peer (on re-session)
    pub fn revoke_for_peer(&self, peer_id: &PeerId);
    
    /// Cleanup expired and idle sessions
    pub fn cleanup_expired(&self);
    
    /// Get session count (for metrics)
    pub fn session_count(&self) -> usize;
}
```

| Method | Description |
|--------|-------------|
| `create_session(peer_id)` | 新規セッション作成、既存トークンは無効化 |
| `validate(token)` | トークン検証、有効ならpeer_idを返す |
| `revoke_for_peer(peer_id)` | peer_idの既存トークンを無効化 |
| `cleanup_expired()` | 期限切れ・アイドルセッションを削除 |

**State Transitions**:

```
                create_session()
                     │
                     ▼
    ┌─────────────────────────────────┐
    │           ACTIVE                │
    │  (issued_at < now < expires_at) │
    └─────────────────────────────────┘
           │                    │
           │ expires_at < now   │ idle_timeout exceeded
           │ OR revoke_for_peer │ OR cleanup_expired
           ▼                    ▼
    ┌─────────────────────────────────┐
    │           EXPIRED               │
    │     (removed from HashMap)      │
    └─────────────────────────────────┘
```

---

### ConnectedPeers

libp2pが管理する接続済みpeer_idのセット。

```rust
use std::collections::HashSet;
use std::sync::RwLock;

pub struct ConnectedPeers {
    peers: RwLock<HashSet<PeerId>>,
}

impl ConnectedPeers {
    /// Called on SwarmEvent::ConnectionEstablished
    pub fn add(&self, peer_id: PeerId);
    
    /// Called on SwarmEvent::ConnectionClosed
    pub fn remove(&self, peer_id: &PeerId);
    
    /// Check if peer is connected
    pub fn contains(&self, peer_id: &PeerId) -> bool;
}
```

**Event Handling**:

| SwarmEvent | Action |
|------------|--------|
| `ConnectionEstablished { peer_id, .. }` | `connected_peers.add(peer_id)` |
| `ConnectionClosed { peer_id, .. }` | `connected_peers.remove(&peer_id)` |

---

### SessionRequest

セッションリクエストメッセージ。

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionRequest {
    /// JSON-RPC method
    pub method: String,
    /// Request parameters
    pub params: SessionRequestParams,
    /// JSON-RPC ID
    pub id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SessionRequestParams {
    /// Request new session
    Request {
        /// Ed25519 public key (hex, 64 characters)
        public_key: String,
        /// Unix timestamp (seconds)
        timestamp: u64,
        /// Random nonce (hex, 32 characters = 16 bytes)
        nonce: String,
        /// Ed25519 signature (hex, 128 characters)
        signature: String,
    },
    /// Renew existing session
    Renew { token: String },
    /// Revoke session
    Revoke { token: String },
}
```

| Field | Type | Description |
|-------|------|-------------|
| method | `String` | `"storage_requestSession"`, `"storage_renewSession"`, or `"storage_revokeSession"` |
| params.public_key | `String` | Ed25519公開鍵（hex, 64文字） |
| params.timestamp | `u64` | リクエスト時刻（UNIX epoch秒） |
| params.nonce | `String` | ランダムnonce（hex, 32文字）、リプレイ攻撃防止 |
| params.signature | `String` | Ed25519署名（hex, 128文字） |
| params.token | `String` | セッショントークン（renew/revoke時） |

**Validation (storage_requestSession)**:
1. `public_key` からpeer_idを復元
2. peer_idが`connected_peers`に含まれるか確認
3. nonceが32 hex文字であることを確認
4. nonceがNonceCacheに存在しないことを確認（リプレイ防止）
5. 署名検証（メッセージ = `"anarchy-session-request:{timestamp}:{nonce}"`）
6. タイムスタンプが現在時刻±30秒以内か確認

**Validation (storage_renewSession / storage_revokeSession)**:
1. tokenが有効なセッショントークンであることを確認
2. 署名検証は不要（トークンベース認証）

---

### SessionResponse

セッションレスポンスメッセージ。

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    /// Session token (256-bit hex)
    pub token: String,
    /// Token expiration (UNIX timestamp)
    pub expires_at: u64,
}
```

| Field | Type | Description |
|-------|------|-------------|
| token | `String` | 64文字のセッショントークン |
| expires_at | `u64` | 有効期限（UNIX epoch秒） |

---

## Relationships

```
┌──────────────────────────────────────────────────────────────────┐
│                     Storage Node                                  │
│                                                                   │
│  ┌─────────────────┐       ┌────────────────────────────────┐   │
│  │ ConnectedPeers  │◄──────│ SwarmEvent Handler             │   │
│  │ HashSet<PeerId> │       │ (ConnectionEstablished/Closed) │   │
│  └────────┬────────┘       └────────────────────────────────┘   │
│           │                                                      │
│           │ contains?                                            │
│           ▼                                                      │
│  ┌─────────────────┐       ┌────────────────────────────────┐   │
│  │ SessionRegistry │◄──────│ storage_requestSession RPC     │   │
│  │ HashMap<Token,  │       │ (signature verification)       │   │
│  │   SessionInfo>  │       └────────────────────────────────┘   │
│  └────────┬────────┘                                             │
│           │                                                      │
│           │ validate                                             │
│           ▼                                                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ HTTP RPC Endpoints (storage_storeFragment, etc.)        │    │
│  │ X-Session-Token header required for write operations    │    │
│  └─────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                     Blockchain Node                               │
│                                                                   │
│  ┌─────────────────┐       ┌────────────────────────────────┐   │
│  │ SessionClient   │──────►│ storage_requestSession RPC     │   │
│  │ (holds token)   │       │ (signs with Ed25519 keypair)   │   │
│  └────────┬────────┘       └────────────────────────────────┘   │
│           │                                                      │
│           │ auto-renew (1 hour before expiry)                    │
│           ▼                                                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ HTTP RPC Calls (storage_storeFragment, etc.)            │    │
│  │ X-Session-Token header attached                         │    │
│  └─────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Peer not connected")]
    NotConnected,
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Invalid public key")]
    InvalidPublicKey,
    
    #[error("Request timestamp out of range")]
    TimestampOutOfRange,
    
    #[error("Missing session token")]
    MissingToken,
    
    #[error("Invalid or expired session token")]
    InvalidToken,
}
```

| Error | HTTP Status | Description |
|-------|-------------|-------------|
| `NotConnected` | 403 Forbidden | peer_idがconnected_peersに含まれない |
| `InvalidSignature` | 403 Forbidden | 署名検証失敗 |
| `InvalidPublicKey` | 400 Bad Request | 公開鍵のデコード失敗 |
| `TimestampOutOfRange` | 400 Bad Request | タイムスタンプが許容範囲外 |
| `MissingToken` | 401 Unauthorized | X-Session-Tokenヘッダーなし |
| `InvalidToken` | 401 Unauthorized | トークンが無効または期限切れ |

## Configuration

```rust
pub struct SessionConfig {
    /// Token TTL (default: 24 hours)
    pub token_ttl: Duration,
    /// Idle timeout (default: 1 hour)
    pub idle_timeout: Duration,
    /// Cleanup interval (default: 10 minutes)
    pub cleanup_interval: Duration,
    /// Timestamp tolerance (default: 5 minutes)
    pub timestamp_tolerance: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            token_ttl: Duration::from_secs(24 * 60 * 60),
            idle_timeout: Duration::from_secs(60 * 60),
            cleanup_interval: Duration::from_secs(10 * 60),
            timestamp_tolerance: Duration::from_secs(5 * 60),
        }
    }
}
```

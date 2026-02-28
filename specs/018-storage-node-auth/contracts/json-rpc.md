# JSON-RPC API Contract: Session Authentication

**Version**: 1.0.0  
**Transport**: libp2p request-response protocol  
**Protocol ID**: `/anarchy/session/1.0.0`

## Overview

ストレージノードとブロックチェーンノード間のセッション認証用JSON-RPC API。
libp2p経由でのみ利用可能（HTTP経由での呼び出し不可）。

## Methods

### storage_requestSession

セッショントークンをリクエスト。P2P接続済みのブロックチェーンノードのみ利用可能。

**Request**:

```json
{
  "jsonrpc": "2.0",
  "method": "storage_requestSession",
  "params": {
    "public_key": "0x...",
    "timestamp": 1709251200,
    "signature": "0x..."
  },
  "id": 1
}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| public_key | `string` | Yes | Ed25519公開鍵（hex, 64文字） |
| timestamp | `integer` | Yes | Unix timestamp（秒）、±30秒以内 |
| signature | `string` | Yes | Ed25519署名（hex, 128文字） |

**Signature Payload**:

```
"anarchy-session-request:{timestamp}"
```

**Response (Success)**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "token": "a1b2c3d4...",
    "expires_at": 1709337600
  },
  "id": 1
}
```

| Field | Type | Description |
|-------|------|-------------|
| token | `string` | セッショントークン（hex, 64文字） |
| expires_at | `integer` | トークン有効期限（Unix timestamp） |

**Response (Error)**:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32001,
    "message": "Not connected via P2P"
  },
  "id": 1
}
```

**Error Codes**:

| Code | Message | Description |
|------|---------|-------------|
| -32001 | Not connected via P2P | P2P接続されていないノードからのリクエスト |
| -32002 | Invalid signature | 署名検証失敗 |
| -32003 | Invalid timestamp | タイムスタンプが許容範囲外 |
| -32004 | Invalid public key format | 公開鍵フォーマット不正 |
| -32600 | Invalid Request | JSON-RPCフォーマット不正 |
| -32601 | Method not found | 不明なメソッド |
| -32602 | Invalid params | パラメータ不正 |

---

### storage_renewSession

セッショントークンを更新。有効期限の1時間前から更新可能。

**Request**:

```json
{
  "jsonrpc": "2.0",
  "method": "storage_renewSession",
  "params": {
    "token": "a1b2c3d4..."
  },
  "id": 2
}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| token | `string` | Yes | 現在のセッショントークン（hex, 64文字） |

**Response (Success)**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "token": "e5f6g7h8...",
    "expires_at": 1709424000
  },
  "id": 2
}
```

新しいトークンが発行され、古いトークンは無効化される。

**Response (Error)**:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32005,
    "message": "Session not found or expired"
  },
  "id": 2
}
```

**Error Codes**:

| Code | Message | Description |
|------|---------|-------------|
| -32005 | Session not found or expired | トークンが無効または期限切れ |
| -32006 | Renewal not allowed yet | 有効期限まで1時間以上残っている |

---

### storage_revokeSession

セッショントークンを明示的に無効化。

**Request**:

```json
{
  "jsonrpc": "2.0",
  "method": "storage_revokeSession",
  "params": {
    "token": "a1b2c3d4..."
  },
  "id": 3
}
```

**Response (Success)**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "revoked": true
  },
  "id": 3
}
```

**Note**: 既に無効なトークンでもエラーは返さない（冪等性）。

---

## Sequence Diagram

```
Blockchain Node                          Storage Node
     │                                        │
     │ ────── P2P Connect ──────────────────► │
     │                                        │
     │ ───── storage_requestSession ────────► │
     │        {public_key, timestamp, sig}    │
     │                                        │
     │ ◄──────── Success ──────────────────── │
     │        {token, expires_at}             │
     │                                        │
     │ ════════════════════════════════════   │
     │  (Use token for HTTP fragment ops)     │
     │ ════════════════════════════════════   │
     │                                        │
     │ ───── storage_renewSession ──────────► │
     │        {token}                         │
     │                                        │
     │ ◄──────── Success ──────────────────── │
     │        {new_token, expires_at}         │
     │                                        │
```

---

## Implementation Notes

### Rust Types

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionRequest {
    pub public_key: String,  // hex-encoded Ed25519 public key
    pub timestamp: u64,      // Unix timestamp in seconds
    pub signature: String,   // hex-encoded Ed25519 signature
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    pub token: String,       // hex-encoded 256-bit token
    pub expires_at: u64,     // Unix timestamp in seconds
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenewRequest {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevokeRequest {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevokeResponse {
    pub revoked: bool,
}
```

### Validation

1. **Timestamp tolerance**: ±30秒
2. **Signature verification**: Ed25519、payload = `"anarchy-session-request:{timestamp}"`
3. **P2P connection check**: リクエスト元peer_idが`ConnectedPeers`に存在すること

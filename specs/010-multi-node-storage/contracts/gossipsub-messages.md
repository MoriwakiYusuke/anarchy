# Gossipsub Messages Specification

**Feature**: 010-multi-node-storage  
**Protocol Version**: 1.0.0

## Overview

ストレージノード間でブロックチェーンエンドポイント情報を共有するためのGossipsubプロトコル仕様。

## Topics

### `/anarchy/endpoints/1.0.0`

ブロックチェーンノードRPCエンドポイントの共有トピック。

**Message Flow**:
1. ノードが新しいエンドポイントを発見（または既存の検証更新）
2. EndpointMessageを構築し署名
3. Gossipsubで publish
4. 受信ノードは署名検証後、自身のエンドポイントキャッシュを更新

### `/anarchy/storage-nodes/1.0.0` (FR-520)

ストレージノードアドレスの共有トピック。

**Message Flow**:
1. ノードがチェーンノードから`storage_getNodes`で新しいストレージノード一覧を取得
2. 各ノードへの接続検証（`/health`エンドポイント呼び出し、FR-518）
3. StorageNodeMessageを構築し署名（FR-517）
4. Gossipsubで publish
5. 受信ノードは署名検証後、自身のストレージノードキャッシュを更新

## Message Structures

### EndpointMessage

```
┌──────────────────────────────────────────────────────────────┐
│ EndpointMessage                                              │
├──────────────────────────────────────────────────────────────┤
│ version: u8                    │ Protocol version (1)        │
├────────────────────────────────┼─────────────────────────────┤
│ sender_peer_id: [u8; 38-52]    │ Ed25519 PeerID (multihash)  │
├────────────────────────────────┼─────────────────────────────┤
│ timestamp: u64                 │ Unix timestamp (seconds)    │
├────────────────────────────────┼─────────────────────────────┤
│ endpoints_count: u8            │ Number of endpoints (1-20)  │
├────────────────────────────────┼─────────────────────────────┤
│ endpoints: [BlockchainEndpoint]│ Array of endpoints          │
├────────────────────────────────┼─────────────────────────────┤
│ signature: [u8; 64]            │ Ed25519 signature           │
└──────────────────────────────────────────────────────────────┘
```

### BlockchainEndpoint

```
┌──────────────────────────────────────────────────────────────┐
│ BlockchainEndpoint                                           │
├──────────────────────────────────────────────────────────────┤
│ url_len: u16                   │ URL length                  │
├────────────────────────────────┼─────────────────────────────┤
│ url: [u8; url_len]             │ WebSocket RPC URL (UTF-8)   │
├────────────────────────────────┼─────────────────────────────┤
│ chain_id: [u8; 32]             │ Genesis hash                │
├────────────────────────────────┼─────────────────────────────┤
│ last_verified: u64             │ Unix timestamp              │
├────────────────────────────────┼─────────────────────────────┤
│ latency_ms: u32                │ Measured latency            │
├────────────────────────────────┼─────────────────────────────┤
│ ttl_secs: u32                  │ Time-to-live (default 300)  │
└──────────────────────────────────────────────────────────────┘
```

### StorageNodeMessage (FR-515〜520)

```
┌──────────────────────────────────────────────────────────────┐
│ StorageNodeMessage                                           │
├──────────────────────────────────────────────────────────────┤
│ sender_peer_id: String         │ Ed25519 PeerID (base58)     │
├────────────────────────────────┼─────────────────────────────┤
│ sender_public_key: String      │ Protobuf-encoded (hex)      │
├────────────────────────────────┼─────────────────────────────┤
│ timestamp: u64                 │ Unix timestamp (seconds)    │
├────────────────────────────────┼─────────────────────────────┤
│ nodes_count: u8                │ Number of nodes (1-20)      │
├────────────────────────────────┼─────────────────────────────┤
│ nodes: [StorageNodeEndpoint]   │ Array of storage nodes      │
├────────────────────────────────┼─────────────────────────────┤
│ signature: String              │ Ed25519 signature (hex)     │
└──────────────────────────────────────────────────────────────┘
```

### StorageNodeEndpoint

```
┌──────────────────────────────────────────────────────────────┐
│ StorageNodeEndpoint                                          │
├──────────────────────────────────────────────────────────────┤
│ url_len: u16                   │ URL length                  │
├────────────────────────────────┼─────────────────────────────┤
│ url: [u8; url_len]             │ HTTP RPC URL (UTF-8)        │
├────────────────────────────────┼─────────────────────────────┤
│ last_verified: u64             │ Unix timestamp              │
├────────────────────────────────┼─────────────────────────────┤
│ latency_ms: u32                │ Measured latency            │
├────────────────────────────────┼─────────────────────────────┤
│ ttl_secs: u32                  │ Time-to-live (default 300)  │
└──────────────────────────────────────────────────────────────┘
```

## Constraints

| Constraint | Value | Rationale |
|------------|-------|-----------|
| Max message size | 4096 bytes | Torオーバーヘッド考慮、効率的な伝播 |
| Max endpoints per message | 20 | 4KB制限内で十分な情報量 |
| Max URL length | 256 bytes | 一般的なURLに十分 |
| Message TTL | 60 seconds | Gossipsubデフォルト |
| Min publish interval | 10 seconds | スパム防止 |

## Signature Scheme

### Signing Process

1. メッセージデータを構築（signature フィールドを除く）
2. データをバイナリシリアライズ
3. Ed25519秘密鍵で署名
4. 署名をメッセージに追加

```rust
fn sign_message(msg: &mut EndpointMessage, keypair: &Keypair) {
    let data_to_sign = serialize_without_signature(msg);
    msg.signature = keypair.sign(&data_to_sign);
}

fn verify_message(msg: &EndpointMessage, peer_id: &PeerId) -> bool {
    let public_key = peer_id.as_public_key()?;
    let data = serialize_without_signature(msg);
    public_key.verify(&data, &msg.signature)
}
```

### Verification Process

1. sender_peer_id からEd25519公開鍵を抽出
2. メッセージデータ（signature除く）をシリアライズ
3. 署名を検証
4. 失敗時: Reputation Score を -20

## Processing Rules

### On Message Receive

```
1. Check message size <= 4096 bytes
   └─ Reject if over: log warning, skip

2. Deserialize message
   └─ Reject if invalid: log warning, skip

3. Check sender reputation >= 50
   └─ Reject if below: log debug, skip

4. Verify Ed25519 signature
   └─ If invalid:
      - reputation[sender] -= 20
      - log warning, skip

5. Check timestamp within ±60 seconds
   └─ Reject if stale: log debug, skip

6. For each endpoint:
   a. Verify chain_id matches expected genesis hash
   b. Check TTL not expired
   c. Verify URL format (ws:// or wss://)
   d. Update local endpoint cache (merge)

7. reputation[sender] += 1 (cap at 100)

8. Log info: "Received N endpoints from {sender}"
```

### On Endpoint Discovery

```
1. Connect to new endpoint
2. Verify chain_id via system_chain RPC
3. Measure latency (avg of 3 pings)
4. Add to local cache with TTL
5. If significant change (new or latency delta > 50ms):
   - Build EndpointMessage
   - Sign with node's Ed25519 key
   - Publish to /anarchy/endpoints/1.0.0
```

## Serialization Format

JSON over UTF-8 (for debugging convenience, production may use bincode)

```json
{
  "version": 1,
  "sender_peer_id": "12D3KooW...",
  "timestamp": 1707900000,
  "endpoints": [
    {
      "url": "ws://127.0.0.1:9944",
      "chain_id": "0x91b171bb158e2d3848fa23a9f1c25182fb8e20313b2c1eb49219da7a70ce90c3",
      "last_verified": 1707899990,
      "latency_ms": 15,
      "ttl_secs": 300
    }
  ],
  "signature": "0x..."
}
```

## Error Handling

| Error | Action | Reputation Impact |
|-------|--------|-------------------|
| Oversized message | Drop silently | None |
| Invalid signature | Drop, log | -20 |
| Expired timestamp | Drop silently | None |
| Invalid chain_id | Skip endpoint | None |
| Invalid URL | Skip endpoint | None |
| Sender reputation < 50 | Drop silently | None |

## Gossipsub Configuration

```rust
let gossipsub_config = ConfigBuilder::default()
    .heartbeat_interval(Duration::from_secs(1))
    .validation_mode(ValidationMode::Strict)
    .message_id_fn(|msg| {
        // Use hash of sender + timestamp for deduplication
        let mut hasher = blake2b_simd::Params::new()
            .hash_length(32)
            .to_state();
        hasher.update(&msg.source.to_bytes());
        hasher.update(&msg.data[..8]); // timestamp portion
        MessageId::from(hasher.finalize().as_bytes().to_vec())
    })
    .max_transmit_size(4096)
    .build()
    .expect("valid config");
```

## Test Scenarios

### T-001: Valid Message Propagation
1. Node A discovers endpoint X
2. Node A publishes EndpointMessage
3. Node B receives and validates
4. Node B's cache includes endpoint X
5. Node B's Node A reputation = 101

### T-002: Invalid Signature Rejection
1. Node A sends message with tampered signature
2. Node B detects invalid signature
3. Node B drops message
4. Node B's Node A reputation = 80 (100 - 20)

### T-003: Low Reputation Ignore
1. Node A repeatedly sends invalid messages
2. Node A reputation drops to 40
3. Node B ignores all messages from Node A
4. Valid messages from Node A are not processed

### T-004: TTL Expiry
1. Node A shares endpoint with TTL=60s
2. Node B receives and caches
3. After 60s, endpoint is marked expired
4. GC removes expired endpoint from cache

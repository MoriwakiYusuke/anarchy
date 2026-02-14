# Data Model: マルチノード対応とストレージセキュリティ

**Feature**: 010-multi-node-storage  
**Date**: 2026-02-14

## On-Chain Entities (Storage Pallet)

### Existing Entities (Modifications)

#### FragmentMetadata
既存のまま変更なし。

```rust
pub struct FragmentMetadata<T: Config> {
    pub size: u32,
    pub creator: T::AccountId,
    pub created_at: BlockNumberFor<T>,
}
```

#### StorageNodeInfo
**Modified**: PoW検証用フィールド追加

```rust
pub struct StorageNodeInfo<T: Config> {
    pub operator: T::AccountId,
    pub capacity: u64,
    pub registered_at: BlockNumberFor<T>,
    // NEW: PoW nonce used for registration
    pub pow_nonce: u64,
}
```

### New Entities

#### RegistrationCountPerBlock
PoW動的難易度計算用の登録カウンター

```rust
/// Block number → registration count in that block
#[pallet::storage]
pub type RegistrationCountPerBlock<T: Config> = 
    StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, u32, ValueQuery>;
```

#### DeclareHoldingCountPerBlock
declare_holdingレート制限用カウンター

```rust
/// (Block number, PeerID) → declaration count
#[pallet::storage]
pub type DeclareHoldingCountPerBlock<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat, BlockNumberFor<T>,
    Blake2_128Concat, BoundedVec<u8, T::MaxPeerIdLen>,
    u32,
    ValueQuery,
>;
```

### New Errors

```rust
pub enum Error<T> {
    // ... existing errors ...
    
    /// PoW nonce does not meet current difficulty
    InvalidPow,
    /// Too many node registrations this block
    TooManyRegistrationsThisBlock,
    /// Too many holding declarations this block
    TooManyDeclarationsThisBlock,
    /// Node capacity below minimum (1GB)
    CapacityTooSmall,
    /// PeerID too short (< 38 bytes)
    PeerIdTooShort,
    /// PeerID too long (> 64 bytes)  
    PeerIdTooLong,
}
```

### New Config Constants

```rust
pub trait Config: frame_system::Config {
    // ... existing ...
    
    /// Minimum PeerID length (default: 38)
    #[pallet::constant]
    type MinPeerIdLen: Get<u32>;
    
    /// Maximum node registrations per block (default: 5)
    #[pallet::constant]
    type MaxRegistrationsPerBlock: Get<u32>;
    
    /// Maximum holding declarations per block per node (default: 10)
    #[pallet::constant]
    type MaxDeclarationsPerBlockPerNode: Get<u32>;
    
    /// Minimum node capacity in bytes (default: 1GB)
    #[pallet::constant]
    type MinNodeCapacity: Get<u64>;
    
    /// PoW observation period in blocks (default: 10)
    #[pallet::constant]
    type PowObservationPeriod: Get<u32>;
    
    /// Base PoW difficulty (leading zero bits, default: 12)
    #[pallet::constant]
    type BasePowDifficulty: Get<u8>;
}
```

---

## Off-Chain Entities (Storage Node)

### BlockchainEndpoint

ブロックチェーンノード情報（Gossipsubで共有）

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockchainEndpoint {
    /// WebSocket RPC URL
    pub url: String,             // max 256 bytes
    
    /// Chain genesis hash (for verification)
    pub chain_id: [u8; 32],
    
    /// Last successful health check timestamp
    pub last_verified: u64,
    
    /// Average latency in milliseconds
    pub latency_ms: u32,
    
    /// Time-to-live in seconds (default: 300)
    pub ttl_secs: u32,
}
```

### EndpointMessage

Gossipsubメッセージ構造

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EndpointMessage {
    /// List of known endpoints (max 20)
    pub endpoints: Vec<BlockchainEndpoint>,
    
    /// Sender's PeerID
    pub sender_peer_id: PeerId,
    
    /// Message timestamp
    pub timestamp: u64,
    
    /// Ed25519 signature of (sender_peer_id || timestamp || hash(endpoints))
    pub signature: [u8; 64],
}

impl EndpointMessage {
    /// Maximum serialized size (4KB)
    pub const MAX_SIZE: usize = 4096;
    
    /// Maximum endpoints per message
    pub const MAX_ENDPOINTS: usize = 20;
}
```

### PeerReputation

ピア評価情報

```rust
#[derive(Clone, Debug)]
pub struct PeerReputation {
    /// Peer identifier
    pub peer_id: PeerId,
    
    /// Current reputation score (0-100, initial: 100)
    pub score: i32,
    
    /// Last update timestamp
    pub last_updated: Instant,
    
    /// Count of invalid messages
    pub invalid_count: u32,
    
    /// Count of valid messages
    pub valid_count: u32,
}

impl PeerReputation {
    pub const INITIAL_SCORE: i32 = 100;
    pub const INVALID_PENALTY: i32 = -20;
    pub const VALID_REWARD: i32 = 1;
    pub const IGNORE_THRESHOLD: i32 = 50;
}
```

### ConnectionState

ブロックチェーン接続状態マシン

```rust
#[derive(Clone, Debug)]
pub enum ConnectionRole {
    Primary,
    HotStandby,
    Disconnected,
}

#[derive(Clone, Debug)]
pub struct ConnectionState {
    /// Endpoint URL
    pub endpoint: BlockchainEndpoint,
    
    /// Current role
    pub role: ConnectionRole,
    
    /// PAPI client (if connected)
    pub client: Option<PolkadotClient>,
    
    /// Consecutive liveness check failures
    pub failure_count: u32,
    
    /// Last successful communication
    pub last_success: Instant,
}

impl ConnectionState {
    /// Liveness check interval
    pub const CHECK_INTERVAL: Duration = Duration::from_secs(2);
    
    /// Liveness check timeout
    pub const CHECK_TIMEOUT: Duration = Duration::from_secs(2);
    
    /// Failure threshold for failover
    pub const FAILURE_THRESHOLD: u32 = 3;
}
```

### SignedRequest

HTTP API認証リクエスト

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedRequest {
    /// Sr25519 public key (AccountId)
    pub account_id: [u8; 32],
    
    /// Unix timestamp in seconds
    pub timestamp: u64,
    
    /// 128-bit random nonce
    pub nonce: [u8; 16],
    
    /// Blake2b hash of request body
    pub payload_hash: [u8; 32],
    
    /// Sr25519 signature
    pub signature: [u8; 64],
}

impl SignedRequest {
    /// Signature validity period
    pub const VALIDITY_SECS: u64 = 300; // 5 minutes
}
```

### NonceCache

リプレイ攻撃防止用ノンスキャッシュ

```rust
pub struct NonceCache {
    /// nonce → expiry timestamp
    cache: DashMap<[u8; 16], u64>,
    
    /// Cache TTL (same as signature validity)
    ttl_secs: u64,
}

impl NonceCache {
    /// Check if nonce is fresh (not seen before)
    pub fn check_and_insert(&self, nonce: [u8; 16]) -> bool;
    
    /// Run garbage collection (remove expired entries)
    pub fn gc(&self);
}
```

---

## Frontend Entities

### FragmentPlacement

断片配置状態

```typescript
interface FragmentPlacement {
  fragmentId: string;       // hex-encoded 32 bytes
  fragmentIndex: number;    // 0 to n-1
  nodeId: string;           // PeerID (base58)
  nodeEndpoint: string;     // HTTP endpoint URL
  status: FragmentStatus;
  lastVerified?: Date;
}

enum FragmentStatus {
  Uploading = 'uploading',
  Stored = 'stored',
  Unreachable = 'unreachable',
  Failed = 'failed',
}
```

### StorageHealthStatus

投稿の健全性ステータス

```typescript
interface StorageHealthStatus {
  totalFragments: number;           // n (e.g., 5)
  reachableFragments: number;       // currently reachable
  requiredFragments: number;        // k (e.g., 3)
  isRecoverable: boolean;           // reachable >= required
  placements: FragmentPlacement[];
}
```

---

## State Transitions

### PoW Difficulty State

```
Initial State:
  - RegistrationCountPerBlock empty
  - Difficulty = BasePowDifficulty (12)

On register_node:
  1. Calculate current difficulty from last 10 blocks
  2. Verify PoW meets difficulty
  3. Increment RegistrationCountPerBlock[current_block]
  4. Check MaxRegistrationsPerBlock not exceeded

Difficulty Formula:
  recent = sum(RegistrationCountPerBlock[current-9..current])
  difficulty = min(12 + recent/5, 24)
```

### Connection Failover State

```
State: PRIMARY_CONNECTED
  - Liveness check every 2s
  - On success: reset failure_count
  - On failure: increment failure_count
  - If failure_count >= 3: transition to FAILING_OVER

State: FAILING_OVER
  - Select best Hot Standby
  - Promote to Primary
  - Demote old Primary to Disconnected
  - Transition to PRIMARY_CONNECTED

State: HOT_STANDBY
  - Maintain handshake with endpoint
  - Ready for immediate promotion
  - Periodic health verification (30s)
```

### Reputation State

```
Initial State:
  - score = 100
  - invalid_count = 0
  - valid_count = 0

On valid message:
  - score = min(score + 1, 100)
  - valid_count++

On invalid message:
  - score = score - 20
  - invalid_count++
  - If score <= 50: mark as ignored

Ignored State:
  - All messages from this peer are dropped
  - Can recover if score increases above 50 (manual reset)
```

---

## Validation Rules

### PeerID Validation (FR-405)
- Length: 38 <= len <= 64 bytes
- First byte: valid multicodec prefix (0x00 for identity, 0x12 for SHA256)

### Capacity Validation (FR-411)
- Minimum: 1,073,741,824 bytes (1GB)
- Maximum: 2^64 - 1 bytes

### PoW Validation (FR-409)
- Hash: Blake2b-256(peer_id || nonce.to_le_bytes())
- Difficulty: first N bits must be zero (N = calculated difficulty)

### Signature Validation (FR-201-205)
- Timestamp: within ±300 seconds of server time
- Nonce: not in cache (128-bit, checked against TTL cache)
- Payload hash: matches Blake2b-256 of request body
- Signature: valid Sr25519 signature over (timestamp || nonce || payload_hash)

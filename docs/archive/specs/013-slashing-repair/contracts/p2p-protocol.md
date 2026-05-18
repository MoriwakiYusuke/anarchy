# P2P Protocol Contracts: 自己修復プロトコル

**Feature**: 013-slashing-repair  
**Created**: 2026-02-24

## Overview

storage-node間のlibp2p request-responseプロトコル仕様。

---

## Protocol ID

```
/anarchy/repair/1.0.0
```

---

## Message Types

### 1. RepairRequest

**Direction**: Coordinator → Donor (k nodes in parallel)

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepairRequest {
    /// Target fragment ID (content hash)
    pub fragment_id: [u8; 32],
    
    /// Coordinator's public key for authentication
    pub coordinator_pubkey: [u8; 32],
    
    /// Timestamp for replay protection
    pub timestamp_ms: u64,
    
    /// Signature of (fragment_id || coordinator_pubkey || timestamp_ms)
    pub signature: [u8; 64],
    
    /// Requested share indices (usually single, but can request multiple)
    pub requested_indices: Vec<u8>,
}
```

**Validation**:
1. `timestamp_ms`が現在時刻±60秒以内
2. `signature`が`coordinator_pubkey`で検証可能
3. `coordinator_pubkey`がon-chainで登録済みノードのもの

---

### 2. RepairResponse

**Direction**: Donor → Coordinator

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepairResponse {
    /// Original request fragment ID echo
    pub fragment_id: [u8; 32],
    
    /// Response status
    pub status: RepairResponseStatus,
    
    /// VSS share data (if Success)
    pub share: Option<VssShareData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RepairResponseStatus {
    /// Share successfully retrieved
    Success,
    
    /// Fragment not held by this node
    NotHolder,
    
    /// Fragment data corrupted or unavailable
    DataUnavailable,
    
    /// Request validation failed
    InvalidRequest,
    
    /// Rate limited
    RateLimited,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VssShareData {
    /// Share index (0-based, from VSS split)
    pub index: u8,
    
    /// Share value (field element bytes)
    pub value: [u8; 32],
    
    /// Optional commitment proof
    pub commitment_proof: Option<Vec<u8>>,
}
```

---

### 3. SharePushRequest（オプショナル）

**Direction**: Coordinator → New Holder

**Purpose**: 再生成したシェアを新ホルダーに配信

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SharePushRequest {
    /// Fragment ID
    pub fragment_id: [u8; 32],
    
    /// The regenerated VSS share
    pub share: VssShareData,
    
    /// KZG proof for the share
    pub kzg_proof: Vec<u8>,
    
    /// Coordinator's signature
    pub coordinator_signature: [u8; 64],
    
    /// Timestamp
    pub timestamp_ms: u64,
}
```

**Response**: ACK/NACK

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SharePushResponse {
    Accepted,
    Rejected { reason: String },
}
```

---

## Sequence Diagram

```
┌────────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌───────────┐
│Coordinator │     │ Donor1  │     │ Donor2  │     │ Donor3  │     │New Holder │
└─────┬──────┘     └────┬────┘     └────┬────┘     └────┬────┘     └─────┬─────┘
      │                 │               │               │                 │
      │ RepairRequest   │               │               │                 │
      │────────────────>│               │               │                 │
      │────────────────────────────────>│               │                 │
      │────────────────────────────────────────────────>│                 │
      │                 │               │               │                 │
      │ RepairResponse  │               │               │                 │
      │<────────────────│               │               │                 │
      │<────────────────────────────────│               │                 │
      │<────────────────────────────────────────────────│                 │
      │                 │               │               │                 │
      │ (k=3 shares collected, regenerate new share)    │                 │
      │                 │               │               │                 │
      │ SharePushRequest│               │               │                 │
      │─────────────────────────────────────────────────────────────────>│
      │                 │               │               │                 │
      │ SharePushResponse (Accepted)    │               │                 │
      │<─────────────────────────────────────────────────────────────────│
      │                 │               │               │                 │
      │ confirm_repair(fragment_id, new_holder, ...)                     │
      │═══════════════════════════════════════════════════════════>CHAIN │
```

---

## Error Handling

### Timeout Strategy

| Phase | Timeout | Action on Timeout |
|-------|---------|-------------------|
| RepairRequest | 30s per donor | Retry with next donor |
| Collect k shares | 5min total | Abort, retry next block |
| SharePush | 30s | Retry 3x, then select another new holder |
| confirm_repair | 60min total | Abort, report failure (penalty if coordinator) |

### Retry Logic

```rust
const MAX_DONOR_RETRIES: u32 = 2;
const MAX_COORDINATOR_ATTEMPTS: u32 = 3;

async fn repair_with_retry(fragment_id: [u8; 32], donors: Vec<PeerId>) -> Result<()> {
    let mut collected_shares = Vec::new();
    let mut tried_donors = HashSet::new();
    
    while collected_shares.len() < 3 {
        // Select next untried donor
        let donor = donors.iter()
            .find(|d| !tried_donors.contains(*d))
            .ok_or(Error::InsufficientDonors)?;
        
        tried_donors.insert(donor.clone());
        
        match request_share(donor, fragment_id).await {
            Ok(share) => collected_shares.push(share),
            Err(e) => {
                log::warn!("Donor {} failed: {:?}", donor, e);
                // Continue to next donor
            }
        }
    }
    
    // Regenerate and push
    Ok(())
}
```

---

## Rate Limiting

### Per-Peer Limits

```rust
struct RepairRateLimiter {
    /// Max requests per fragment per hour
    per_fragment_per_hour: u32,  // default: 3
    
    /// Max total repair requests per peer per hour
    per_peer_per_hour: u32,  // default: 100
    
    /// Window for tracking
    window_seconds: u64,  // default: 3600
}
```

### Response

When rate limited:
```rust
RepairResponse {
    fragment_id,
    status: RepairResponseStatus::RateLimited,
    share: None,
}
```

---

## Security Considerations

### 1. Authentication

- All requests must be signed by registered node's key
- Verify coordinator is legitimate (on-chain check before responding)
- No anonymous repair requests

### 2. Replay Protection

- Timestamp must be within ±60 seconds of current time
- Keep seen-requests cache for 2 minutes to reject exact duplicates
- Include fragment_id in signature to prevent cross-fragment replay

### 3. Share Leakage Prevention

- Only respond to at most `n-k+1 = 3` unique coordinators per fragment per hour
- Log all share distributions for audit
- Consider encrypting repsonse to coordinator's ephemeral key

### 4. Denial of Service

- Rate limiting as specified above
- Priority queue for legitimate repair requests (AtRisk fragments first)
- Ignore requests from nodes with low on-chain score

---

## Wire Format

All messages serialized with **bincode** or **SCALE** (consistent with substrate):

```rust
// Using SCALE codec for substrate compatibility
use parity_scale_codec::{Encode, Decode};

#[derive(Encode, Decode)]
pub struct RepairRequest { ... }

#[derive(Encode, Decode)]
pub struct RepairResponse { ... }
```

**Framing**: Standard libp2p length-prefixed frames.

---

## Integration Points

### storage-node Modules

1. **`src/repair/coordinator.rs`**
   - Initiate repair requests
   - Collect k shares
   - Call `regenerate_share`
   - Push to new holder
   - Submit `confirm_repair` tx

2. **`src/repair/donor.rs`**
   - Handle incoming RepairRequest
   - Validate and respond with share
   - Rate limiting

3. **`src/repair/receiver.rs`**
   - Handle incoming SharePushRequest
   - Store new share locally
   - Respond with ACK

4. **`src/repair/discovery.rs`**
   - Query Runtime API for AtRisk fragments
   - Select repair candidates
   - Coordinate scheduling

# Phase 1 Technical Research: Storage Node Daemon Implementation

**Status**: Research Complete  
**Date**: 2026-02-09  
**Focus**: libp2p request-response, subxt, local storage, PeerID

---

## 1. libp2p request-response Protocol

### Decision
Use `libp2p::request_response` with a custom `FragmentProtocol` for fragment transfer.

### Rationale
- request-response is the standard pattern for point-to-point data exchange in libp2p
- Built-in request/response correlation, timeouts, and back-pressure
- Suitable for 1MB fragments (configurable max message size)
- Already proven in IPFS Bitswap, Filecoin retrieval

### Code Structure

```rust
use libp2p::{
    identity, noise, request_response, swarm::SwarmBuilder, tcp, yamux, PeerId,
};
use std::time::Duration;

// Protocol definition
#[derive(Debug, Clone)]
pub struct FragmentProtocol;

impl request_response::ProtocolName for FragmentProtocol {
    fn protocol_name(&self) -> &[u8] {
        b"/anarchy/fragment/1.0.0"
    }
}

// Request/Response types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FragmentRequest {
    /// Request a fragment by ID
    Get { fragment_id: [u8; 32] },
    /// Store a fragment
    Put { fragment_id: [u8; 32], data: Vec<u8> },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FragmentResponse {
    /// Fragment data
    Data(Vec<u8>),
    /// Fragment not found
    NotFound,
    /// Storage success
    Stored,
    /// Storage failed (e.g., capacity exceeded)
    StorageFailed { reason: String },
}

// Codec for serialization (using serde + length-prefixed framing)
#[derive(Clone)]
pub struct FragmentCodec {
    max_request_size: usize,
    max_response_size: usize,
}

impl Default for FragmentCodec {
    fn default() -> Self {
        Self {
            max_request_size: 2 * 1024 * 1024,  // 2MB (fragment + overhead)
            max_response_size: 2 * 1024 * 1024, // 2MB
        }
    }
}

// SwarmBuilder setup
async fn build_swarm(keypair: identity::Keypair) -> Result<Swarm<FragmentBehaviour>, Error> {
    let peer_id = PeerId::from(keypair.public());
    
    let behaviour = FragmentBehaviour::new(
        request_response::Behaviour::new(
            [(FragmentProtocol, request_response::ProtocolSupport::Full)],
            request_response::Config::default()
                .with_request_timeout(Duration::from_secs(60))
                .with_max_concurrent_streams(100),
        ),
    );
    
    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|_| behaviour)?
        .with_swarm_config(|cfg| {
            cfg.with_idle_connection_timeout(Duration::from_secs(60))
        })
        .build();
    
    Ok(swarm)
}
```

### Large Data Transfer Best Practices

| Concern | Solution |
|---------|----------|
| Memory pressure | Stream-based codec with length-prefixed framing |
| Timeout | 60s for 1MB @ 150KB/s minimum |
| Concurrent requests | Limit to 100 concurrent streams |
| Backpressure | libp2p's yamux handles flow control |
| Verification | Hash verification after receipt |

### Alternatives Considered
1. **GossipSub**: For broadcast, not point-to-point transfer
2. **Kademlia**: For DHT lookup, not data transfer
3. **Custom TCP**: Loses libp2p's peer discovery, NAT traversal

---

## 2. subxt Transaction Submission

### Decision
Use `subxt` crate for Rust-based extrinsic submission. Note: existing scripts use JavaScript `polkadot-api`, but daemon should use native Rust.

### Rationale
- `subxt` is the official Substrate Rust client library
- Type-safe metadata-based API generation
- Supports Ed25519/Sr25519 signing
- Async/await with tokio runtime (matches libp2p)

### Existing Pattern (JavaScript Reference)
The project uses `polkadot-api` in [scripts/](scripts/) for dev tooling:

```javascript
// From transfer-native.mjs - signing pattern
const entropy = mnemonicToEntropy(DEV_PHRASE);
const miniSecret = entropyToMiniSecret(entropy);
const derive = sr25519CreateDerive(miniSecret);
const aliceKeyPair = derive('//Alice');

const signer = getPolkadotSigner(
    aliceKeyPair.publicKey,
    'Sr25519',
    (input) => aliceKeyPair.sign(input)
);

const result = await transferTx.signAndSubmit(signer);
```

### Rust subxt Pattern

```rust
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::Keypair;

// Generate metadata at compile time (optional, but type-safe)
#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod anarchy_runtime {}

// Or use dynamic metadata (simpler, but less type-safe)
pub async fn submit_holding_declaration(
    api: &OnlineClient<PolkadotConfig>,
    signer: &Keypair,
    fragment_id: [u8; 32],
    peer_id: Vec<u8>,
) -> Result<(), subxt::Error> {
    // For dynamic calls (if metadata isn't pre-generated)
    let call = subxt::dynamic::tx(
        "Storage",
        "declare_holding",
        vec![
            ("fragment_id", subxt::dynamic::Value::from_bytes(fragment_id)),
            ("peer_id", subxt::dynamic::Value::from_bytes(peer_id)),
        ],
    );
    
    let result = api
        .tx()
        .sign_and_submit_then_watch_default(&call, signer)
        .await?
        .wait_for_finalized_success()
        .await?;
    
    println!("Holding declared in block: {:?}", result.block_hash());
    Ok(())
}

// Ed25519 keypair from seed
pub fn keypair_from_seed(seed: &[u8; 32]) -> subxt_signer::sr25519::Keypair {
    subxt_signer::sr25519::Keypair::from_seed(*seed).expect("valid seed")
}

// Or from mnemonic
pub fn keypair_from_mnemonic(phrase: &str, path: &str) -> subxt_signer::sr25519::Keypair {
    subxt_signer::sr25519::Keypair::from_phrase(phrase, None)
        .expect("valid mnemonic")
        .derive(path.as_bytes())
}
```

### Connection Setup

```rust
use subxt::{OnlineClient, PolkadotConfig};

pub async fn connect_to_chain(url: &str) -> Result<OnlineClient<PolkadotConfig>, subxt::Error> {
    // For custom chain, may need custom config
    OnlineClient::<PolkadotConfig>::from_url(url).await
}

// With retry logic
pub async fn connect_with_retry(url: &str, max_retries: u32) -> Result<OnlineClient<PolkadotConfig>, Error> {
    let mut attempts = 0;
    loop {
        match OnlineClient::<PolkadotConfig>::from_url(url).await {
            Ok(client) => return Ok(client),
            Err(e) if attempts < max_retries => {
                attempts += 1;
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}
```

### Alternatives Considered
1. **polkadot-api (JS)**: Not suitable for Rust daemon
2. **substrate-api-client**: Less maintained than subxt
3. **Direct RPC calls**: No type safety, more boilerplate

---

## 3. Local Storage for Fragments

### Decision
Use **direct file I/O with directory hierarchy** (not embedded database).

### Rationale
- Fragments are 1KB–1MB, not tiny KV pairs
- Simple filesystem is more debuggable and recoverable
- No DB compaction overhead for large values
- OS page cache handles read optimization
- Easier backup/migration (just copy files)

### Directory Structure

```
$STORAGE_DATA_DIR/
├── config.toml              # Node configuration
├── identity/
│   ├── peer_id              # PeerID (base58)
│   └── keypair.bin          # Ed25519 keypair (encrypted)
├── fragments/
│   ├── 00/                  # First 2 hex chars of fragment_id
│   │   ├── 00/              # Next 2 hex chars
│   │   │   ├── 00abcd...ef.bin
│   │   │   └── 00abcd...ff.bin
│   │   └── 01/
│   └── ff/
├── index/
│   └── fragments.db         # sled/sqlite for metadata index (optional)
└── logs/
    └── storage.log
```

### Code Example

```rust
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct FragmentStore {
    base_path: PathBuf,
    max_capacity_bytes: u64,
    current_usage: AtomicU64,
}

impl FragmentStore {
    pub fn new(base_path: PathBuf, max_capacity_bytes: u64) -> Self {
        Self {
            base_path,
            max_capacity_bytes,
            current_usage: AtomicU64::new(0),
        }
    }
    
    fn fragment_path(&self, fragment_id: &[u8; 32]) -> PathBuf {
        let hex = hex::encode(fragment_id);
        self.base_path
            .join("fragments")
            .join(&hex[0..2])
            .join(&hex[2..4])
            .join(format!("{}.bin", hex))
    }
    
    pub async fn store(&self, fragment_id: [u8; 32], data: &[u8]) -> Result<(), StorageError> {
        // Check capacity
        let new_usage = self.current_usage.load(Ordering::Relaxed) + data.len() as u64;
        if new_usage > self.max_capacity_bytes {
            return Err(StorageError::CapacityExceeded);
        }
        
        // Verify hash matches fragment_id
        let hash = blake2b_256(data);
        if hash != fragment_id {
            return Err(StorageError::HashMismatch);
        }
        
        let path = self.fragment_path(&fragment_id);
        
        // Create parent directories
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Atomic write via temp file
        let temp_path = path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(data).await?;
        file.sync_all().await?;
        fs::rename(&temp_path, &path).await?;
        
        self.current_usage.fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(())
    }
    
    pub async fn retrieve(&self, fragment_id: &[u8; 32]) -> Result<Vec<u8>, StorageError> {
        let path = self.fragment_path(fragment_id);
        
        if !path.exists() {
            return Err(StorageError::NotFound);
        }
        
        let mut file = fs::File::open(&path).await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        
        Ok(data)
    }
    
    pub async fn exists(&self, fragment_id: &[u8; 32]) -> bool {
        self.fragment_path(fragment_id).exists()
    }
    
    pub async fn delete(&self, fragment_id: &[u8; 32]) -> Result<(), StorageError> {
        let path = self.fragment_path(fragment_id);
        let metadata = fs::metadata(&path).await?;
        fs::remove_file(&path).await?;
        self.current_usage.fetch_sub(metadata.len(), Ordering::Relaxed);
        Ok(())
    }
}
```

### Alternatives Considered

| Option | Pros | Cons | Decision |
|--------|------|------|----------|
| **File I/O** | Simple, debuggable, no compaction | Need manual indexing | ✅ Selected |
| **sled** | Embedded, ACID, good for small values | Compaction overhead for 1MB values | ❌ Not for fragments |
| **RocksDB** | Battle-tested, good for large values | Complex config, LSM overhead | ❌ Overkill |
| **mmap** | Zero-copy reads | Complex lifecycle, Linux-specific optimizations | ❌ Premature optimization |

### Index Strategy (Optional)
For fast fragment listing/querying, use sled for metadata only:

```rust
// Index stores: fragment_id -> FragmentMetadata
// FragmentMetadata: { size: u64, stored_at: u64, last_accessed: u64 }
pub struct FragmentIndex {
    db: sled::Db,
}
```

---

## 4. PeerID Format & Persistence

### Decision
Use libp2p standard Ed25519-based PeerID with 12D3KooW prefix.

### Rationale
- Standard libp2p format ensures interoperability
- Ed25519 is fast, secure, and has small keys (32 bytes)
- PeerID derived from public key = consistent identity
- 12D3KooW prefix indicates Ed25519 key type (multicodec)

### PeerID Generation

```rust
use libp2p::identity::{Keypair, ed25519, PeerId};
use std::path::Path;
use tokio::fs;

pub struct NodeIdentity {
    pub keypair: Keypair,
    pub peer_id: PeerId,
}

impl NodeIdentity {
    /// Generate new identity
    pub fn generate() -> Self {
        let keypair = Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        Self { keypair, peer_id }
    }
    
    /// Load from file or generate new
    pub async fn load_or_generate(path: &Path) -> Result<Self, Error> {
        let keypair_path = path.join("keypair.bin");
        
        if keypair_path.exists() {
            let bytes = fs::read(&keypair_path).await?;
            let keypair = Keypair::from_protobuf_encoding(&bytes)?;
            let peer_id = PeerId::from(keypair.public());
            Ok(Self { keypair, peer_id })
        } else {
            let identity = Self::generate();
            fs::create_dir_all(path).await?;
            let bytes = identity.keypair.to_protobuf_encoding()?;
            fs::write(&keypair_path, &bytes).await?;
            
            // Also save peer_id as text for easy reference
            let peer_id_path = path.join("peer_id");
            fs::write(&peer_id_path, identity.peer_id.to_base58()).await?;
            
            Ok(identity)
        }
    }
}
```

### PeerID Format Details

```
Format: 12D3KooW[44 base58 chars]

Example: 12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN

Breakdown:
- "12D3KooW" = multicodec prefix for Ed25519 public key
- Remaining = base58-encoded Ed25519 public key (32 bytes)
- Total length: 52 characters

Encoding:
  multicodec(0x00) + multihash(identity, ed25519-pub-key) -> base58btc
```

### Onchain Storage Consideration
For storing PeerID onchain, use bytes representation:

```rust
// In pallet-storage
#[pallet::storage]
pub type StorageNodes<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BoundedVec<u8, ConstU32<64>>,  // PeerID as bytes (max ~52 bytes)
    StorageNodeInfo<T>,
    OptionQuery,
>;

// Conversion
pub fn peer_id_to_bytes(peer_id: &PeerId) -> Vec<u8> {
    peer_id.to_bytes()  // Returns multihash bytes
}

pub fn peer_id_from_bytes(bytes: &[u8]) -> Result<PeerId, Error> {
    PeerId::from_bytes(bytes).map_err(|_| Error::InvalidPeerId)
}
```

---

## 5. Integration Architecture

### Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    Storage Node Daemon                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐        │
│  │   libp2p     │   │    subxt     │   │  FragmentStore│        │
│  │   Swarm      │   │   Client     │   │   (FileIO)   │        │
│  └──────┬───────┘   └──────┬───────┘   └──────┬───────┘        │
│         │                  │                  │                 │
│         │    ┌─────────────┴─────────────┐    │                 │
│         │    │      Event Loop           │    │                 │
│         └────┤   (tokio select!)         ├────┘                 │
│              │                           │                      │
│              └───────────────────────────┘                      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
            │                    │
            │ libp2p             │ JSON-RPC
            │ (TCP/QUIC)         │ (WebSocket)
            ▼                    ▼
    ┌───────────┐        ┌───────────────┐
    │ Other     │        │  Substrate    │
    │ Peers     │        │  Node         │
    └───────────┘        └───────────────┘
```

### Main Event Loop Structure

```rust
use tokio::select;

pub async fn run_daemon(
    mut swarm: Swarm<FragmentBehaviour>,
    chain_client: OnlineClient<PolkadotConfig>,
    fragment_store: FragmentStore,
    config: Config,
) -> Result<(), Error> {
    loop {
        select! {
            // Handle libp2p events
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &fragment_store, &chain_client).await?;
            }
            
            // Handle chain events (new blocks, etc.)
            block = chain_client.blocks().subscribe_finalized().next() => {
                if let Some(block) = block {
                    handle_new_block(block?, &config).await?;
                }
            }
            
            // Graceful shutdown
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting down...");
                break;
            }
        }
    }
    Ok(())
}
```

---

## 6. Cargo Dependencies

```toml
[dependencies]
# libp2p (P2P networking)
libp2p = { version = "0.54", features = [
    "tokio",
    "tcp",
    "noise",
    "yamux",
    "request-response",
    "macros",
    "ed25519",
] }

# subxt (Substrate client)
subxt = "0.37"
subxt-signer = { version = "0.37", features = ["subxt"] }

# Async runtime
tokio = { version = "1", features = ["full"] }
futures = "0.3"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Crypto
blake2 = "0.10"
hex = "0.4"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Config
config = "0.14"
toml = "0.8"

# Optional: metadata index
sled = "0.34"
```

---

## 7. Summary of Decisions

| Topic | Decision | Key Rationale |
|-------|----------|---------------|
| P2P Protocol | libp2p request-response | Standard, battle-tested, proper semantics |
| Chain Client | subxt (Rust) | Type-safe, official Substrate client |
| Fragment Storage | Direct file I/O with hierarchy | Simple, debuggable, good for large values |
| Metadata Index | Optional sled | Only if fast listing needed |
| PeerID | Ed25519-based (12D3KooW...) | libp2p standard, interoperable |
| Identity Persistence | Protobuf-encoded keypair file | libp2p native format |

---

## 8. Open Questions for Implementation

1. **Encryption at rest**: Should fragments be encrypted on disk? (Currently no, since fragments are already encrypted by client)
2. **Connection limits**: How many concurrent peer connections to allow?
3. **Bootstrap peers**: How to discover initial peers? (DHT? Static list? Chain-based registry?)
4. **Rate limiting**: Should PUT requests be rate-limited to prevent spam?
5. **Fragment expiry**: Should fragments have TTL? (Spec says Phase 2, but affects storage design)

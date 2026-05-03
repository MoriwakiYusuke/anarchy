# Storage Pallet API Contract

**Version**: 1.0.0 (Phase 1)  
**Generated**: 2026-02-09  
**Source**: [spec.md](../spec.md), [data-model.md](../data-model.md)

---

## 1. Overview

`pallet-storage`は分散ストレージシステムのオンチェーン部分を担当。断片メタデータの登録、ストレージノードの管理、保持表明の記録を行う。

### Module Path
```rust
pallet_storage
```

### Dependencies
- `frame_support`, `frame_system`
- `sp_runtime`, `sp_core`

---

## 2. Configuration (Config trait)

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    /// The overarching event type.
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

    /// Maximum fragment size in bytes (default: 1MB)
    #[pallet::constant]
    type MaxFragmentSize: Get<u32>;

    /// Maximum PeerID length in bytes (default: 64)
    #[pallet::constant]
    type MaxPeerIdLen: Get<u32>;

    /// Maximum holders per fragment (default: 100)
    #[pallet::constant]
    type MaxHoldersPerFragment: Get<u32>;

    /// Maximum fragments per node (default: 10,000)
    #[pallet::constant]
    type MaxFragmentsPerNode: Get<u32>;
}
```

---

## 3. Extrinsics (Dispatchable Calls)

### 3.1 `register_fragment`

断片メタデータをチェーンに登録する。

```rust
#[pallet::call_index(0)]
#[pallet::weight(T::WeightInfo::register_fragment())]
pub fn register_fragment(
    origin: OriginFor<T>,
    fragment_id: FragmentId,
    size: u32,
) -> DispatchResult;
```

| Parameter | Type | Validation |
|-----------|------|------------|
| `origin` | `OriginFor<T>` | Must be signed |
| `fragment_id` | `[u8; 32]` | Must not already exist |
| `size` | `u32` | Must be 1 ≤ size ≤ MaxFragmentSize |

**Events**:
- `FragmentRegistered { fragment_id, creator, size }`

**Errors**:
- `FragmentAlreadyExists` - Fragment ID already registered
- `FragmentTooLarge` - Size exceeds MaxFragmentSize
- `FragmentTooSmall` - Size is 0

**Example**:
```javascript
// Using polkadot.js
api.tx.storage.registerFragment(fragmentId, 1024).signAndSend(account);
```

---

### 3.2 `register_node`

ストレージノードをチェーンに登録する。

```rust
#[pallet::call_index(1)]
#[pallet::weight(T::WeightInfo::register_node())]
pub fn register_node(
    origin: OriginFor<T>,
    peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
    capacity: u64,
) -> DispatchResult;
```

| Parameter | Type | Validation |
|-----------|------|------------|
| `origin` | `OriginFor<T>` | Must be signed |
| `peer_id` | `BoundedVec<u8, MaxPeerIdLen>` | Valid libp2p PeerID, must not exist |
| `capacity` | `u64` | Must be > 0 |

**Events**:
- `NodeRegistered { peer_id, operator, capacity }`

**Errors**:
- `NodeAlreadyRegistered` - PeerID already registered
- `OperatorAlreadyHasNode` - Account already has a registered node
- `InvalidPeerId` - PeerID format invalid
- `InvalidCapacity` - Capacity is 0

**Example**:
```javascript
// PeerID as bytes
const peerId = api.createType('Vec<u8>', '0x...');
api.tx.storage.registerNode(peerId, 10_000_000_000).signAndSend(account);
```

---

### 3.3 `update_node`

登録済みノードの情報を更新する。

```rust
#[pallet::call_index(2)]
#[pallet::weight(T::WeightInfo::update_node())]
pub fn update_node(
    origin: OriginFor<T>,
    new_capacity: u64,
) -> DispatchResult;
```

| Parameter | Type | Validation |
|-----------|------|------------|
| `origin` | `OriginFor<T>` | Must be signed, must have registered node |
| `new_capacity` | `u64` | Must be > 0 |

**Events**:
- `NodeUpdated { peer_id, new_capacity }`

**Errors**:
- `NodeNotRegistered` - Caller has no registered node
- `InvalidCapacity` - Capacity is 0

---

### 3.4 `unregister_node`

ストレージノードの登録を解除する。

```rust
#[pallet::call_index(3)]
#[pallet::weight(T::WeightInfo::unregister_node())]
pub fn unregister_node(
    origin: OriginFor<T>,
) -> DispatchResult;
```

| Parameter | Type | Validation |
|-----------|------|------------|
| `origin` | `OriginFor<T>` | Must be signed, must have registered node |

**Events**:
- `NodeUnregistered { peer_id, operator }`

**Errors**:
- `NodeNotRegistered` - Caller has no registered node
- `NodeHasHoldings` - Node still has active holdings (must revoke all first)

**Note**: 保持表明が残っている場合は先に`revoke_holding`でクリアする必要がある。

---

### 3.5 `declare_holding`

特定の断片を保持していることを表明する。

```rust
#[pallet::call_index(4)]
#[pallet::weight(T::WeightInfo::declare_holding())]
pub fn declare_holding(
    origin: OriginFor<T>,
    fragment_id: FragmentId,
) -> DispatchResult;
```

| Parameter | Type | Validation |
|-----------|------|------------|
| `origin` | `OriginFor<T>` | Must be signed, must have registered node |
| `fragment_id` | `[u8; 32]` | Must exist in Fragments storage |

**Events**:
- `HoldingDeclared { peer_id, fragment_id }`

**Errors**:
- `NodeNotRegistered` - Caller has no registered node
- `FragmentNotFound` - Fragment ID does not exist
- `AlreadyHolding` - Already declared holding for this fragment

**Idempotency**: 既に保持表明済みの場合は成功を返す（べき等）。

---

### 3.6 `revoke_holding`

保持表明を取り消す。

```rust
#[pallet::call_index(5)]
#[pallet::weight(T::WeightInfo::revoke_holding())]
pub fn revoke_holding(
    origin: OriginFor<T>,
    fragment_id: FragmentId,
) -> DispatchResult;
```

| Parameter | Type | Validation |
|-----------|------|------------|
| `origin` | `OriginFor<T>` | Must be signed, must have registered node |
| `fragment_id` | `[u8; 32]` | Must exist and caller must be holding |

**Events**:
- `HoldingRevoked { peer_id, fragment_id }`

**Errors**:
- `NodeNotRegistered` - Caller has no registered node
- `NotHolding` - Not holding this fragment

---

## 4. Storage

### 4.1 `Fragments`

断片メタデータのマップ。

```rust
#[pallet::storage]
#[pallet::getter(fn fragments)]
pub type Fragments<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    FragmentId,
    FragmentMetadata<T>,
    OptionQuery,
>;
```

### 4.2 `StorageNodes`

ストレージノード情報のマップ。

```rust
#[pallet::storage]
#[pallet::getter(fn storage_nodes)]
pub type StorageNodes<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BoundedVec<u8, T::MaxPeerIdLen>,
    StorageNodeInfo<T>,
    OptionQuery,
>;
```

### 4.3 `OperatorNodes`

オペレーター → PeerIDの逆引きマップ。

```rust
#[pallet::storage]
#[pallet::getter(fn operator_nodes)]
pub type OperatorNodes<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<u8, T::MaxPeerIdLen>,
    OptionQuery,
>;
```

### 4.4 `FragmentHolders`

断片ID → 保持ノード一覧。

```rust
#[pallet::storage]
#[pallet::getter(fn fragment_holders)]
pub type FragmentHolders<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    FragmentId,
    BoundedVec<BoundedVec<u8, T::MaxPeerIdLen>, T::MaxHoldersPerFragment>,
    ValueQuery,
>;
```

### 4.5 `NodeHoldings`

ノードPeerID → 保持断片一覧。

```rust
#[pallet::storage]
#[pallet::getter(fn node_holdings)]
pub type NodeHoldings<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BoundedVec<u8, T::MaxPeerIdLen>,
    BoundedVec<FragmentId, T::MaxFragmentsPerNode>,
    ValueQuery,
>;
```

---

## 5. Events

```rust
#[pallet::event]
#[pallet::generate_deposit(pub(super) fn deposit_event)]
pub enum Event<T: Config> {
    /// Fragment registered
    FragmentRegistered {
        fragment_id: FragmentId,
        creator: T::AccountId,
        size: u32,
    },
    
    /// Storage node registered
    NodeRegistered {
        peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
        operator: T::AccountId,
        capacity: u64,
    },
    
    /// Storage node updated
    NodeUpdated {
        peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
        new_capacity: u64,
    },
    
    /// Storage node unregistered
    NodeUnregistered {
        peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
        operator: T::AccountId,
    },
    
    /// Holding declared
    HoldingDeclared {
        peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
        fragment_id: FragmentId,
    },
    
    /// Holding revoked
    HoldingRevoked {
        peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
        fragment_id: FragmentId,
    },
}
```

---

## 6. Errors

```rust
#[pallet::error]
pub enum Error<T> {
    /// Fragment ID already exists
    FragmentAlreadyExists,
    /// Fragment size exceeds maximum
    FragmentTooLarge,
    /// Fragment size is zero
    FragmentTooSmall,
    /// Fragment not found
    FragmentNotFound,
    
    /// Storage node already registered with this PeerID
    NodeAlreadyRegistered,
    /// Operator already has a registered node
    OperatorAlreadyHasNode,
    /// Storage node not registered
    NodeNotRegistered,
    /// Invalid PeerID format
    InvalidPeerId,
    /// Invalid capacity (zero)
    InvalidCapacity,
    /// Node has active holdings
    NodeHasHoldings,
    
    /// Already holding this fragment
    AlreadyHolding,
    /// Not holding this fragment
    NotHolding,
    /// Maximum holders reached for this fragment
    TooManyHolders,
    /// Maximum fragments reached for this node
    TooManyFragments,
}
```

---

## 7. RPC Methods (Read-only)

### 7.1 `storage_getFragment`

断片メタデータを取得。

```json
{
  "method": "storage_getFragment",
  "params": ["0x<fragment_id_hex>"],
  "response": {
    "size": 1024,
    "creator": "5GrwvaEF...",
    "created_at": 12345
  }
}
```

### 7.2 `storage_getNode`

ノード情報を取得。

```json
{
  "method": "storage_getNode", 
  "params": ["0x<peer_id_hex>"],
  "response": {
    "operator": "5GrwvaEF...",
    "capacity": 10000000000,
    "registered_at": 12345
  }
}
```

### 7.3 `storage_getFragmentHolders`

断片の保持ノード一覧を取得。

```json
{
  "method": "storage_getFragmentHolders",
  "params": ["0x<fragment_id_hex>"],
  "response": [
    "0x<peer_id_1_hex>",
    "0x<peer_id_2_hex>"
  ]
}
```

### 7.4 `storage_getNodeHoldings`

ノードが保持する断片一覧を取得。

```json
{
  "method": "storage_getNodeHoldings",
  "params": ["0x<peer_id_hex>"],
  "response": [
    "0x<fragment_id_1_hex>",
    "0x<fragment_id_2_hex>"
  ]
}
```

---

## 8. Runtime Integration

```rust
// runtime/src/lib.rs

parameter_types! {
    pub const MaxFragmentSize: u32 = 1024 * 1024; // 1MB
    pub const MaxPeerIdLen: u32 = 64;
    pub const MaxHoldersPerFragment: u32 = 100;
    pub const MaxFragmentsPerNode: u32 = 10_000;
}

impl pallet_storage::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxFragmentSize = MaxFragmentSize;
    type MaxPeerIdLen = MaxPeerIdLen;
    type MaxHoldersPerFragment = MaxHoldersPerFragment;
    type MaxFragmentsPerNode = MaxFragmentsPerNode;
}

construct_runtime!(
    pub enum Runtime {
        // ... existing pallets ...
        Storage: pallet_storage,
    }
);
```

---

## 9. Weight Estimation (Placeholder)

```rust
pub trait WeightInfo {
    fn register_fragment() -> Weight;
    fn register_node() -> Weight;
    fn update_node() -> Weight;
    fn unregister_node() -> Weight;
    fn declare_holding() -> Weight;
    fn revoke_holding() -> Weight;
}

// Default implementation (to be benchmarked)
impl WeightInfo for () {
    fn register_fragment() -> Weight { Weight::from_parts(10_000, 0) }
    fn register_node() -> Weight { Weight::from_parts(15_000, 0) }
    fn update_node() -> Weight { Weight::from_parts(10_000, 0) }
    fn unregister_node() -> Weight { Weight::from_parts(20_000, 0) }
    fn declare_holding() -> Weight { Weight::from_parts(15_000, 0) }
    fn revoke_holding() -> Weight { Weight::from_parts(15_000, 0) }
}
```

---

## 10. Test Coverage Requirements

| Test ID | Description | Extrinsic |
|---------|-------------|-----------|
| T-001 | Register fragment successfully | `register_fragment` |
| T-002 | Reject duplicate fragment ID | `register_fragment` |
| T-003 | Reject oversized fragment | `register_fragment` |
| T-003b | Reject zero-sized fragment | `register_fragment` |
| T-004 | Register node successfully | `register_node` |
| T-005 | Reject duplicate PeerID | `register_node` |
| T-005b | Reject if operator already has node | `register_node` |
| T-006 | Update node capacity | `update_node` |
| T-007 | Unregister node successfully | `unregister_node` |
| T-007b | Reject unregister with active holdings | `unregister_node` |
| T-008 | Declare holding successfully | `declare_holding` |
| T-008b | Idempotent declare holding | `declare_holding` |
| T-009 | Revoke holding successfully | `revoke_holding` |
| T-010 | Query fragment holders | RPC |
| T-011 | Query node holdings | RPC |

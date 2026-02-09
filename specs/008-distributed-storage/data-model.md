# Data Model: Storage MVP - Phase 1

**Generated**: 2026-02-09  
**Source**: [spec.md](spec.md) Key Entities, [research.md](research.md)

---

## 1. Overview

Phase 1のデータモデルは2層構造：
- **オンチェーン（Pallet Storage）**: メタデータのみ（断片索引、ノード登録）
- **オフチェーン（Daemon）**: 実データ（断片バイナリ、ローカル索引）

```
┌─────────────────────────────────────────────────────────────┐
│                      On-Chain (Substrate)                   │
├─────────────────────────────────────────────────────────────┤
│  FragmentMetadata    StorageNode     HoldingDeclaration    │
│  (断片カタログ)        (ノード一覧)      (保持表明)            │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ subxt (RPC)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Off-Chain (Daemon Disk)                  │
├─────────────────────────────────────────────────────────────┤
│  Fragment Files (実データ)    LocalIndex (キャッシュ)        │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. On-Chain Entities (Pallet Storage)

### 2.1 FragmentMetadata

断片のメタデータ。Fragment IDで一意に識別される。

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `fragment_id` | `[u8; 32]` | Primary Key, Blake2-256 hash | 断片の一意識別子（データのハッシュ） |
| `size` | `u32` | 1 ≤ x ≤ 1,048,576 (1MB) | 断片サイズ（バイト） |
| `creator` | `AccountId` | Must exist | 登録者のアカウント |
| `created_at` | `BlockNumber` | Auto-set | 登録時のブロック番号 |

**Storage Layout**:
```rust
#[pallet::storage]
pub type Fragments<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    FragmentId,           // [u8; 32]
    FragmentMetadata<T>,
    OptionQuery,
>;
```

**Validation Rules**:
- `fragment_id` must be unique (FR-002)
- `size` must be > 0 and ≤ MAX_FRAGMENT_SIZE (FR-003)

---

### 2.2 StorageNode

ストレージノードの登録情報。PeerIDで一意に識別される。

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `peer_id` | `BoundedVec<u8, 64>` | Primary Key, libp2p format | ノードのPeerID |
| `operator` | `AccountId` | Must exist | 運営者のアカウント |
| `capacity` | `u64` | > 0 | 提供容量（バイト） |
| `registered_at` | `BlockNumber` | Auto-set | 登録時のブロック番号 |

**Storage Layout**:
```rust
#[pallet::storage]
pub type StorageNodes<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BoundedVec<u8, ConstU32<64>>,  // PeerID
    StorageNodeInfo<T>,
    OptionQuery,
>;

// Reverse lookup: AccountId -> PeerID
#[pallet::storage]
pub type OperatorNodes<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<u8, ConstU32<64>>,  // PeerID
    OptionQuery,
>;
```

**Validation Rules**:
- `peer_id` must be valid libp2p PeerID format (FR-004)
- `peer_id` must be unique (FR-004)
- `operator` can only have one node registered (1:1 relationship)

---

### 2.3 HoldingDeclaration

特定のノードが特定の断片を保持していることの表明。

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `peer_id` | `BoundedVec<u8, 64>` | FK → StorageNode | 保持表明するノード |
| `fragment_id` | `[u8; 32]` | FK → FragmentMetadata | 保持する断片 |
| `declared_at` | `BlockNumber` | Auto-set | 表明時のブロック番号 |

**Storage Layout**:
```rust
// Fragment -> list of holders
#[pallet::storage]
pub type FragmentHolders<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    FragmentId,
    BoundedVec<BoundedVec<u8, ConstU32<64>>, ConstU32<100>>,  // max 100 holders
    ValueQuery,
>;

// Node -> list of held fragments (for efficient cleanup)
#[pallet::storage]
pub type NodeHoldings<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BoundedVec<u8, ConstU32<64>>,  // PeerID
    BoundedVec<FragmentId, ConstU32<10000>>,  // max 10k fragments per node
    ValueQuery,
>;
```

**Validation Rules**:
- `peer_id` must be registered (FR-006)
- `fragment_id` must exist (FR-006)
- Same (peer_id, fragment_id) pair must be idempotent (Edge Case: 重複保持表明)

---

## 3. Off-Chain Entities (Daemon Storage)

### 3.1 Fragment File

断片の実データ。ファイルシステムに保存。

| Field | Type | Description |
|-------|------|-------------|
| Path | `fragments/{hex[0:2]}/{hex[2:4]}/{hex}.bin` | 階層型パス |
| Content | `Vec<u8>` | 生データ（暗号化済み想定） |

**Directory Structure**:
```
$DATA_DIR/
├── fragments/
│   ├── 00/
│   │   ├── 00/
│   │   │   └── 00abc...def.bin
│   │   └── 01/
│   └── ff/
└── ...
```

**Validation Rules**:
- File size must match declared size (T-101)
- Blake2-256(content) must equal fragment_id (Research: Hash verification)

---

### 3.2 NodeIdentity

ノードのアイデンティティ（永続化）。

| Field | Type | Description |
|-------|------|-------------|
| `keypair.bin` | libp2p protobuf | Ed25519キーペア |
| `peer_id` | text file | Base58エンコードされたPeerID（参照用） |

**Directory Structure**:
```
$DATA_DIR/
├── identity/
│   ├── keypair.bin
│   └── peer_id
└── ...
```

---

### 3.3 LocalIndex (Optional)

高速な断片一覧取得用のローカル索引。sled KVS使用。

| Key | Value | Description |
|-----|-------|-------------|
| `fragment:{fragment_id}` | `LocalFragmentMeta` | 断片のローカルメタデータ |

```rust
#[derive(Serialize, Deserialize)]
pub struct LocalFragmentMeta {
    pub size: u64,
    pub stored_at: u64,      // Unix timestamp
    pub last_accessed: u64,  // Unix timestamp
    pub on_chain: bool,      // チェーンに保持表明済みか
}
```

---

## 4. Entity Relationships

```
┌───────────────────┐       1:N       ┌────────────────────┐
│  FragmentMetadata │◄────────────────│  HoldingDeclaration│
│  (fragment_id PK) │                 │  (fragment_id FK)  │
└───────────────────┘                 └────────────────────┘
                                              │
                                              │ N:1
                                              ▼
                                      ┌───────────────────┐
                                      │    StorageNode    │
                                      │    (peer_id PK)   │
                                      └───────────────────┘
                                              │
                                              │ 1:1
                                              ▼
                                      ┌───────────────────┐
                                      │   OperatorNodes   │
                                      │ (operator FK)     │
                                      └───────────────────┘
```

**Cardinality**:
- FragmentMetadata : HoldingDeclaration = 1 : N (一つの断片を複数ノードが保持可能)
- StorageNode : HoldingDeclaration = 1 : N (一つのノードが複数断片を保持可能)
- AccountId : StorageNode = 1 : 1 (一つのアカウントで一つのノードのみ)

---

## 5. State Transitions

### 5.1 Fragment Lifecycle

```
┌──────────┐ register_fragment() ┌────────────┐
│ (none)   │────────────────────►│ Registered │
└──────────┘                     └────────────┘
                                       │
                    declare_holding()  │  (can be called multiple times
                                       │   by different nodes)
                                       ▼
                                ┌────────────┐
                                │   Held     │
                                │ (by N nodes)│
                                └────────────┘
                                       │
                    revoke_holding()   │  (each node can revoke)
                                       ▼
                                ┌────────────┐
                                │ Registered │  (if all holders revoke)
                                └────────────┘
```

### 5.2 StorageNode Lifecycle

```
┌──────────┐ register_node()  ┌────────────┐
│ (none)   │─────────────────►│ Registered │
└──────────┘                  └────────────┘
                                    │
                  update_node()  ◄──┤
                                    │
               unregister_node()    │  (must revoke all holdings first)
                                    ▼
                              ┌──────────┐
                              │ (none)   │
                              └──────────┘
```

---

## 6. Rust Type Definitions

```rust
// === Pallet Types ===

/// Fragment ID (32 bytes, Blake2-256 hash)
pub type FragmentId = [u8; 32];

/// Fragment metadata stored on-chain
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, Debug, PartialEq)]
#[scale_info(skip_type_params(T))]
pub struct FragmentMetadata<T: Config> {
    pub size: u32,
    pub creator: T::AccountId,
    pub created_at: BlockNumberFor<T>,
}

/// Storage node information
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, Debug, PartialEq)]
#[scale_info(skip_type_params(T))]
pub struct StorageNodeInfo<T: Config> {
    pub operator: T::AccountId,
    pub capacity: u64,
    pub registered_at: BlockNumberFor<T>,
}

// === Daemon Types ===

/// Local fragment metadata (off-chain)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalFragmentMeta {
    pub size: u64,
    pub stored_at: u64,
    pub last_accessed: u64,
    pub on_chain_declared: bool,
}

/// Node identity
#[derive(Debug)]
pub struct NodeIdentity {
    pub keypair: libp2p::identity::Keypair,
    pub peer_id: libp2p::PeerId,
}
```

---

## 7. Constants & Limits

| Constant | Value | Rationale |
|----------|-------|-----------|
| `MAX_FRAGMENT_SIZE` | 1,048,576 (1MB) | メモリ効率とネットワーク帯域のバランス |
| `MAX_PEER_ID_LEN` | 64 bytes | libp2p Ed25519 PeerIDの最大長 |
| `MAX_HOLDERS_PER_FRAGMENT` | 100 | 初期段階での現実的な上限 |
| `MAX_FRAGMENTS_PER_NODE` | 10,000 | ノードあたりの管理上限 |
| `MIN_CAPACITY` | 1,073,741,824 (1GB) | 最小提供容量 |

---

## 8. Migration Notes

Phase 2への移行時に追加予定のフィールド：

| Entity | New Field | Type | Purpose |
|--------|-----------|------|---------|
| FragmentMetadata | `reward_pool` | u128 | 報酬プール残高 |
| StorageNodeInfo | `stake` | u128 | デポジット額 |
| HoldingDeclaration | `last_verified` | BlockNumber | 最終検証時刻 |
| HoldingDeclaration | `challenge_count` | u32 | チャレンジ回数 |

これらは別マイグレーションで追加し、Phase 1の構造を変更しない。

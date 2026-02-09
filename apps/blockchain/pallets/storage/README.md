# pallet-storage

Storage pallet for the Anarchy distributed storage network.

## Overview

This pallet manages fragment metadata and storage node registration on-chain. It provides:

- **Fragment Registration**: Register content fragments with their metadata (ID, size, owner)
- **Node Registration**: Storage nodes register their PeerID and available capacity
- **Holding Declaration**: Nodes declare which fragments they are storing

## Extrinsics

| Extrinsic | Description |
|-----------|-------------|
| `register_fragment` | Register a new fragment with ID and size |
| `register_node` | Register a storage node with PeerID and capacity |
| `update_node` | Update node's available capacity |
| `unregister_node` | Remove node registration (requires no active holdings) |
| `declare_holding` | Declare that this node is holding a fragment |
| `revoke_holding` | Revoke a holding declaration |

## Storage Items

- `Fragments`: Map of FragmentId → FragmentMetadata
- `StorageNodes`: Map of PeerId → StorageNodeInfo  
- `OperatorNodes`: Map of AccountId → PeerId (reverse lookup)
- `FragmentHolders`: Map of FragmentId → Vec<PeerId> (who holds each fragment)
- `NodeHoldings`: Map of PeerId → Vec<FragmentId> (what each node holds)

## Configuration

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    
    /// Maximum fragment size in bytes (default: 1MB)
    #[pallet::constant]
    type MaxFragmentSize: Get<u64>;
    
    /// Maximum PeerID length in bytes
    #[pallet::constant]
    type MaxPeerIdLen: Get<u32>;
    
    /// Maximum holders per fragment
    #[pallet::constant]
    type MaxHoldersPerFragment: Get<u32>;
    
    /// Maximum fragments per node
    #[pallet::constant]
    type MaxFragmentsPerNode: Get<u32>;
}
```

## Usage

### Register a Fragment

```rust
// Register a 1KB fragment
Storage::register_fragment(
    origin,
    fragment_id,  // [u8; 32] - Blake2-256 hash of content
    1024,         // size in bytes
)?;
```

### Register a Storage Node

```rust
// Register as a storage node with 10GB capacity
Storage::register_node(
    origin,
    peer_id.to_vec().try_into()?,
    10 * 1024 * 1024 * 1024,  // 10GB
)?;
```

### Declare Holding

```rust
// Declare that this node is storing a fragment
Storage::declare_holding(
    origin,
    fragment_id,
)?;
```

## Testing

```bash
cargo test -p pallet-storage
```

## License

MIT

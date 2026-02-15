# anarchy-storage-node

Distributed storage node daemon for the Anarchy network.

## Overview

This daemon participates in the Anarchy distributed storage network by:

- Storing content fragments locally
- Serving fragments to requesting peers via libp2p
- Declaring holdings to the blockchain

## Features

- **libp2p P2P Networking**: Request-response protocol for fragment exchange
- **Blake2-256 Hash Verification**: Ensures fragment integrity
- **Capacity Quota Management**: Configurable storage limits
- **Rate-Limited Chain Integration**: Prevents wallet drain attacks (FR-108)
- **Graceful Shutdown**: Clean shutdown on SIGINT/SIGTERM

## Installation

```bash
cd apps/storage-node
cargo build --release
```

## Configuration

Copy the example configuration:

```bash
cp config.example.toml config.toml
```

Edit `config.toml`:

```toml
# Directory for storing node data (identity, fragments)
data_dir = "./data"

# Maximum storage capacity in bytes (default: 10GB)
capacity = 10737418240

# Chain RPC endpoint URL
chain_url = "ws://127.0.0.1:9944"

# libp2p listen address (multiaddr format)
listen_addr = "/ip4/0.0.0.0/tcp/4001"

# Rate limit for declare_holding calls per minute
declare_rate_limit = 10
```

## Usage

### Start the Node

```bash
./target/release/anarchy-storage-node --config config.toml
```

### CLI Options

```
anarchy-storage-node [OPTIONS]

Options:
  -c, --config <CONFIG>      Path to configuration file [default: config.toml]
  -d, --data-dir <DATA_DIR>  Data directory (overrides config)
      --chain-url <URL>      Chain RPC URL (overrides config)
      --listen <ADDR>        Listen address (overrides config)
  -h, --help                 Print help
```

### Environment Variables

Set log level:

```bash
RUST_LOG=anarchy_storage_node=debug ./anarchy-storage-node
```

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    main.rs                           │
│  CLI parsing, config loading, event loop             │
└────────────────────┬────────────────────────────────┘
                     │
    ┌────────────────┼────────────────┬───────────────┐
    ▼                ▼                ▼               ▼
┌────────┐    ┌──────────┐    ┌──────────┐    ┌───────────┐
│identity│    │ storage  │    │ network  │    │   chain   │
│ .rs    │    │  /mod.rs │    │ /mod.rs  │    │  /mod.rs  │
├────────┤    ├──────────┤    ├──────────┤    ├───────────┤
│PeerID  │    │Fragment  │    │libp2p    │    │RateLimiter│
│Keypair │    │Store     │    │Swarm     │    │subxt stub │
└────────┘    └──────────┘    └──────────┘    └───────────┘
```

## Module Overview

| Module | Purpose |
|--------|---------|
| `identity` | PeerID/keypair management, persistence |
| `storage` | Fragment storage, hash verification, quota |
| `network` | libp2p swarm, request-response protocol |
| `chain` | Chain client, rate limiter for declare_holding |
| `config` | TOML configuration loading |
| `metrics` | Basic observability metrics |

## Protocol

Fragment requests use libp2p request-response:

```
Protocol: /anarchy/fragment/1.0.0

Request:
  - Get { fragment_id: [u8; 32] }
  - Put { fragment_id: [u8; 32], data: Vec<u8> }

Response:
  - Data(Option<Vec<u8>>)
  - Ack { success: bool, error: Option<String> }
```

## Security

- **FR-107**: Fragments are only stored if registered on-chain (prevents spam)
- **FR-108**: Rate limiting on `declare_holding` (max 10/min default)
- **Hash Verification**: All stored fragments verified against their ID

## Multi-Node Storage (010-multi-node-storage)

### Overview

Multiple storage nodes can be deployed for increased availability and redundancy.
The blockchain node automatically distributes fragments across registered nodes.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Blockchain Node                              │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │              StorageNodeRegistry                            │ │
│  │  - register(): Add new storage node                         │ │
│  │  - select_node_for_fragment(index): Distribute fragments    │ │
│  │  - online_nodes(): Filter healthy nodes                     │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│        ┌──────────────────┼──────────────────┐                  │
│        ▼                  ▼                  ▼                  │
│  ┌──────────┐      ┌──────────┐      ┌──────────┐              │
│  │ Node 1   │      │ Node 2   │      │ Node 3   │              │
│  │ F0, F3..│      │ F1, F4..│      │ F2, F5..│              │
│  └──────────┘      └──────────┘      └──────────┘              │
└─────────────────────────────────────────────────────────────────┘
```

### Fragment Distribution

Fragments are distributed using index-based routing:
- Fragment 0 → Node `0 % node_count`
- Fragment 1 → Node `1 % node_count`
- Fragment N → Node `N % node_count`

This ensures even distribution across all online nodes.

### Deploying Multiple Nodes

1. **Start multiple storage nodes** with unique ports:

```bash
# Node 1
./anarchy-storage-node --config node1-config.toml  # rpc_port = 3030

# Node 2
./anarchy-storage-node --config node2-config.toml  # rpc_port = 3031

# Node 3
./anarchy-storage-node --config node3-config.toml  # rpc_port = 3032
```

2. **Auto-registration**: Each node registers itself with the blockchain on startup via `storage_registerEndpoint` RPC.

3. **Verify registration**: Use `storage_getNodes` RPC to list registered nodes:

```bash
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"storage_getNodes","params":[]}' \
  http://127.0.0.1:9944
```

### Failover Behavior

- **Fragment retrieval**: Falls back to other nodes if primary is offline
- **Offline detection**: Nodes marked offline after connection failures
- **k-of-n redundancy**: Content recoverable if ≥k nodes are online (default: k=3, n=5)

### RPC Endpoints

| Method | Description |
|--------|-------------|
| `storage_registerEndpoint` | Register a storage node |
| `storage_getNodes` | List all registered nodes |
| `storage_uploadFragment` | Upload fragment (routed to appropriate node) |
| `storage_getFragment` | Retrieve fragment (with fallback) |

## Testing

```bash
# Unit tests
cargo test -p anarchy-storage-node

# Integration tests
cargo test --test integration
```

## License

MIT

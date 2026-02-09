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

## Testing

```bash
# Unit tests
cargo test -p anarchy-storage-node

# Integration tests
cargo test --test integration
```

## License

MIT

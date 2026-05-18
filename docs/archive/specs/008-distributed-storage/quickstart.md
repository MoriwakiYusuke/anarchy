# Quickstart: Storage MVP - Phase 1

**Generated**: 2026-02-09  
**Prerequisites**: Rust 1.75+, cargo, running Anarchy node

---

## 1. Development Setup

### 1.1 Clone & Build

```bash
# リポジトリは既にクローン済みの場合
cd apps/blockchain

# ストレージパレットのディレクトリを作成
mkdir -p pallets/storage/src
```

### 1.2 Create Pallet Cargo.toml

```bash
cat > pallets/storage/Cargo.toml << 'EOF'
[package]
name = "pallet-storage"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Distributed storage pallet for Anarchy network"

[dependencies]
codec = { package = "parity-scale-codec", version = "3.6", default-features = false, features = ["derive"] }
scale-info = { version = "2.10", default-features = false, features = ["derive"] }

frame-support = { version = "45.0.0", default-features = false }
frame-system = { version = "45.0.0", default-features = false }
sp-runtime = { version = "45.0.0", default-features = false }
sp-core = { version = "45.0.0", default-features = false }

[dev-dependencies]
sp-io = { version = "45.0.0" }

[features]
default = ["std"]
std = [
    "codec/std",
    "scale-info/std",
    "frame-support/std",
    "frame-system/std",
    "sp-runtime/std",
    "sp-core/std",
]
EOF
```

### 1.3 Add to Workspace

```bash
# apps/blockchain/Cargo.toml に追加
# [workspace].members に "pallets/storage" を追加
```

---

## 2. Minimal Pallet Implementation

### 2.1 Create lib.rs

```rust
// pallets/storage/src/lib.rs
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    pub type FragmentId = [u8; 32];

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        #[pallet::constant]
        type MaxFragmentSize: Get<u32>;

        #[pallet::constant]
        type MaxPeerIdLen: Get<u32>;
    }

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, Debug, PartialEq)]
    #[scale_info(skip_type_params(T))]
    pub struct FragmentMetadata<T: Config> {
        pub size: u32,
        pub creator: T::AccountId,
        pub created_at: BlockNumberFor<T>,
    }

    #[pallet::storage]
    #[pallet::getter(fn fragments)]
    pub type Fragments<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        FragmentId,
        FragmentMetadata<T>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        FragmentRegistered {
            fragment_id: FragmentId,
            creator: T::AccountId,
            size: u32,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        FragmentAlreadyExists,
        FragmentTooLarge,
        FragmentTooSmall,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn register_fragment(
            origin: OriginFor<T>,
            fragment_id: FragmentId,
            size: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(size > 0, Error::<T>::FragmentTooSmall);
            ensure!(size <= T::MaxFragmentSize::get(), Error::<T>::FragmentTooLarge);
            ensure!(!Fragments::<T>::contains_key(&fragment_id), Error::<T>::FragmentAlreadyExists);

            let metadata = FragmentMetadata {
                size,
                creator: who.clone(),
                created_at: frame_system::Pallet::<T>::block_number(),
            };

            Fragments::<T>::insert(fragment_id, metadata);

            Self::deposit_event(Event::FragmentRegistered {
                fragment_id,
                creator: who,
                size,
            });

            Ok(())
        }
    }
}
```

### 2.2 Build & Test

```bash
# パレットのビルド
cargo build -p pallet-storage

# テストの実行（テストモジュール追加後）
cargo test -p pallet-storage
```

---

## 3. Storage Node Daemon Setup

### 3.1 Create Project

```bash
cd ~/self/anarchy/apps
mkdir -p storage-node/src
cd storage-node

cat > Cargo.toml << 'EOF'
[package]
name = "anarchy-storage-node"
version = "0.1.0"
edition = "2021"

[dependencies]
libp2p = { version = "0.54", features = [
    "tokio",
    "tcp",
    "noise",
    "yamux",
    "request-response",
    "macros",
    "ed25519",
] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
hex = "0.4"
blake2 = "0.10"
EOF
```

### 3.2 Minimal Main

```rust
// src/main.rs
use libp2p::{identity, PeerId};
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Generate identity
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());

    info!("Storage node starting with PeerID: {}", peer_id);

    // TODO: Build swarm, event loop, etc.
}
```

### 3.3 Build

```bash
cargo build -p anarchy-storage-node
```

---

## 4. Integration Test

### 4.1 Start Local Node

```bash
# ターミナル1: ノード起動
cd apps/blockchain
cargo run --release -- --dev
```

### 4.2 Register Fragment (Manual Test)

```bash
# ターミナル2: polkadot.js appsで接続
# https://polkadot.js.org/apps/?rpc=ws://127.0.0.1:9944

# Developer > Extrinsics > storage > registerFragment
# fragment_id: 0x0000...0001 (32 bytes)
# size: 1024
# Submit Transaction
```

### 4.3 Verify Storage

```bash
# Developer > Chain State > storage > fragments
# 入力: 0x0000...0001
# 結果: FragmentMetadata { size: 1024, creator: ..., created_at: ... }
```

---

## 5. Next Steps

1. **Pallet完成**: 残りのextrinsic（`register_node`, `declare_holding`等）を実装
2. **Daemon完成**: libp2p swarmとrequest-responseを実装
3. **Runtime統合**: `runtime/src/lib.rs`にパレットを追加
4. **E2Eテスト**: ノード間断片転送のテストスクリプト作成

---

## 6. Useful Commands

```bash
# パレットテスト
cargo test -p pallet-storage

# ノードビルド（リリース）
cargo build --release -p anarchy-node

# ノード起動（開発モード）
./target/release/anarchy-node --dev

# チェーンデータクリア
./target/release/anarchy-node purge-chain --dev -y

# ログレベル設定
RUST_LOG=info ./target/release/anarchy-node --dev
```

---

## 7. References

- [spec.md](spec.md) - Feature specification
- [data-model.md](data-model.md) - Data model details
- [research.md](research.md) - Technical research
- [contracts/storage-pallet.md](contracts/storage-pallet.md) - Pallet API contract
- [Substrate Docs](https://docs.substrate.io/) - Official documentation
- [libp2p Rust](https://docs.rs/libp2p/) - P2P networking library

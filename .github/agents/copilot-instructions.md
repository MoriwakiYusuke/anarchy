# anarchy Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-07

## Active Technologies
- Substrate on-chain storage（既存Identity Palletを拡張） (002-webauthn-verification)
- TypeScript 5.3, React 18.2, Node.js 20+ + Next.js 14.1 (App Router), React 18, polkadot-api (005-frontend-ui-redesign)
- localStorage（言語設定の永続化） (005-frontend-ui-redesign)
- Rust 1.83+ (stable2503), Bash (セットアップスクリプト) + sc-network (Substrate libp2p実装), Tor 0.4.x (外部デーモン) (006-libp2p-tor)
- N/A（ネットワーク層のみ） (006-libp2p-tor)
- Rust 1.82 (Polkadot SDK stable2503), TypeScript 5.x (Next.js 15) + frame-support, frame-system, pallet-balances, PAPI, blakejs (007-pow-faucet)
- Substrate on-chain storage (RocksDB) (007-pow-faucet)
- Substrate Storage (StorageMap, StorageValue) (007-pow-faucet)
- Rust 1.75+ (Polkadot SDK stable2503), TypeScript 5.x (Frontend) (008-distributed-storage)
- Rust 1.87 (stable2503), TypeScript 5.x (Next.js 15) + Polkadot SDK (stable2503), PAPI (polkadot-api), libp2p 0.54, wasm-bindgen, subxt (009-post-storage-migration)
- オンチェーン（MerkleRootのみ）、オフチェーン（Storage Node分散保存） (009-post-storage-migration)
- Rust 1.75+ (stable2503 toolchain), TypeScript 5.x (010-multi-node-storage)
- Rust (Polkadot SDK stable2503), TypeScript (Next.js 14) + arkworks (ark-bls12-381, ark-poly, ark-poly-commit), wasm-pack, PAPI (011-kzg-proof-rewards)
- Substrate on-chain storage (`Fragments`, `RewardPoolBalance`) (011-kzg-proof-rewards)

- Rust 1.75+ (Polkadot SDK stable2503) + frame-support, frame-system, sp-runtime, sp-core (001-identity-pallet)

## Project Structure

```text
src/
tests/
```

## Commands

cargo test [ONLY COMMANDS FOR ACTIVE TECHNOLOGIES][ONLY COMMANDS FOR ACTIVE TECHNOLOGIES] cargo clippy

## Code Style

Rust 1.75+ (Polkadot SDK stable2503): Follow standard conventions

## Recent Changes
- 011-kzg-proof-rewards: Added Rust (Polkadot SDK stable2503), TypeScript (Next.js 14) + arkworks (ark-bls12-381, ark-poly, ark-poly-commit), wasm-pack, PAPI
- 010-multi-node-storage: Added Rust 1.75+ (stable2503 toolchain), TypeScript 5.x
- 009-post-storage-migration: Added Rust 1.87 (stable2503), TypeScript 5.x (Next.js 15) + Polkadot SDK (stable2503), PAPI (polkadot-api), libp2p 0.54, wasm-bindgen, subxt


<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->

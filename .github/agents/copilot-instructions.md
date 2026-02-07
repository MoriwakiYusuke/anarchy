# anarchy Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-07

## Active Technologies
- Substrate on-chain storage（既存Identity Palletを拡張） (002-webauthn-verification)
- TypeScript 5.3.3 + Next.js 14.1.0, React 18.2.0, polkadot-api 1.23.3, cbor-x 1.5.x (003-frontend-webauthn)
- LocalStorage (クレデンシャルID永続化のみ) (003-frontend-webauthn)

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
- 003-frontend-webauthn: Added TypeScript 5.3.3 + Next.js 14.1.0, React 18.2.0, polkadot-api 1.23.3, cbor-x 1.5.x
- 003-frontend-webauthn: Added [if applicable, e.g., PostgreSQL, CoreData, files or N/A]
- 002-webauthn-verification: Added Rust 1.75+ (Polkadot SDK stable2503)


<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->

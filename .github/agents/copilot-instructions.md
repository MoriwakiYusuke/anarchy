# anarchy Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-07

## Active Technologies
- Substrate on-chain storage（既存Identity Palletを拡張） (002-webauthn-verification)
- TypeScript 5.3, React 18.2, Node.js 20+ + Next.js 14.1 (App Router), React 18, polkadot-api (005-frontend-ui-redesign)
- localStorage（言語設定の永続化） (005-frontend-ui-redesign)
- Rust 1.83+ (stable2503), Bash (セットアップスクリプト) + sc-network (Substrate libp2p実装), Tor 0.4.x (外部デーモン) (006-libp2p-tor)
- N/A（ネットワーク層のみ） (006-libp2p-tor)

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
- 006-libp2p-tor: Added Rust 1.83+ (stable2503), Bash (セットアップスクリプト) + sc-network (Substrate libp2p実装), Tor 0.4.x (外部デーモン)
- 005-frontend-ui-redesign: Added TypeScript 5.3, React 18.2, Node.js 20+ + Next.js 14.1 (App Router), React 18, polkadot-api
- 002-webauthn-verification: Added Rust 1.75+ (Polkadot SDK stable2503)


<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->

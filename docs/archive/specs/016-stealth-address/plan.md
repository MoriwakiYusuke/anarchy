# Implementation Plan: ステルスアドレス統合

**Branch**: `016-stealth-address` | **Date**: 2026-02-27 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/016-stealth-address/spec.md`

## Summary

ステルスアドレス統合機能の実装計画。EIP-5564互換プロトコルを採用し、X25519鍵交換とBlake2bハッシュを使用したワンタイムアドレス導出を実現する。クライアントサイドでの暗号処理（Wasm + Web Worker）、軽量なStealth Pallet（エフェメラル公開鍵格納）、バックグラウンドスキャナーの3層で構成。

## Technical Context

**Language/Version**: Rust 1.87 (blockchain/wasm-engine), TypeScript 5.x (frontend)  
**Primary Dependencies**: x25519-dalek (Wasm暗号), polkadot-api/PAPI (チェーン通信), wasm-bindgen (Wasm binding)  
**Storage**: On-chain (Substrate Storage: EphemeralKeys), Off-chain (セッションメモリのみ、永続化なし)  
**Testing**: cargo test (pallet, wasm-engine), Jest (frontend)  
**Target Platform**: Browser (Wasm), Substrate Runtime  
**Project Type**: Monorepo (apps/blockchain, apps/frontend, packages/wasm-engine)  
**Performance Goals**: スキャン処理 1000ブロック/秒 (Web Worker内)  
**Constraints**: 秘密鍵はセッション中のみメモリ保持、ブラウザストレージへの永続化禁止  
**Scale/Scope**: 初期は全ブロックフルスキャン、将来インデクサー対応予定

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Network Anonymity | ✅ PASS | ステルスアドレス自体がオンチェーンでの匿名性を提供。Tor/I2P統合は別フィーチャー（006-libp2p-tor）で対応済み |
| II. Keyless UX | ⚠️ PARTIAL | ステルスアドレスはWebAuthnとは別の鍵体系（X25519）を使用。ただし秘密鍵はセッション中のみメモリ保持でハードウェア外に出ない。バックアップファイルはパスワード暗号化。「シードフレーズを扱わせない」には準拠。 |
| III. Client-Side Completion | ✅ PASS | すべての暗号処理（鍵生成、ワンタイムアドレス導出、スキャン判定）はWeb Worker + Wasmでクライアント側実行 |
| IV. Zero-Trust Hydra | ✅ PASS | 秘密鍵はブラウザ外に送信されない。フロントエンドが悪意を持っていても、プロトコル層で保護 |
| V. Economic Autonomy | ✅ N/A | 本機能は経済メカニズムに直接関与しない |
| VI. Test-First Development | ✅ REQUIRED | pallet-stealth単体テスト、Wasm暗号テスト、フロントエンドテストを先行実装 |

**Gate Result**: PASS（Principle IIは部分準拠だが、秘密鍵の取り扱いはConstitution趣旨に沿っている）

## Project Structure

### Documentation (this feature)

```text
specs/016-stealth-address/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── stealth-pallet-api.md
└── tasks.md             # Phase 2 output
```

### Source Code (repository root)

```text
apps/blockchain/
├── pallets/
│   └── stealth/         # NEW: Stealth Pallet
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── types.rs
│           └── tests.rs
└── runtime/
    └── src/lib.rs       # MODIFY: Add pallet-stealth to runtime

packages/wasm-engine/
└── src/
    ├── lib.rs           # MODIFY: Add stealth module exports
    └── stealth/         # NEW: Stealth address crypto
        ├── mod.rs
        ├── keys.rs      # X25519 key generation
        ├── address.rs   # One-time address derivation
        └── scan.rs      # Scan detection logic

apps/frontend/
└── src/
    ├── lib/
    │   └── stealth/     # NEW: Stealth service layer
    │       ├── worker.ts        # Web Worker entry
    │       ├── scanner.ts       # Background scanner
    │       ├── keyManager.ts    # Session key management
    │       └── types.ts
    ├── components/
    │   └── stealth/     # NEW: UI components
    │       ├── StealthAddressGenerator.tsx
    │       ├── StealthSendForm.tsx
    │       ├── StealthBalanceList.tsx
    │       └── BackupImportDialog.tsx
    └── app/
        └── stealth/     # NEW: Stealth pages
            └── page.tsx
```

**Structure Decision**: 既存のモノレポ構造に沿って、blockchain/pallets/stealth、packages/wasm-engine/src/stealth、apps/frontend/src/lib/stealthを追加。

## Complexity Tracking

> Constitution Check passed with no violations requiring justification.
> Principle II (Keyless UX) is partial but compliant - stealth keys use different scheme from WebAuthn but follow same security principles (session-only, no persistence).

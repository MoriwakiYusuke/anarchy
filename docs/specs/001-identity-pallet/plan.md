# Implementation Plan: Identity Pallet

**Branch**: `001-identity-pallet` | **Date**: 2026-02-07 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-identity-pallet/spec.md`

## Summary

WebAuthn公開鍵をオンチェーンで管理し、「秘密鍵をユーザーに扱わせない」を実現するSubstrateパレット。Constitution原則 **II. Keyless UX**【NON-NEGOTIABLE】の中核実装。1 Identity ID → N Passkeys のマルチデバイス対応を提供する。

## Technical Context

**Language/Version**: Rust 1.75+ (Polkadot SDK stable2503)  
**Primary Dependencies**: frame-support, frame-system, sp-runtime, sp-core  
**Storage**: Substrate on-chain storage (RocksDB)  
**Testing**: cargo test (frame_support::assert_ok!, assert_noop!)  
**Target Platform**: Substrate Runtime (WASM)  
**Project Type**: Substrate Pallet (モノレポ内 `apps/blockchain/pallets/identity/`)  
**Performance Goals**: N/A (ブロック生成時間内に処理完了)  
**Constraints**: 最大10 Passkeys/Identity、公開鍵サイズ上限 256 bytes  
**Scale/Scope**: 無制限 Identity数、オンチェーン永続化

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原則 | 状態 | 検証 |
|------|------|------|
| I. Network Anonymity | N/A | パレット自体はネットワーク層に関与しない |
| **II. Keyless UX** | ✅ PASS | 本パレットがこの原則を実装する |
| III. Client-Side Completion | ✅ PASS | 署名はクライアント側で実行、パレットは公開鍵のみ保存 |
| IV. Zero-Trust Hydra | ✅ PASS | 公開鍵の形式検証（将来: WebAuthn署名検証）で悪意あるフロント対策 |
| V. Economic Autonomy | N/A | 報酬システムには関与しない |
| VI. Test-First Development | ✅ PASS | Acceptance Scenariosに基づくテストを先に作成 |

**Gate Result**: ✅ PASSED - No violations

## Project Structure

### Documentation (this feature)

```text
specs/001-identity-pallet/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── identity-pallet.md
└── tasks.md             # Phase 2 output (created by /speckit.tasks)
```

### Source Code (repository root)

```text
apps/blockchain/
├── pallets/
│   ├── identity/              # NEW: Identity Pallet
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs         # Pallet main
│   │       ├── tests.rs       # Unit tests
│   │       └── benchmarking.rs # (optional)
│   ├── moral/                 # Existing
│   └── post/                  # Existing
├── runtime/
│   └── src/
│       └── lib.rs             # Runtime integration
└── node/
    └── src/
        └── chain_spec.rs      # Genesis config (if needed)
```

**Structure Decision**: 既存の `moral/` および `post/` パレットと同じ構造に従う。新規パレット `identity/` を `apps/blockchain/pallets/` 配下に作成。

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (なし) | - | - |

---

## Phase 0: Outline & Research

以下の調査を `research.md` にまとめる:

1. **WebAuthn公開鍵フォーマット**: COSEキー構造、ES256/P-256のエンコーディング
2. **Substateでの可変長データ保存**: BoundedVec vs Vec、ストレージコスト最適化
3. **PasskeyIdの導出方法**: 公開鍵からの一意ID生成（Blake2b-256ハッシュ）
4. **既存パレット設計パターン**: moral/post パレットの構造を踏襲

## Phase 1: Design & Contracts

以下を作成:

1. **data-model.md**: Identity, Passkey エンティティのオンチェーン構造
2. **contracts/identity-pallet.md**: Extrinsic API仕様
3. **quickstart.md**: 開発者向けクイックスタートガイド

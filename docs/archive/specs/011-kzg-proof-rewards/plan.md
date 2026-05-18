# Implementation Plan: KZG-VSS 保持証明・報酬システム

**Branch**: `011-kzg-proof-rewards` | **Date**: 2026-02-16 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/011-kzg-proof-rewards/spec.md`

## Summary

現在のSSS実装（sharks crate, GF(256)）をKZGベースの検証可能秘密分散（VSS）に置換。BLS12-381曲線上で同一多項式がSSS（秘密分散）とKZG（コミットメント）の両役割を果たす。保持証明成功時に$moralを報酬として分配し、スコア閾値未満のデータは報酬0で「経済的忘却」される。

**技術アプローチ**: arkworksスタック（ark-bls12-381, ark-poly-commit）でWasm/no_std両対応のKZG-VSSを実装。Ethereum KZG Ceremony (Powers of Tau)のTrusted Setupを再利用。

## Technical Context

**Language/Version**: Rust (Polkadot SDK stable2503), TypeScript (Next.js 14)  
**Primary Dependencies**: arkworks (ark-bls12-381, ark-poly, ark-poly-commit), wasm-pack, PAPI  
**Storage**: Substrate on-chain storage (`Fragments`, `RewardPoolBalance`)  
**Testing**: `cargo test` (pallets, wasm-engine), Jest (frontend)  
**Target Platform**: Wasm (wasm32v1-none for runtime, wasm32-unknown-unknown for browser), no_std (Substrate runtime), Linux (storage-node)  
**Project Type**: Monorepo (apps/blockchain, apps/frontend, packages/wasm-engine, apps/storage-node)  
**Performance Goals**: 1MB KZG-VSS split <5s (browser), KZG verify <10ms (on-chain), 100-node batch <1s  
**Constraints**: BLS12-381 pairing in Wasm, SRS embedding size (~300KB for degree-1024), 3-of-5 threshold  
**Scale/Scope**: Multi-node storage network, 100+ nodes, 32KB max per polynomial segment

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原則 | ステータス | 根拠 |
|------|----------|------|
| **I. Network Anonymity** [NON-NEGOTIABLE] | N/A | 本機能はネットワーク層に影響しない。Tor/I2P統合は既存のまま |
| **II. Keyless UX** [NON-NEGOTIABLE] | N/A | 既存のWebAuthn/AA認証を使用。秘密鍵操作なし |
| **III. Client-Side Completion** [NON-NEGOTIABLE] | ✅ PASS | KZG-VSSシェア生成・圧縮・暗号化は**全てクライアントサイドで実行** (FR-301, FR-306) |
| IV. Zero-Trust Hydra | ✅ PASS | フロントエンドが悪意を持っていても、KZGコミットメントはオンチェーンで検証。改ざん不可 |
| V. Economic Autonomy | ✅ PASS | スコア閾値による報酬制御。需要のないデータは報酬停止→経済的忘却 (FR-107, FR-110) |
| VI. Test-First Development | ✅ PASS | spec.mdにテスト要件 (T-001〜T-206) が先に定義済み |

**ゲート評価**: 全てのNON-NEGOTIABLE原則をパス。Phase 0に進行可能。

## Project Structure

### Documentation (this feature)

```text
specs/011-kzg-proof-rewards/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── storage-pallet.md
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
packages/wasm-engine/
├── src/
│   ├── lib.rs              # Existing entry point
│   └── kzg/                # NEW: KZG-VSS module (replaces sss_core.rs)
│       ├── mod.rs
│       ├── vss.rs          # vss_split, vss_recover
│       ├── proof.rs        # vss_prove, verify
│       └── srs.rs          # Trusted Setup (SRS) loading
├── srs/
│   └── mainnet.bin         # Ethereum KZG Ceremony SRS (embedded)
└── tests/
    └── kzg_tests.rs

apps/blockchain/pallets/storage/
├── src/
│   ├── lib.rs              # Existing pallet
│   ├── kzg.rs              # NEW: KZG verification logic
│   ├── rewards.rs          # NEW: Reward pool & distribution
│   └── challenge.rs        # NEW: Challenge generation
└── tests/
    └── kzg_tests.rs

apps/storage-node/
├── src/
│   ├── challenge.rs        # NEW: Challenge monitoring
│   ├── prover.rs           # NEW: KZG proof generation
│   └── gc.rs               # UPDATE: Score-based GC
└── tests/
    └── proof_tests.rs

apps/frontend/src/
├── services/
│   ├── kzg-vss.ts          # NEW: KZG-VSS wrapper for Wasm
│   └── compression.ts      # NEW: gzip compress/decompress
└── components/
    └── ScoreIndicator.tsx  # NEW: Score display component
```

**Structure Decision**: 既存のモノレポ構造を維持。packages/wasm-engineにKZGモジュールを追加し、sharksを置換。pallets/storageに報酬・チャレンジロジックを追加。

## Complexity Tracking

> Constitution Check has no violations. This section remains empty.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none) | — | — |

---

## Post-Design Constitution Re-evaluation

*Re-checked after Phase 1 design completion (2026-02-16)*

| 原則 | ステータス | 再評価根拠 |
|------|----------|----------|
| **I. Network Anonymity** [NON-NEGOTIABLE] | ✅ PASS | 設計はネットワーク層に影響なし。libp2p/Tor統合は既存のまま |
| **II. Keyless UX** [NON-NEGOTIABLE] | ✅ PASS | 既存WebAuthn/AA使用。鍵管理への変更なし |
| **III. Client-Side Completion** [NON-NEGOTIABLE] | ✅ PASS | `vss_split` はWasm Engineでクライアント実行。`compress` → `encrypt` → `split` の順序で実装 |
| IV. Zero-Trust Hydra | ✅ PASS | KZGコミットメントはオンチェーン検証。フロントエンドが改ざんしても無効なproofは拒否 |
| V. Economic Autonomy | ✅ PASS | スコアベース報酬、経済的忘却メカニズム。報酬プール設計で循環経済を実現 |
| VI. Test-First Development | ✅ PASS | T-001〜T-206のテスト要件が実装前に定義済み |

**結論**: Phase 1設計完了後も全てのConstitution原則を満たしている。Phase 2（tasks生成）に進行可能。

---

## Generated Artifacts

| ファイル | 説明 |
|---------|------|
| [plan.md](./plan.md) | 本実装計画 |
| [research.md](./research.md) | 技術選定・調査結果 |
| [data-model.md](./data-model.md) | エンティティ・状態遷移定義 |
| [contracts/storage-pallet.md](./contracts/storage-pallet.md) | Storage Pallet API契約 |
| [contracts/wasm-engine.md](./contracts/wasm-engine.md) | Wasm Engine API契約 |
| [quickstart.md](./quickstart.md) | 開発環境セットアップ手順 |

**次のステップ**: `/speckit.tasks` でタスク分解を実行

# Implementation Plan: マルチノード対応とストレージセキュリティ

**Branch**: `010-multi-node-storage` | **Date**: 2026-02-14 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/010-multi-node-storage/spec.md`

## Summary

現在の分散ストレージシステムを拡張し、SSS断片（n=5）を複数ストレージノードに分散配置する機能、署名ベースのアクセス認証、Storage Palletのセキュリティ強化（PoW/レート制限）、およびストレージノード間P2P通信（ブロックチェーンノード情報共有）を実装する。

## Technical Context

**Language/Version**: Rust 1.75+ (stable2503 toolchain), TypeScript 5.x  
**Primary Dependencies**: 
- Blockchain: Polkadot SDK stable2503, FRAME pallets
- Storage Node: libp2p 0.54+, axum 0.8+, prometheus 0.13+
- Frontend: Next.js 14, PAPI (polkadot-api)

**Storage**: 
- On-chain: Substrate Storage (ストレージマップ)
- Off-chain: ファイルシステム (apps/storage-node/data/)

**Testing**: 
- Pallet: `cargo test -p pallet-storage`, `cargo test -p pallet-post`
- Storage Node: `cargo test` (apps/storage-node/)
- Integration: Shell-based tests (apps/blockchain/tests/integration/)

**Target Platform**: Linux server (Tor/I2P対応), WebAssembly (wasm-engine)

**Project Type**: Monorepo (apps/blockchain, apps/storage-node, apps/frontend, packages/wasm-engine)

**Performance Goals**: 
- 断片アップロード: <500ms/断片
- フェイルオーバー: <6秒
- Gossipsub伝播: <30秒

**Constraints**: 
- Gossipsubメッセージ: ≤4KB
- 最低ノード容量: 1GB
- ノンスTTL: 5分

**Scale/Scope**: 100ノード以上のストレージノードをサポート

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Network Anonymity【NON-NEGOTIABLE】| ✅ PASS | FR-509でTorサーキット事前構築を明記。ストレージノード間P2P通信はlibp2p over Tor |
| II. Keyless UX【NON-NEGOTIABLE】| ✅ PASS | FR-202/Assumptionsでユーザー署名はSr25519 via Secure Enclave/WebAuthn。ノード間はEd25519（FR-512） |
| III. Client-Side Completion【NON-NEGOTIABLE】| ✅ PASS | SSS断片化・暗号化は既存wasm-engineでクライアント側実行（既存実装継続） |
| IV. Zero-Trust Hydra | ✅ PASS | 署名検証（FR-201〜207）で悪意あるリクエストを防止 |
| V. Economic Autonomy | ✅ PASS | PoW（FR-409）でスパム防止、Reputation（FR-513）で悪意あるノード抑制 |
| VI. Test-First Development | ✅ PASS | Pallet/Storage Node/Integration各レベルでテスト必須 |
| PAPI必須 | ✅ PASS | フロントエンド統合は既存PAPI継続使用（Assumptions明記） |

**Gate Result**: ✅ ALL PASS - Phase 0へ進行可能

### Post-Design Re-evaluation (Phase 1完了後)

| Principle | Status | Design Validation |
|-----------|--------|-------------------|
| I. Network Anonymity | ✅ PASS | research.md R-001: Gossipsubはlibp2p over Torで動作。contracts/gossipsub-messages.md: 4KB制限はTor MTUに適合 |
| II. Keyless UX | ✅ PASS | research.md R-004: Sr25519署名はWebAuthn経由で実行可能。contracts/storage-node-rpc.yaml: X-Anarchy-Authヘッダー仕様 |
| III. Client-Side Completion | ✅ PASS | data-model.md: SignedRequest構造はクライアント側で構築。既存wasm-engineでSSS断片化継続 |
| IV. Zero-Trust Hydra | ✅ PASS | contracts/storage-node-rpc.yaml: 401/403エラーで不正リクエスト拒否 |
| V. Economic Autonomy | ✅ PASS | data-model.md: PeerReputation/PoW動的難易度でスパム抑制 |
| VI. Test-First Development | ✅ PASS | quickstart.md: テストシナリオ4件定義、テストコマンド明記 |

**Post-Design Gate Result**: ✅ ALL PASS - Phase 2（tasks.md生成）へ進行可能

## Project Structure

### Documentation (this feature)

```text
specs/010-multi-node-storage/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── storage-node-rpc.yaml   # OpenAPI spec for storage node HTTP API
│   └── gossipsub-messages.md   # Gossipsub message format spec
└── tasks.md             # Phase 2 output (NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
# Existing monorepo structure with modifications

apps/blockchain/
├── pallets/
│   ├── storage/src/
│   │   ├── lib.rs          # MODIFY: Extrinsic security enhancements
│   │   ├── pow.rs          # NEW: PoW verification module
│   │   ├── rate_limit.rs   # NEW: Rate limiting storage
│   │   └── tests.rs        # MODIFY: Security tests
│   └── post/src/
│       └── lib.rs          # MODIFY: Tight coupling with storage pallet
└── runtime/src/
    └── lib.rs              # MODIFY: Pallet coupling configuration

apps/storage-node/src/
├── network/
│   ├── mod.rs              # MODIFY: Add Gossipsub for endpoint sharing
│   ├── gossip.rs           # NEW: Gossipsub protocol implementation
│   ├── endpoint_cache.rs   # NEW: Blockchain endpoint cache with TTL
│   └── reputation.rs       # NEW: Peer reputation tracking
├── rpc/
│   ├── mod.rs              # MODIFY: Add signature verification
│   └── auth.rs             # NEW: Request authentication middleware
├── chain/
│   └── failover.rs         # NEW: Active-Standby failover logic
└── metrics.rs              # MODIFY: Add new Prometheus metrics

apps/frontend/src/
├── hooks/
│   └── useStorage.ts       # MODIFY: Multi-node distribution support
├── components/
│   └── FragmentStatus.tsx  # NEW: Fragment placement visualization
└── stores/
    └── storageSettings.ts  # MODIFY: Node selection strategy settings
```

**Structure Decision**: 既存monorepo構造を維持し、各コンポーネント内に新モジュールを追加

## Complexity Tracking

> Constitution Check passed without violations. No justifications needed.

| Area | Complexity | Justification |
|------|------------|---------------|
| Storage Pallet PoW | Medium | 動的難易度計算は観測期間ストレージと簡単な算術で実装可能 |
| Post-Storage Tight Coupling | Low | Runtime内でのpallet間呼び出しは標準パターン |
| Gossipsub Integration | Medium | libp2pのGossipsub APIで標準的に実装可能 |
| Active-Standby Failover | Medium | ステートマシン + タイマーで実装 |

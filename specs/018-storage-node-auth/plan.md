# Implementation Plan: ストレージノードアクセス制限（セッショントークン認証）

**Branch**: `018-storage-node-auth` | **Date**: 2026-03-01 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/018-storage-node-auth/spec.md`

## Summary

ストレージノードへのアクセスをブロックチェーンノードからのみに制限する。初回署名＋セッショントークン方式を採用し、P2P接続済みのピア（ブロックチェーンノード）のみがセッショントークンを取得可能。フロントエンドからの直接アクセスは100%拒否される。

**主要な変更**:
1. ストレージノード: `storage_requestSession` RPC追加、セッションレジストリ実装
2. ストレージノード: HTTP repair/recoveryコード削除、libp2p P2Pに統一
3. ブロックチェーンノード: 起動時セッション確立、自動更新処理追加

## Technical Context

**Language/Version**: Rust 1.81+ (stable2503互換)
**Primary Dependencies**: libp2p 0.53+, axum 0.7+, ed25519-dalek, rand
**Storage**: In-memory HashMap<Token, SessionInfo>（永続化不要）
**Testing**: cargo test (unit), shell-based integration tests
**Target Platform**: Linux server (x86_64), Tor/I2P対応
**Project Type**: Multi-crate workspace (storage-node, blockchain node)
**Performance Goals**: セッション確立後のフラグメント転送レイテンシ50%以上改善
**Constraints**: 24時間トークン有効期限、1時間前自動更新、同時10台ブロックチェーンノード
**Scale/Scope**: 10台ブロックチェーンノード、複数ストレージノード

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Network Anonymity | ✅ PASS | libp2pトランスポート層のTor/I2P統合に影響なし |
| II. Keyless UX | ✅ PASS | ユーザー向け機能ではないため適用外（ノード間認証） |
| III. Client-Side Completion | ✅ PASS | フロントエンドはこの認証に関与しない |
| IV. Zero-Trust Hydra | ✅ PASS | フロントエンドからの直接アクセスを100%拒否 |
| V. Economic Autonomy | ✅ PASS | 既存の報酬システムに影響なし |
| VI. Test-First Development | ✅ PASS | ユニットテスト・統合テスト計画済み |

## Project Structure

### Documentation (this feature)

```text
specs/018-storage-node-auth/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
apps/storage-node/
├── src/
│   ├── rpc/
│   │   ├── server.rs        # storage_requestSession RPC追加
│   │   └── client.rs        # HTTP repair削除対象
│   ├── session/
│   │   ├── mod.rs           # 新規: セッションモジュール
│   │   ├── registry.rs      # 新規: SessionRegistry実装
│   │   └── token.rs         # 新規: SessionToken生成・検証
│   ├── auth.rs              # 既存: Ed25519署名検証ロジック流用
│   └── sync/
│       └── repair.rs        # HTTP経由リカバリロジック削除対象
└── tests/
    └── session_test.rs      # 新規: セッション認証テスト

apps/blockchain/node/
├── src/
│   └── storage/
│       └── session_client.rs  # 新規: セッション確立・自動更新
└── tests/
    └── storage_session_test.rs  # 新規: セッション統合テスト
```

**Structure Decision**: Multi-crate workspace。storage-nodeに認証機能追加、blockchain nodeにセッションクライアント追加。

## Complexity Tracking

> **All Constitution checks passed. No violations to justify.**

N/A

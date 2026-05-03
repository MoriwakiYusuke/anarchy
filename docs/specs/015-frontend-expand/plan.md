# Implementation Plan: フロントエンド拡充

**Branch**: `015-frontend-expand` | **Date**: 2026-02-25 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/015-frontend-expand/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command.

## Summary

Anarchyフロントエンドに4つの主要機能を追加：送金フォーム（PAPI経由）、メディア添付（既存分散ストレージ基盤活用）、投稿者名表示（AccountId短縮+コピー）、ニックネーム登録（新規Nickname Pallet）。UX重視で実装し、ブロックチェーン側の変更も許可。

## Technical Context

**Language/Version**: Rust (Polkadot SDK stable2503), TypeScript (ES2022), Next.js 14  
**Primary Dependencies**: PAPI (polkadot-api), React 18, wasm-engine (KZG-VSS/SSS), @polkadot/util  
**Storage**: Substrate blockchain state (pallets), Distributed storage nodes (libp2p + axum RPC)  
**Testing**: `cargo test` (pallets), Jest (frontend unit), shell-based integration tests  
**Target Platform**: Web browser (Chrome/Firefox/Safari), Substrate node (Linux)  
**Project Type**: Web monorepo (apps/frontend, apps/blockchain, apps/storage-node, packages/wasm-engine)  
**Performance Goals**: 送金60秒以内完了, 10MB画像アップロード30秒以内, 100MB動画アップロード5分以内  
**Constraints**: クライアントサイド暗号化必須, 1投稿最大4メディア, 画像100MB/動画1GB上限, i18n対応必須  
**Scale/Scope**: 単一ユーザー操作, 256KB/fragment (分割後), 4新コンポーネント追加

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### I. Network Anonymity（ネットワーク秘匿）
| 要件 | 状態 | 備考 |
|------|------|------|
| Tor/I2P経由の通信 | N/A | フロントエンド機能のため、IP露出は「許容」範囲（constitution準拠） |
| オンチェーンデータとIPの紐付け防止 | ✅ | オンチェーンにはAccountIdのみ保存、IPは紐付けなし |

### II. Keyless UX（秘密鍵の排除）
| 要件 | 状態 | 備考 |
|------|------|------|
| WebAuthn + AA | ⚠️ DEVIATION | 現状はシードフレーズ署名を使用（spec 003/004で対応予定） |
| Passkey対応 | ⚠️ DEVIATION | 上記同様、将来対応 |

**Deviation Justification**: WebAuthn/AA統合は別仕様（003-frontend-webauthn, 004-accountid-only-auth）で対応済み/対応中。本仕様は現行アーキテクチャで利用可能な署名方式を前提とする。

### III. Client-Side Completion（クライアントサイド完結）
| 要件 | 状態 | 備考 |
|------|------|------|
| 暗号化はクライアント側 | ✅ | メディアはwasm-engineでSSS分割してから送信 |
| メタデータ削除 | ✅ | 画像EXIF削除をクライアント側で実行 |

### IV. Zero-Trust Hydra
| 要件 | 状態 | 備考 |
|------|------|------|
| フロントエンドを信頼しない | ✅ | 全トランザクションはユーザー署名必須 |
| WYSIWYS | ✅ | 送金確認ダイアログで宛先・金額を明示 |

### V. Economic Autonomy
| 要件 | 状態 | 備考 |
|------|------|------|
| 経済的自律性 | ✅ | MORAL送金機能によりユーザー間価値移転を実現 |

### VI. Test-First Development
| 要件 | 状態 | 備考 |
|------|------|------|
| テストから始める | ✅ | Nickname Palletはcargo test先行、フロントエンドはJest先行 |

**GATE RESULT**: ✅ PASS（Deviation justified）

## Project Structure

### Documentation (this feature)

```text
specs/015-frontend-expand/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (API schemas)
└── tasks.md             # Phase 2 output (speckit.tasks)
```

### Source Code (repository root)

```text
# Monorepo structure for this feature

apps/frontend/
├── src/
│   ├── components/
│   │   ├── TransferForm/       # 送金フォーム（NEW）
│   │   ├── MediaUpload/        # メディアアップロード（NEW）
│   │   ├── PostAuthor/         # 投稿者表示（NEW）
│   │   └── NicknameSettings/   # ニックネーム設定（NEW）
│   ├── hooks/
│   │   ├── useTransfer.ts      # 送金ロジック（NEW）
│   │   ├── useMediaUpload.ts   # メディアアップロード（NEW）
│   │   └── useNickname.ts      # ニックネーム取得/設定（NEW）
│   ├── lib/
│   │   └── mediaProcessor.ts   # メディア処理（EXIF削除、分割）（NEW）
│   └── i18n/
│       ├── ja.json             # UPDATE: 新規テキスト追加
│       ├── en.json             # UPDATE: 新規テキスト追加
│       └── zh.json             # UPDATE: 新規テキスト追加
└── tests/
    ├── components/
    │   ├── TransferForm.test.tsx
    │   ├── MediaUpload.test.tsx
    │   └── PostAuthor.test.tsx
    └── hooks/
        ├── useTransfer.test.ts
        └── useMediaUpload.test.ts

apps/blockchain/
├── pallets/
│   └── nickname/               # 新規Nickname Pallet（NEW）
│       ├── Cargo.toml
│       └── src/lib.rs
└── runtime/src/lib.rs          # UPDATE: Nickname Pallet統合

packages/wasm-engine/
└── src/lib.rs                  # 変更なし（既存SSS/KZG対応で十分）

apps/storage-node/
└── src/                        # 変更なし（既存APIで対応可能）
```

**Structure Decision**: 既存モノレポ構造を維持。フロントエンドに4つの新コンポーネント、ブロックチェーンにNickname Palletを追加。ストレージノードは変更不要。

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Keyless UX deviation | 現行アーキテクチャでの開発継続 | WebAuthn統合は別仕様で並行開発中。本機能はシードフレーズ署名でも動作必須 |

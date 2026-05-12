# Implementation Plan: smoldot Light Client統合

**Branch**: `014-smoldot-integration` | **Date**: 2026-02-24 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/014-smoldot-integration/spec.md`

## Summary

フロントエンドアプリケーションにsmoldotライトクライアントを統合し、WebSocket RPC接続を完全に置き換える。polkadot-apiのsmoldotプロバイダーを使用し、チェーンスペックをビルド時に静的埋め込みする。後方互換性は不要であり、レガシーコードは完全に削除する。

## Technical Context

**Language/Version**: TypeScript 5.3.3, Rust (stable) for blockchain
**Primary Dependencies**: polkadot-api ^1.23.3, @polkadot-api/smoldot, smoldot, Next.js 14
**Storage**: N/A (ブラウザ内ライトクライアント)
**Testing**: Jest (フロントエンド)
**Target Platform**: Web Browser (Chrome, Firefox, Safari - Web Worker, WebAssembly必須。SharedArrayBuffer不要)
**Project Type**: Monorepo - apps/frontend (Next.js), apps/blockchain (Substrate)
**Performance Goals**: 初期化5秒以内、同期60秒以内
**Constraints**: 追加バンドルサイズ2MB以下、メインスレッドブロッキング禁止
**Scale/Scope**: 単一フロントエンドアプリ、既存機能の接続層のみ変更

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Analysis |
|-----------|--------|----------|
| I. Network Anonymity | ✅ PASS | smoldotはlibp2pベースで動作し、将来的なTor/I2P統合に影響しない。ライトクライアントはP2Pネットワーク経由で接続するため、フルノードへの直接接続が不要になり、IP露出リスクを低減可能 |
| II. Keyless UX | ✅ PASS | smoldot統合は署名方式に影響しない。既存のWebAuthn/パスキー署名フローはそのまま維持される |
| III. Client-Side Completion | ✅ PASS | smoldotはブラウザ内で動作するライトクライアントであり、暗号処理のクライアントサイド完結性に影響しない |
| IV. Zero-Trust Hydra | ✅ PASS | smoldotはブロックチェーンと直接通信するため、中間サーバー（RPC）への依存を削減し、ゼロトラスト設計を強化する |
| V. Economic Autonomy | ✅ PASS | 報酬システムに影響なし |
| VI. Test-First Development | ✅ PASS | 既存テストがsmoldot接続でも合格することを検証する |

**Gate Result**: ✅ ALL PASS - Phase 0に進行可能

## Project Structure

### Documentation (this feature)

```text
specs/014-smoldot-integration/
├── plan.md              # This file
├── research.md          # Phase 0: smoldot/PAPI統合調査
├── data-model.md        # Phase 1: ConnectionState定義
├── quickstart.md        # Phase 1: 開発者向けクイックスタート
├── contracts/           # Phase 1: N/A (内部変更のみ、外部API変更なし)
└── tasks.md             # Phase 2: 実装タスク
```

### Source Code (repository root)

```text
apps/frontend/
├── src/
│   ├── hooks/
│   │   ├── useApi.ts           # 変更: smoldotプロバイダーに置き換え
│   │   ├── useSmoldot.ts       # 新規: smoldot初期化・状態管理
│   │   └── useFaucet.ts        # 既存: 変更不要（useApi経由）
│   ├── types/
│   │   └── connection.ts       # 新規: ConnectionState型定義
│   ├── lib/
│   │   ├── chainspec.json      # 新規: Anarchyチェーンスペック
│   │   └── smoldot-provider.ts # 新規: PAPIプロバイダーラッパー
│   ├── components/             # 既存: 変更最小限（状態表示テキストのみ）
│   └── app/                    # 既存: 変更不要
└── tests/
    └── hooks/
        └── useSmoldot.test.ts  # 新規: smoldotフック単体テスト（オプション）

apps/blockchain/
├── scripts/
│   └── export-chainspec.sh     # 新規: チェーンスペック出力スクリプト
└── [既存構造は変更なし]
```

**Structure Decision**: 既存のMonorepo構造を維持。apps/frontendの接続層（hooks/useApi.ts）をsmoldotベースに置き換え、チェーンスペックをビルド時に静的埋め込み。

## Complexity Tracking

> 違反項目なし - Constitution Check全項目合格

# Implementation Plan: Frontend UI Redesign

**Branch**: `005-frontend-ui-redesign` | **Date**: 2026-02-08 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/005-frontend-ui-redesign/spec.md`

## Summary

Anarchyフロントエンドに多言語対応（英語・日本語・中国語）とcMatrixスタイル背景アニメーション（Blood Glitchテーマ）を実装する。既存のNext.js 14 App Router構造を活かし、React Context APIで言語状態を管理、Canvas APIで背景アニメーションを描画する。

## Technical Context

**Language/Version**: TypeScript 5.3, React 18.2, Node.js 20+  
**Primary Dependencies**: Next.js 14.1 (App Router), React 18, polkadot-api  
**Storage**: localStorage（言語設定の永続化）  
**Testing**: Jest + React Testing Library, Playwright（E2E）  
**Target Platform**: モダンブラウザ（Chrome, Firefox, Safari, Edge 最新2バージョン）  
**Project Type**: web（既存のapps/frontend構造を拡張）  
**Performance Goals**: 60fps背景アニメーション、言語切替500ms以内  
**Constraints**: バンドルサイズ増加を最小限に、モバイルでのCPU負荷考慮  
**Scale/Scope**: 3言語対応、翻訳対象UI要素約50箇所

### Existing Color Scheme

```css
--bg-primary: #0a0a0a;      /* 背景メイン */
--bg-secondary: #141414;    /* 背景サブ */
--text-primary: #ffffff;    /* テキストメイン */
--text-secondary: #888888;  /* テキストサブ */
--accent: #ff4444;          /* アクセント（赤） */
--border: #2a2a2a;          /* ボーダー */
```

### Blood Glitch Color Palette

```css
--matrix-main: #333333;     /* 文字落下の基本色（ダークグレー） */
--matrix-head: #999999;     /* 先頭文字（ライトグレー） */
--matrix-glitch: #CC0000;   /* Blood Glitch（赤） */
--matrix-trail: rgba(0, 0, 0, 0.05); /* 残像 */
```

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Network Anonymity | ✅ N/A | フロントエンドUI変更のみ、ネットワーク層に影響なし |
| II. Keyless UX | ✅ N/A | 認証フローに変更なし |
| III. Client-Side Completion | ✅ N/A | 暗号化処理に変更なし |
| IV. Zero-Trust Hydra | ✅ PASS | 言語設定はlocalStorageのみ、機密情報なし |
| V. Economic Autonomy | ✅ N/A | トークン経済に影響なし |
| VI. Test-First Development | ⚠️ REQUIRED | 各コンポーネントのテストを先に作成すること |

**Gate Result**: ✅ PASS（ただしTest-First Developmentの遵守が必要）

## Project Structure

### Documentation (this feature)

```text
specs/005-frontend-ui-redesign/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── i18n-api.md      # 言語APIコントラクト
└── tasks.md             # Phase 2 output
```

### Source Code (repository root)

```text
apps/frontend/
├── src/
│   ├── app/
│   │   ├── layout.tsx           # [MODIFY] 言語Provider追加、背景コンポーネント追加
│   │   ├── page.tsx
│   │   └── globals.css          # [MODIFY] Matrix色変数追加
│   ├── components/
│   │   ├── MatrixBackground.tsx # [NEW] cMatrix背景アニメーション
│   │   ├── MatrixBackground.module.css
│   │   ├── LanguageSwitcher.tsx # [NEW] 言語切替UI
│   │   ├── LanguageSwitcher.module.css
│   │   └── ... (existing)
│   ├── hooks/
│   │   ├── useLocale.ts         # [NEW] 言語状態フック
│   │   ├── useReducedMotion.ts  # [NEW] prefers-reduced-motion検出
│   │   └── ... (existing)
│   ├── i18n/
│   │   ├── index.ts             # [NEW] i18nエクスポート
│   │   ├── context.tsx          # [NEW] LocaleContext Provider
│   │   ├── translations/
│   │   │   ├── en.json          # [NEW] 英語翻訳
│   │   │   ├── ja.json          # [NEW] 日本語翻訳
│   │   │   └── zh.json          # [NEW] 中国語翻訳
│   │   └── types.ts             # [NEW] 型定義
│   └── lib/
│       └── matrix/
│           ├── index.ts         # [NEW] Matrixアニメーションエンジン
│           ├── config.ts        # [NEW] 設定定数
│           └── types.ts         # [NEW] 型定義
└── tests/
    ├── components/
    │   ├── MatrixBackground.test.tsx
    │   └── LanguageSwitcher.test.tsx
    └── hooks/
        ├── useLocale.test.ts
        └── useReducedMotion.test.ts
```

**Structure Decision**: 既存のapps/frontend構造を維持し、i18n/とlib/matrix/を新規追加。Next.js 14 App Routerパターンに従いServer/Client Componentsを適切に分離。

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| なし | - | - |

**Note**: 本機能はシンプルなフロントエンドUI変更であり、Constitution違反やアーキテクチャ上の複雑性は発生しない。

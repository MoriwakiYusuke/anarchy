# Implementation Plan: フロントエンドWebAuthn統合

**Branch**: `003-frontend-webauthn` | **Date**: 2026-02-07 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/003-frontend-webauthn/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

フロントエンドにWebAuthn（パスキー）機能を統合し、ユーザーがシードフレーズなしで安全にIdentity登録・署名付き投稿を行えるようにする。主要機能は: (1) パスキー登録によるIdentity作成、(2) WYSIWYS署名付き投稿、(3) マルチデバイス対応。React Hooks + PAPI で実装し、ネイティブWebAuthn APIを直接使用する。

## Technical Context

**Language/Version**: TypeScript 5.3.3  
**Primary Dependencies**: Next.js 14.1.0, React 18.2.0, polkadot-api 1.23.3, cbor-x 1.5.x  
**Storage**: LocalStorage (クレデンシャルID永続化のみ)  
**Testing**: Vitest 1.2.x + Testing Library + Playwright  
**Target Platform**: Web (Chrome 67+, Safari 14+, Firefox 60+, Edge 79+)
**Project Type**: Web application (frontend)  
**Performance Goals**: 登録30秒以内、署名投稿10秒以内（ダイアログ操作時間除く）  
**Constraints**: WebAuthn Level 2対応ブラウザ必須、HTTPS（localhost除く）  
**Scale/Scope**: 初期は単一ユーザーフロー、3コンポーネント + 4フック + 2ユーティリティ

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Network Anonymity | ✅ PASS | フロントエンドIP露出は許容範囲（Constitutionで明記） |
| II. Keyless UX | ✅ ALIGNED | WebAuthn/Passkey使用が核心要件 |
| III. Client-Side Completion | ✅ ALIGNED | WebAuthn署名はクライアント側で完結 |
| IV. Zero-Trust Hydra | ✅ ALIGNED | WYSIWYS実装（FR-004）で署名内容を保証 |
| V. Economic Autonomy | ✅ ALIGNED | Moral消費フローを維持 |
| VI. Test-First Development | ✅ PLANNED | Vitest + Playwright でテスト実装予定 |

**Post-Design Re-check**: 全原則に準拠。違反なし。

## Project Structure

### Documentation (this feature)

```text
specs/003-frontend-webauthn/
├── plan.md              # This file
├── spec.md              # Feature specification
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── webauthn-hooks.md
└── tasks.md             # Phase 2 output (NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
apps/frontend/
├── src/
│   ├── hooks/
│   │   ├── useApi.ts                   # 既存: PAPI接続
│   │   ├── useMoralBalance.ts          # 既存: Moral残高
│   │   ├── usePostCost.ts              # 既存: 投稿コスト
│   │   ├── useWebAuthn.ts              # 新規: メインフック
│   │   ├── useWebAuthnRegistration.ts  # 新規: 登録フロー
│   │   ├── useWebAuthnSigning.ts       # 新規: 署名フロー
│   │   └── useWebAuthnSupport.ts       # 新規: 機能検出
│   ├── utils/
│   │   ├── webauthn.ts                 # 新規: WebAuthn操作
│   │   └── cose.ts                     # 新規: COSE公開鍵抽出
│   ├── components/
│   │   ├── PostForm.tsx                # 既存: 拡張予定
│   │   ├── PasskeyRegister.tsx         # 新規: 登録UI
│   │   ├── PasskeySignPost.tsx         # 新規: 署名投稿UI
│   │   └── WebAuthnGate.tsx            # 新規: 機能ゲート
│   ├── contexts/
│   │   └── WebAuthnContext.tsx         # 新規: グローバル状態
│   └── __tests__/
│       ├── useWebAuthn.test.ts         # 新規
│       ├── webauthn.test.ts            # 新規
│       └── cose.test.ts                # 新規
├── vitest.config.ts                    # 新規: テスト設定
└── playwright.config.ts                # 新規: E2E設定
```

**Structure Decision**: 既存のフロントエンド構造（hooks/, components/）を維持しつつ、WebAuthn関連を追加。contextsとutilsディレクトリを新設。

## Complexity Tracking

> No Constitution violations requiring justification.

| Item | Complexity | Justification |
|------|------------|---------------|
| 4 hooks | Necessary | 関心分離（機能検出、登録、署名、統合） |
| 2 utils | Necessary | COSEとWebAuthnロジックの再利用 |
| 3 components | Minimal | 登録UI、署名UI、機能ゲート |

## Dependencies Added

| Package | Version | Purpose |
|---------|---------|---------|
| cbor-x | ^1.5.0 | COSE公開鍵のCBORデコード |
| @noble/hashes | ^1.3.0 | SHA-256（Web Crypto fallback） |
| vitest | ^1.2.0 | 単体/統合テスト |
| @testing-library/react | ^14.0.0 | コンポーネントテスト |
| @vitejs/plugin-react | ^4.2.0 | Vitest React対応 |
| playwright | ^1.40.0 | E2Eテスト |

## Implementation Phases

### Phase 1: 基盤 (P1 prerequisite)
- useWebAuthnSupport.ts
- utils/cose.ts
- utils/webauthn.ts
- Vitest設定

### Phase 2: パスキー登録 (US1)
- useWebAuthnRegistration.ts
- PasskeyRegister.tsx
- 単体テスト

### Phase 3: 署名投稿 (US2)
- useWebAuthnSigning.ts
- PasskeySignPost.tsx / PostForm拡張
- 単体テスト

### Phase 4: 統合 (US1-3)
- useWebAuthn.ts（統合フック）
- WebAuthnContext.tsx
- E2Eテスト

## References

- [spec.md](./spec.md) - 機能仕様
- [research.md](./research.md) - 技術調査
- [data-model.md](./data-model.md) - データモデル
- [quickstart.md](./quickstart.md) - クイックスタート
- [contracts/webauthn-hooks.md](./contracts/webauthn-hooks.md) - APIコントラクト
- [Identity Pallet Contract](../001-identity-pallet/contracts/identity-pallet.md) - バックエンドAPI

# Specification Quality Checklist: ウォレット認証統合

**Purpose**: Validate specification completeness and quality before proceeding to planning  
**Created**: 2026-02-08  
**Updated**: 2026-02-08  
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Scope Summary

### フロントエンド要件（FR-001〜FR-007）
- Polkadot.js Extension連携
- AccountId選択・セッション管理
- ウォレット署名によるSigned Extrinsic送信
- polkadot-api（PAPI）使用

### ブロックチェーン要件（FR-008〜FR-009）
- **Post Pallet**: 既存`create_post`をそのまま使用
- **Identity Pallet**: 本スコープでは不使用

## Design Decision

### WebAuthnアプローチ廃止の経緯

以下の理由によりWebAuthnアプローチを完全に廃止:

1. **rpId問題**: WebAuthnはドメインに紐付く → 異なるハイドラ間でのパスキー共有が困難
2. **擬似AccountId問題**: P-256公開鍵ハッシュからのAccountId導出 → 対応秘密鍵が存在しない
3. **複雑性**: Unsigned Extrinsic + オンチェーンWebAuthn検証という非標準的アプローチ
4. **本質的相性**: WebAuthnは「Web2の究極の進化系」であり、分散プロトコルと相性が悪い

### 新アプローチ: ウォレット方式

- **標準Substrate互換**: Signed Extrinsicをそのまま使用
- **クロスハイドラ**: ウォレットがドメイン非依存
- **エコシステム活用**: 既存ウォレット（Polkadot.js等）をそのまま利用
- **シンプル**: 特殊なオンチェーン検証ロジック不要

## Notes

- ブランチ名のリネーム推奨: `003-frontend-webauthn` → `003-wallet-auth`
- 旧spec.mdは`spec.md.bak`にバックアップ済み
- Identity Palletは将来的にプロフィール・設定用として残す可能性あり

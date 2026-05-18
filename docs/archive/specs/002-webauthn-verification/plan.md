# Implementation Plan: WebAuthn署名検証

**Branch**: `002-webauthn-verification` | **Date**: 2026-02-07 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/002-webauthn-verification/spec.md`

## Summary

WebAuthn（パスキー）で生成されたES256署名をSubstrateランタイム内でオンチェーン検証する機能を実装する。これにより、ユーザーが生体認証で投稿を行う際、なりすまし防止とWYSIWYS（What You See Is What You Sign）を実現する。

## Technical Context

**Language/Version**: Rust 1.75+ (Polkadot SDK stable2503)  
**Primary Dependencies**:
- p256 ^0.13 (P-256曲線、no_std対応)
- ecdsa ^0.16 (ECDSA署名検証、no_std対応)
- coset (COSEパーサー) または 手動CBORパース
- sha2 ^0.10 (SHA-256、no_std対応)

**Storage**: Substrate on-chain storage（既存Identity Palletを拡張）  
**Testing**: `cargo test -p pallet-identity`, `cargo test -p pallet-post`  
**Target Platform**: Linux server, WASM runtime (no_std)  
**Project Type**: Substrate blockchain pallets  
**Performance Goals**: 署名検証を含むエクストリンシックが6秒以内に処理される  
**Constraints**: no_std環境で動作、Wasmランタイム内で実行可能  
**Scale/Scope**: 1ブロックあたり複数の署名検証を処理可能

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原則 | 状態 | 根拠 |
|-----|------|-----|
| I. Network Anonymity | ✅ 関係なし | 署名検証はネットワーク層に影響しない |
| II. Keyless UX | ✅ 強化 | WebAuthn検証によりパスキー認証を完全にサポート |
| III. Client-Side Completion | ✅ 維持 | 署名生成はクライアント側、検証のみオンチェーン |
| IV. Zero-Trust Hydra | ✅ 強化 | WYSIWYS実装でなりすまし防止 |
| V. Economic Autonomy | ✅ 関係なし | 経済モデルに影響しない |
| VI. Test-First Development | ✅ 遵守 | テストケース先行で実装 |

**ゲート評価**: ✅ PASS - Constitution違反なし、原則II/IVを強化

## Project Structure

### Documentation (this feature)

```text
specs/002-webauthn-verification/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── webauthn-api.md
└── tasks.md             # Phase 2 output
```

### Source Code (repository root)

```text
apps/blockchain/
├── pallets/
│   ├── identity/
│   │   ├── Cargo.toml        # p256, ecdsa, sha2 依存追加
│   │   └── src/
│   │       ├── lib.rs        # WebAuthn検証統合
│   │       ├── webauthn.rs   # 新規: WebAuthn検証モジュール
│   │       ├── cose.rs       # 新規: COSEパーサー
│   │       └── tests.rs      # テスト拡張
│   └── post/
│       └── src/
│           └── lib.rs        # 署名検証付き投稿エクストリンシック
└── runtime/
    └── src/
        └── lib.rs            # ランタイム設定（必要に応じて）
```

**Structure Decision**: 既存のIdentity Palletを拡張し、`webauthn.rs`と`cose.rs`モジュールを追加。Post Palletは署名検証付きの新しいエクストリンシックを追加。

## Complexity Tracking

> 該当なし - Constitution違反なし
## Post-Design Constitution Re-evaluation

*Phase 1設計完了後の再評価: 2026-02-07*

| 原則 | 設計前 | 設計後 | 変更理由 |
|-----|--------|--------|---------|
| I. Network Anonymity | ✅ | ✅ | 変更なし - 署名検証はネットワーク層に影響しない |
| II. Keyless UX | ✅ 強化 | ✅ 強化 | WYSIWYSチャレンジ設計により、パスキーのみで安全な投稿が可能に |
| III. Client-Side Completion | ✅ | ✅ | 署名生成はクライアント、検証はオンチェーン - 原則維持 |
| IV. Zero-Trust Hydra | ✅ 強化 | ✅ 強化 | challengeにcontent_hashを埋め込むことで、悪意あるフロントエンドからのなりすましを防止 |
| V. Economic Autonomy | ✅ | ✅ | 変更なし |
| VI. Test-First Development | ✅ | ✅ | テストケースをquickstart.mdで定義済み |

**最終評価**: ✅ PASS - 全原則を満たし、II/IVを積極的に強化

## Generated Artifacts

| ファイル | 状態 | 説明 |
|---------|------|-----|
| [research.md](./research.md) | ✅ | no_stdクレート調査、WebAuthnデータ構造、Substrate統合方針 |
| [data-model.md](./data-model.md) | ✅ | エンティティ定義、バリデーションルール、ストレージスキーマ |
| [contracts/webauthn-api.md](./contracts/webauthn-api.md) | ✅ | エクストリンシック定義、内部API、エラー型 |
| [quickstart.md](./quickstart.md) | ✅ | セットアップ手順、実装フロー、テスト方法 |
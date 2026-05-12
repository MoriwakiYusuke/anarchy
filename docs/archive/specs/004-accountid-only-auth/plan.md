# Implementation Plan: AccountIdのみによる認証への移行

**Branch**: `004-accountid-only-auth` | **Date**: 2026-02-08 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/004-accountid-only-auth/spec.md`

## Summary

WebAuthn（パスキー）認証を廃止し、Substrate標準のAccountId（公開鍵）ベースの認証に移行する。これにより、分散プロトコルとの相性問題を解消し、コードベースを大幅に簡素化する。002-webauthn-verificationで追加したCOSE/ES256検証コードを削除し、001-identity-palletを完全に削除する（WebAuthn前提の設計が不要になったため）。

## Technical Context

**Language/Version**: Rust 1.75+ (Polkadot SDK stable2503)  
**Primary Dependencies**:
- 削除: p256, ecdsa（WebAuthn検証用）
- 維持: sha2（コンテンツハッシュ用）
- 新規: なし（Substrate標準機能で完結）

**Storage**: Substrate on-chain storage（Post Palletのみ）  
**Testing**: `cargo test -p pallet-post`  
**Target Platform**: Linux server, WASM runtime (no_std)  
**Project Type**: Substrate blockchain pallets + Next.js frontend  
**Performance Goals**: 標準トランザクション処理（WebAuthn検証オーバーヘッド削除による高速化）  
**Constraints**: no_std環境で動作、Wasmランタイム内で実行可能  
**Scale/Scope**: 既存コードの削除・簡素化（net negative LOC）

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原則 | 状態 | 根拠 |
|-----|------|-----|
| I. Network Anonymity | ✅ 強化 | AccountIdは公開鍵ベース、外部ウォレットと紐付けなしで完全匿名 |
| II. Keyless UX | ⚠️ 変更 | パスキーUXからシードフレーズ入力方式へ変更（トレードオフを受容） |
| III. Client-Side Completion | ✅ 維持 | 署名はクライアント側ブラウザ内で完結 |
| IV. Zero-Trust Hydra | ✅ 維持 | トランザクション署名による認証は維持 |
| V. Economic Autonomy | ✅ 関係なし | 経済モデルに影響しない |
| VI. Test-First Development | ✅ 遵守 | 削除前にテスト実行、削除後に再テスト |

**ゲート評価**: ✅ PASS - Constitution違反なし

**原則IIについての補足**:
WebAuthnは「秘密鍵をユーザーに扱わせない」ためのアプローチだったが、ドメイン依存により分散プロトコルと本質的に相性が悪い。シードフレーズ方式は「生の秘密鍵」ではなく12語のニーモニックであり、ユーザーが扱いやすい形式。また、外部ウォレットとの紐付けがないためプライバシーが強化される。

## Project Structure

### Documentation (this feature)

```text
specs/004-accountid-only-auth/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── identity-api.md
└── tasks.md             # Phase 2 output
```

### Source Code (repository root)

```text
apps/blockchain/
├── Cargo.toml              # ワークスペース依存更新（p256, ecdsa削除）
├── pallets/
│   ├── identity/           # 削除
│   └── post/
│       └── src/
│           ├── lib.rs      # WebAuthn関連コード削除、Identity依存削除
│           └── tests.rs    # WebAuthnテスト削除
└── runtime/
    └── src/
        └── lib.rs          # Identity Pallet参照削除

apps/frontend/
└── src/
    ├── components/
    │   └── WalletConnect.tsx  # ウォレット接続UIに変更
    └── hooks/
        └── useWallet.ts       # ウォレット接続フック（既存useApi.tsを拡張）
```

**Structure Decision**: 既存コードからWebAuthn関連を削除し、Identity Palletを完全に削除。フロントエンドはWebAuthn APIからSubstrate Wallet APIへ変更。

## Complexity Tracking

> 該当なし - むしろ複雑性が削減される

## Change Summary

### 削除対象（002-webauthn-verification由来）

| ファイル | 行数 | 内容 |
|---------|------|-----|
| `pallets/identity/src/cose.rs` | ~570行 | COSEパーサー |
| `pallets/identity/src/webauthn.rs` | ~720行 | WebAuthn署名検証 |
| `pallets/identity/src/lib.rs` (部分) | ~50行 | WebAuthn関連use/mod |
| `pallets/post/src/lib.rs` (部分) | ~100行 | WebAuthnエクストリンシック |
| `pallets/post/src/tests.rs` (部分) | ~150行 | WebAuthnテスト |
| 依存関係: `p256`, `ecdsa` | - | Cargo.toml |

**削除合計**: 約1,600行

### 削除対象（001-identity-pallet）

| ファイル/ディレクトリ | 内容 |
|---------------------|------|
| `pallets/identity/` | Identity Pallet全体 |
| `runtime/src/lib.rs` (部分) | Identity Pallet参照 |
| `pallets/post/` (部分) | Identity Palletへの依存 |

### 維持される機能

- 投稿作成（`create_post` extrinsic）
- $moral コスト消費
- コンテンツハッシュ
- 親投稿参照（リプライ）

## Generated Artifacts

| ファイル | 状態 | 説明 |
|---------|------|-----|
| [research.md](./research.md) | ✅ | 移行理由、ウォレット統合方針 |
| [quickstart.md](./quickstart.md) | ✅ | 移行手順、テスト方法 |

**Note**: data-model.md, contracts/identity-api.md は Identity Pallet 削除により不要となり削除済み。

# Implementation Tasks: ウォレット認証統合

**Feature Branch**: `003-frontend-webauthn` → リネーム推奨: `003-wallet-auth`  
**Created**: 2026-02-08  
**Updated**: 2026-02-08  
**Spec**: [spec.md](spec.md)

## Task Overview

| Phase | 作業領域 | タスク数 | 依存関係 |
|-------|---------|---------|---------|
| 1 | フロントエンド - ウォレット接続 | 3 | なし |
| 2 | フロントエンド - 投稿フロー | 2 | Phase 1完了後 |
| 3 | クリーンアップ（任意） | 2 | 本スコープ外 |

---

## Phase 1: ウォレット接続

### T1.1: Polkadot.js Extension連携
**FR**: FR-001, FR-002  
**ファイル**: `apps/frontend/src/hooks/useWallet.ts`（新規）

- [ ] `@polkadot/extension-dapp` パッケージ追加
- [ ] `web3Enable('Anarchy')` でアプリ登録
- [ ] `web3Accounts()` で利用可能なAccountIdを取得
- [ ] ウォレット未インストール検出

### T1.2: ウォレット接続UI
**FR**: FR-005  
**ファイル**: `apps/frontend/src/components/WalletConnect.tsx`

- [ ] 接続ボタンコンポーネント
- [ ] AccountIdセレクタ（複数アカウント対応）
- [ ] 未インストール時のガイダンス表示
- [ ] 接続状態の表示（接続中AccountId）

### T1.3: セッション管理
**FR**: FR-003  
**ファイル**: `apps/frontend/src/hooks/useSession.ts`（新規）

- [ ] 選択中AccountIdの状態管理
- [ ] 接続ウォレット情報の保持
- [ ] セッション永続化（localStorage、オプション）

---

## Phase 2: 投稿フロー

### T2.1: ウォレット署名による投稿
**FR**: FR-004, FR-007  
**ファイル**: `apps/frontend/src/hooks/usePost.ts`（既存修正）

- [ ] polkadot-api（PAPI）でExtrinsic構築
- [ ] `web3FromAddress()` でSigner取得
- [ ] Signed Extrinsic送信（既存`create_post`使用）
- [ ] 署名拒否時のエラーハンドリング

### T2.2: 投稿フォーム更新
**FR**: FR-006  
**ファイル**: `apps/frontend/src/components/PostForm.tsx`

- [ ] ウォレット接続チェック
- [ ] 署名プロセス中のローディング表示
- [ ] 残高不足エラー表示
- [ ] 成功/失敗フィードバック

---

## Phase 3: クリーンアップ（任意・スコープ外）

> **Note**: 以下はウォレット方式が安定稼働した後の任意作業

### T3.1: Identity Pallet WebAuthn機能削除（任意）
**ファイル**: `apps/blockchain/pallets/identity/src/`

- [ ] WebAuthn関連コード（cose.rs, webauthn.rs）の削除検討
- [ ] Passkey関連ストレージ削除検討
- [ ] または将来のプロフィール機能用に残す

### T3.2: Post Pallet WebAuthn機能削除（任意）
**ファイル**: `apps/blockchain/pallets/post/src/lib.rs`

- [ ] `create_post_with_webauthn` extrinsic削除検討
- [ ] 標準`create_post`のみ残す
- [ ] **ウォレット認証**: Polkadot.js等のAccountId秘密鍵での認証
- [ ] 認証方式の選択UI
- [ ] 認証後のIdentityId/AccountId取得（チェーンから照会）

### T4.4: UI統合
**FR**: FR-007, FR-008, FR-010  
**ファイル**: `apps/frontend/src/components/`

- [ ] 登録フォームコンポーネント（パスキー / ウォレット切り替え）
- [ ] WebAuthn非対応検出とエラー表示
- [ ] 生体認証キャンセル時のハンドリング
- [ ] クロスデバイス認証オプション表示
- [ ] ウォレット接続コンポーネント（Polkadot.js対応）

### T4.5: 投稿フォーム統合
**ファイル**: `apps/frontend/src/components/PostForm.tsx`

- [ ] パスキー署名ボタン追加
- [ ] 署名フローとExtrinsic送信の統合
- [ ] エラーハンドリングと再試行UI

---

## テスト要件

### ユニットテスト
- [ ] `derive_account_id` の計算結果が決定的であること
- [ ] WebAuthn署名検証のエッジケース
- [ ] MORAL残高不足時のエラーハンドリング
- [ ] AccountIdentities（1:N）の正しい動作

### 統合テスト  
- [ ] パスキー登録 → 投稿の全フロー
- [ ] 秘密鍵ユーザーの`link_identity`フロー
- [ ] 秘密鍵ユーザーの`register_identity_signed`フロー
- [ ] 異なるフロントエンドからのdiscoverable credentials認証
- [ ] 不正署名の拒否

### E2Eテスト（将来）
- [ ] 実デバイスでのWebAuthnフロー
- [ ] クロスデバイス認証
- [ ] クロスハイドラ認証（異なるドメインからの認証）

---

## 依存関係図

```
T1.1 ─┬─ T1.2 ─┬─ T1.3 ── T1.4
      │        │     │
      │        │     ├── T1.5 (link_identity)
      │        │     └── T1.6 (register_identity_signed)
      │        │
      └────────┴─── T2.1 ── T2.2 ── T2.3
                      │
                      └─── T3.1 ── T3.2
                             │
                             └─── T4.1 ── T4.2 ── T4.3 ── T4.4 ── T4.5
```

---

## 見積もり

| Phase | 工数目安 | 備考 |
|-------|---------|------|
| Phase 1 | 3-4日 | T1.5, T1.6追加で増加 |
| Phase 2 | 1-2日 | Phase 1のパターンを踏襲 |
| Phase 3 | 0.5日 | 設定のみ |
| Phase 4 | 4-5日 | ウォレット認証追加で増加 |
| **合計** | **9-12日** | |

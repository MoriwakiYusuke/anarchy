# Research: WebAuthn廃止とAccountIdベース認証への移行

**Feature**: 004-accountid-only-auth  
**Date**: 2026-02-08

## 1. WebAuthn廃止の技術的根拠

### 1.1 WebAuthnの設計思想と分散プロトコルの相性問題

WebAuthn（FIDO2）は以下の前提で設計されている：

1. **Relying Party (RP)**: 認証を要求する「信頼できるサーバー」の存在
2. **rpId**: RPを識別するドメイン名（例: `example.com`）
3. **Origin Check**: ブラウザがoriginを検証し、フィッシングを防止
4. **Challenge-Response**: サーバーが発行するチャレンジへの応答

これらは全て**中央集権的なドメイン**を前提としており、以下の問題が発生する：

| 問題 | 説明 |
|-----|-----|
| **ドメイン依存** | パスキーは特定のドメインに紐付く。分散ネットワークでは「正式なドメイン」が存在しない |
| **ハイドラ間の互換性** | 異なるドメインで運用される複数のフロントエンド（ハイドラ）間でパスキーを共有できない |
| **rpIdのハードコード問題** | オンチェーン検証でrpIdをどう扱うか？全てのハイドラのドメインを許可すると本来のフィッシング防止機能が無意味に |
| **オフライン検証不可** | WebAuthnはサーバーとの往復を前提としており、完全オフラインのP2P検証に不向き |

### 1.2 実装上の問題

002-webauthn-verificationで実装したコードの課題：

```
問題1: ランタイムサイズの肥大化
- p256クレート: ~50KB (Wasm)
- ecdsa検証ロジック: 計算コスト
- COSEパーサー: 570行の追加コード

問題2: no_std環境での制約
- 一部のWebAuthn検証（clientDataJSON等）はノーマライズ処理が必要
- 文字列処理がno_std環境で制限される

問題3: 保守コスト
- WebAuthnは継続的に進化しており（Passkey同期等）、追従コストが高い
```

### 1.3 WebAuthnが解決しようとした問題の再評価

| 当初の目的 | WebAuthnでの解決策 | AccountIdでの解決策 |
|-----------|-------------------|-------------------|
| 秘密鍵をユーザーに扱わせない | パスキー（生体認証） | ウォレットアプリ（Polkadot.js等） |
| フィッシング防止 | Origin検証 | 署名内容の表示（ウォレットUI） |
| なりすまし防止 | WYSIWYS | トランザクション署名 |

**結論**: ウォレットアプリケーションが秘密鍵管理を十分に抽象化しており、WebAuthnの追加価値が実装コストを正当化しない。

---

## 2. AccountIdベース認証のアーキテクチャ

### 2.1 Substrateの認証モデル

```
┌─────────────────────────────────────────────────────────────┐
│                     User's Device                           │
│  ┌─────────────────────┐    ┌────────────────────────────┐  │
│  │   Wallet Extension  │    │       Frontend (Hydra)     │  │
│  │   ・秘密鍵保管       │←──→│  ・UI                      │  │
│  │   ・署名生成         │    │  ・トランザクション構築    │  │
│  │   ・署名確認UI       │    │  ・ウォレット接続API呼出   │  │
│  └─────────────────────┘    └────────────────────────────┘  │
└───────────────────────────────│──────────────────────────────┘
                                │ Signed Extrinsic
                                ▼
┌─────────────────────────────────────────────────────────────┐
│                   Substrate Node                            │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                    Runtime                           │    │
│  │  ・署名検証（ed25519/sr25519）                      │    │
│  │  ・origin = AccountId（署名者の公開鍵）             │    │
│  │  ・トランザクション実行                             │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 認証フロー

```
1. ユーザーがフロントエンドで操作（投稿作成等）
   ↓
2. フロントエンドがトランザクションを構築
   ↓
3. ウォレット拡張がトランザクション内容を表示
   ↓
4. ユーザーが承認（パスワード入力またはデバイス認証）
   ↓
5. ウォレットが秘密鍵で署名
   ↓
6. 署名付きトランザクションをノードに送信
   ↓
7. ランタイムが署名を検証し、origin（AccountId）を確定
   ↓
8. エクストリンシックを実行
```

### 2.3 セキュリティ比較

| 観点 | WebAuthn | AccountId + Wallet |
|-----|----------|-------------------|
| 秘密鍵保護 | デバイスのSecure Enclave | ウォレット暗号化ストレージ |
| フィッシング耐性 | Origin検証（自動） | ウォレットUI確認（手動） |
| 署名内容確認 | Challengeに埋込 | ウォレットUIで表示 |
| リカバリ | デバイス依存（困難） | シードフレーズ |
| 相互運用性 | ドメイン単位 | 全Substrate互換 |

---

## 3. 対応ウォレット

### 3.1 Polkadot.js Extension（推奨）

- **URL**: https://polkadot.js.org/extension/
- **サポート**: Chrome, Firefox, Brave
- **特徴**: 
  - Substrate標準
  - アカウント管理UI
  - トランザクション署名確認
  - 複数アカウント対応

### 3.2 その他のウォレット

| ウォレット | プラットフォーム | 状態 |
|-----------|----------------|------|
| Talisman | Browser Extension | 対応可能 |
| SubWallet | Browser/Mobile | 対応可能 |
| Nova Wallet | Mobile | 将来対応 |

### 3.3 ウォレット接続API

```typescript
// @polkadot/extension-dapp
import { web3Enable, web3Accounts, web3FromAddress } from '@polkadot/extension-dapp';

// 1. ウォレット接続
const extensions = await web3Enable('Anarchy');

// 2. アカウント取得
const accounts = await web3Accounts();

// 3. 署名者取得
const injector = await web3FromAddress(account.address);

// 4. トランザクション署名・送信
await api.tx.post
  .createPost(content, null)
  .signAndSend(account.address, { signer: injector.signer });
```

---

## 4. 移行による影響

### 4.1 削除されるコンポーネント

| コンポーネント | ファイル | 理由 |
|--------------|---------|------|
| COSEパーサー | `cose.rs` | WebAuthn公開鍵解析用 |
| WebAuthn検証 | `webauthn.rs` | 署名検証ロジック |
| WebAuthn投稿 | `post/lib.rs`部分 | `create_post_with_webauthn` |
| Passkey管理 | `identity/lib.rs`部分 | 複数デバイス管理 |

### 4.2 簡素化されるストレージ

**Before (WebAuthn)**:
```
Identities: u64 → Identity { passkeys: Vec<Passkey> }
PasskeyOwner: [u8; 32] → u64
NextIdentityId: u64
```

**After (AccountId)**:
```
Identities: AccountId → Identity { created_at }
```

### 4.3 依存関係の削減

**削除されるクレート**:
- `p256` (~50KB Wasm)
- `ecdsa`

**維持されるクレート**:
- `sha2` (コンテンツハッシュ用)

---

## 5. 将来の拡張性

### 5.1 ソーシャルリカバリ（将来機能）

AccountIdベースでも、以下の方法でリカバリ機能を追加可能：

- **マルチシグ**: 複数の信頼できるアカウントによる承認
- **タイムロック**: 一定期間後に新アカウントへ移行
- **リカバリフレンド**: 指定したアカウントによる復旧承認

### 5.2 Passkey対応の可能性（将来）

WebAuthn標準が進化し、以下が実現すれば再検討可能：

- **ドメイン非依存のPasskey同期**: 現在はApple/Google/MSのプラットフォーム依存
- **分散RPサポート**: 標準化が進めば

---

## 6. 参考資料

- [WebAuthn Specification](https://www.w3.org/TR/webauthn-2/)
- [Polkadot.js Extension](https://github.com/polkadot-js/extension)
- [Substrate Signature Verification](https://docs.substrate.io/learn/accounts-addresses-keys/)

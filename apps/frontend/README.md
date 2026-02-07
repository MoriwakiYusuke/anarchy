# Anarchy Frontend

Next.js ベースの匿名分散型SNSフロントエンド。WebAuthn (パスキー) 認証と Substrate ブロックチェーン統合を特徴とします。

## 機能

- 🔐 **WebAuthn パスキー認証** - 生体認証やセキュリティキーでの登録・投稿署名
- ⛓️ **ブロックチェーン統合** - Polkadot-API (PAPI) による Substrate チェーン接続
- 📝 **WYSIWYS 署名** - What You See Is What You Sign 保証
- 🔄 **マルチデバイス対応** - 複数デバイスからの認証サポート
- 💰 **Moral トークン経済** - 投稿コストと信頼スコアシステム

## 技術スタック

- **Framework**: Next.js 14 (App Router)
- **Language**: TypeScript 5.9
- **UI**: React 18, CSS Modules
- **Blockchain**: polkadot-api (PAPI)
- **Crypto**: @noble/hashes, cbor-x
- **Testing**: Vitest, @testing-library/react, Playwright

## セットアップ

### 前提条件

- Node.js 20+
- pnpm 8+
- ローカルブロックチェーンノード (ws://localhost:9944)

### インストール

```bash
# リポジトリのルートから
pnpm install

# フロントエンドディレクトリに移動
cd apps/frontend
```

### 開発サーバー

```bash
# ブロックチェーンノードを先に起動
cd apps/blockchain
cargo run --release -- --dev

# 別ターミナルでフロントエンド起動
cd apps/frontend
pnpm dev
```

ブラウザで http://localhost:3000 にアクセス。

### テスト

```bash
# ユニットテスト
pnpm test        # ウォッチモード
pnpm test:run    # 単発実行

# E2Eテスト (Playwright)
pnpm test:e2e
```

## WebAuthn 統合

### パスキー登録フロー

1. ユーザーが「パスキーで登録」ボタンをクリック
2. ブラウザがWebAuthn API でパスキー作成を要求
3. ユーザーが生体認証/PIN を入力
4. フロントエンドがCOSE公開鍵を抽出
5. `register_identity` エクストリンシックをブロックチェーンに送信
6. Identity ID とパスキー情報を LocalStorage に保存

### 署名付き投稿フロー (WYSIWYS)

1. ユーザーが投稿内容を入力
2. 「署名して投稿」をクリック
3. WYSIWYS チャレンジを生成:
   ```
   challenge = "anarchy:post:" || SHA256(content) || ":end"
   ```
4. WebAuthn `credentials.get()` でパスキー署名を取得
5. `create_post_with_webauthn` エクストリンシックを送信

### マルチデバイス対応

- 既存の Identity に別デバイスのパスキーを追加可能
- `add_passkey` エクストリンシックで新しい公開鍵を登録
- 最大8個のパスキーを1つの Identity に紐付け可能

## プロジェクト構造

```
apps/frontend/
├── src/
│   ├── app/                    # Next.js App Router
│   │   ├── page.tsx            # メインページ
│   │   ├── layout.tsx          # ルートレイアウト
│   │   └── globals.css         # グローバルスタイル
│   ├── components/             # Reactコンポーネント
│   │   ├── PasskeyRegister.tsx # パスキー登録UI
│   │   ├── PasskeySignPost.tsx # 署名付き投稿UI
│   │   ├── WebAuthnGate.tsx    # WebAuthn可用性ゲート
│   │   ├── DeviceSettings.tsx  # デバイス管理UI
│   │   └── Timeline.tsx        # 投稿一覧
│   ├── hooks/                  # カスタムフック
│   │   ├── useWebAuthn.ts      # 統合WebAuthnフック
│   │   ├── useWebAuthnSupport.ts    # 機能検出
│   │   ├── useWebAuthnRegistration.ts # 登録フロー
│   │   └── useWebAuthnSigning.ts     # 署名フロー
│   ├── contexts/               # React Context
│   │   └── WebAuthnContext.tsx # WebAuthn状態管理
│   ├── utils/                  # ユーティリティ
│   │   ├── webauthn.ts         # WebAuthn関連ユーティリティ
│   │   └── cose.ts             # COSE公開鍵処理
│   ├── types/                  # 型定義
│   │   └── webauthn.ts         # WebAuthn型定義
│   └── __tests__/              # ユニットテスト
├── e2e/                        # E2Eテスト (Playwright)
├── vitest.config.ts            # Vitest設定
└── playwright.config.ts        # Playwright設定
```

## API リファレンス

### useWebAuthn Hook

```tsx
import { useWebAuthn } from '@/hooks/useWebAuthn'

function Component() {
  const {
    isSupported,            // WebAuthnサポート状況
    hasPlatformAuthenticator, // プラットフォーム認証器の有無
    identity,               // 現在のIdentity情報
    registrationStatus,     // 登録ステータス
    signingStatus,          // 署名ステータス
    registerPasskey,        // 新規登録
    signAndPost,            // 署名付き投稿
    addPasskey,             // パスキー追加
    loadIdentityById,       // Identity読み込み
    reset,                  // 状態リセット
    error,                  // エラー情報
  } = useWebAuthn({ api, signer })
}
```

### WebAuthnContext

```tsx
import { WebAuthnProvider, useWebAuthnContext } from '@/contexts/WebAuthnContext'

// アプリをラップ
<WebAuthnProvider api={api} signer={signer}>
  <App />
</WebAuthnProvider>

// コンテキストを使用
function Component() {
  const {
    identity,
    persistedCredentials,
    registerPasskey,
    addPasskey,
    // ...
  } = useWebAuthnContext()
}
```

## エラーコード

| コード | 説明 |
|--------|------|
| `USER_CANCELLED` | ユーザーが認証をキャンセル |
| `WEBAUTHN_NOT_SUPPORTED` | ブラウザがWebAuthn非対応 |
| `NO_PLATFORM_AUTHENTICATOR` | プラットフォーム認証器なし |
| `PASSKEY_ALREADY_REGISTERED` | パスキーが既に登録済み |
| `AUTHENTICATOR_ERROR` | 認証器エラー |
| `NETWORK_ERROR` | ネットワークエラー |
| `TRANSACTION_FAILED` | トランザクション失敗 |
| `INSUFFICIENT_BALANCE` | Moral残高不足 |
| `NO_IDENTITY` | Identityが未設定 |
| `TOO_MANY_PASSKEYS` | パスキー数上限到達 |

## ブラウザサポート

WebAuthn は以下のブラウザでサポートされています:

- Chrome/Edge 67+
- Firefox 60+
- Safari 14.1+

**推奨**: プラットフォーム認証器 (Touch ID, Windows Hello, Android生体認証) を搭載したデバイス

## 開発ノート

### テストでのWebAuthnモック

`src/__tests__/setup.ts` でWebAuthn APIがモックされています:

```ts
// navigator.credentials.create/get をモック
Object.defineProperty(global.navigator, 'credentials', {
  value: {
    create: vi.fn(),
    get: vi.fn(),
  },
})
```

### COSE公開鍵の抽出

WebAuthn の `attestationObject` から ES256 (P-256) 公開鍵を抽出:

```ts
import { extractCosePublicKey } from '@/utils/cose'

const credential = await navigator.credentials.create(options)
const coseKey = extractCosePublicKey(credential.response.attestationObject)
// coseKey: COSE_Key format for pallet_identity
```

### Passkey ID の計算

```ts
import { derivePasskeyId } from '@/utils/webauthn'

const passkeyId = derivePasskeyId(cosePublicKey)
// Blake2-256 hash of COSE public key
```

## ライセンス

MIT

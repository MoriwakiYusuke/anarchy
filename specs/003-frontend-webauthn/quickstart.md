# Quickstart: フロントエンドWebAuthn統合

**Date**: 2026-02-07  
**Feature**: 003-frontend-webauthn  
**Estimated Time**: 5-7 days

---

## Prerequisites

- Node.js 20+
- pnpm 8+
- ローカルブロックチェーンノード起動済み (`ws://127.0.0.1:9944`)
- WebAuthn対応ブラウザ（Chrome 108+, Safari 16+, Firefox 119+, Edge 108+）

---

## Quick Setup

```bash
# 1. フロントエンドディレクトリへ移動
cd apps/frontend

# 2. 依存関係インストール（既存 + 新規）
pnpm add cbor-x @noble/hashes

# 3. 開発用依存関係インストール
pnpm add -D vitest @testing-library/react @vitejs/plugin-react jsdom

# 4. 開発サーバー起動
pnpm dev
```

---

## File Structure Overview

```
apps/frontend/src/
├── hooks/
│   ├── useApi.ts              # 既存: PAPI接続
│   ├── useWebAuthn.ts         # 新規: メインフック
│   ├── useWebAuthnRegistration.ts  # 新規: 登録フロー
│   ├── useWebAuthnSigning.ts  # 新規: 署名フロー
│   └── useWebAuthnSupport.ts  # 新規: 機能検出
├── utils/
│   ├── webauthn.ts            # 新規: WebAuthn操作
│   └── cose.ts                # 新規: COSE公開鍵抽出
├── components/
│   ├── PasskeyRegister.tsx    # 新規: 登録コンポーネント
│   ├── PasskeySignPost.tsx    # 新規: 署名投稿コンポーネント
│   └── WebAuthnGate.tsx       # 新規: 機能ゲート
├── contexts/
│   └── WebAuthnContext.tsx    # 新規: グローバル状態
└── __tests__/
    ├── useWebAuthn.test.ts    # 新規: フックテスト
    └── webauthn.test.ts       # 新規: ユーティリティテスト
```

---

## Implementation Order

### Phase 1: 基盤 (Day 1-2)

1. **機能検出** (`useWebAuthnSupport.ts`)
   - WebAuthn API 存在確認
   - Platform authenticator 検出

2. **ユーティリティ** (`utils/webauthn.ts`, `utils/cose.ts`)
   - COSE公開鍵抽出
   - Challenge生成
   - Base64URL エンコード/デコード

3. **テスト基盤**
   - Vitest設定
   - WebAuthn APIモック

### Phase 2: パスキー登録 (Day 3-4)

4. **登録フック** (`useWebAuthnRegistration.ts`)
   - WebAuthn登録フロー
   - COSE公開鍵抽出
   - `register_identity` extrinsic送信

5. **登録コンポーネント** (`PasskeyRegister.tsx`)
   - 登録ボタン
   - 状態表示（ローディング、成功、エラー）

### Phase 3: 署名投稿 (Day 5-6)

6. **署名フック** (`useWebAuthnSigning.ts`)
   - WYSIWYS challenge生成
   - WebAuthn署名取得
   - `create_post_with_webauthn` extrinsic送信

7. **署名投稿コンポーネント** (`PasskeySignPost.tsx`)
   - 既存PostFormの拡張
   - 署名フロー統合

### Phase 4: 統合 (Day 7)

8. **Context統合** (`WebAuthnContext.tsx`)
   - グローバル状態管理
   - LocalStorage永続化

9. **E2Eテスト**
   - Playwright Virtual Authenticator

---

## Key Code Snippets

### WebAuthn登録（簡易版）

```typescript
// hooks/useWebAuthnRegistration.ts
async function registerPasskey(deviceName?: string) {
  // 1. WebAuthn registration
  const credential = await navigator.credentials.create({
    publicKey: {
      challenge: crypto.getRandomValues(new Uint8Array(32)),
      rp: { name: "Anarchy", id: window.location.hostname },
      user: { 
        id: crypto.getRandomValues(new Uint8Array(16)),
        name: "user",
        displayName: "Anarchy User"
      },
      pubKeyCredParams: [{ alg: -7, type: "public-key" }],
      authenticatorSelection: {
        authenticatorAttachment: "platform",
        userVerification: "required",
        residentKey: "required",
      },
      attestation: "none",
    },
  });

  // 2. Extract COSE public key
  const attestation = credential.response as AuthenticatorAttestationResponse;
  const coseKey = extractCosePublicKey(attestation.attestationObject);

  // 3. Submit to blockchain
  const tx = api.tx.Identity.register_identity({
    public_key: Array.from(coseKey),
    device_name: deviceName ? Array.from(new TextEncoder().encode(deviceName)) : null,
  });
  
  const result = await tx.signAndSubmit(signer);
  return result;
}
```

### WYSIWYS署名（簡易版）

```typescript
// hooks/useWebAuthnSigning.ts
async function signAndPost(content: string) {
  // 1. Generate challenge with content hash
  const challenge = await generateWysiwysChallenge(content);

  // 2. Get WebAuthn assertion
  const assertion = await navigator.credentials.get({
    publicKey: {
      challenge,
      rpId: window.location.hostname,
      allowCredentials: [],
      userVerification: "required",
    },
  });

  // 3. Submit to blockchain
  const response = assertion.response as AuthenticatorAssertionResponse;
  const tx = api.tx.Post.create_post_with_webauthn({
    identity_id: identityId,
    passkey_id: Array.from(passkeyId),
    content: Array.from(new TextEncoder().encode(content)),
    authenticator_data: Array.from(new Uint8Array(response.authenticatorData)),
    client_data_json: Array.from(new Uint8Array(response.clientDataJSON)),
    signature: Array.from(new Uint8Array(response.signature)),
    parent_id: null,
  });

  return await tx.signAndSubmit(signer);
}
```

---

## Testing

### Unit Tests

```bash
# Run all tests
pnpm test

# Run with coverage
pnpm test --coverage

# Watch mode
pnpm test --watch
```

### E2E Tests

```bash
# Install Playwright
npx playwright install

# Run E2E tests
pnpm test:e2e
```

---

## Common Issues

### "WebAuthn not supported"

- HTTPS必須（localhostは例外）
- ブラウザを最新版にアップデート

### "No platform authenticator available"

- Touch ID/Face ID/Windows Hello が有効か確認
- セキュリティキーを接続

### "Transaction failed: PasskeyAlreadyRegistered"

- 同じパスキーで二重登録を試みている
- LocalStorageをクリア、または別のパスキーを使用

### "ChallengeMismatch"

- コンテンツハッシュが署名時と異なる
- challenge生成ロジックを確認

---

## Next Steps

1. `pnpm dev` でフロントエンド起動
2. ブラウザで `http://localhost:3000` を開く
3. パスキー登録をテスト
4. 署名付き投稿をテスト

# Research: フロントエンドWebAuthn統合

**Date**: 2026-02-07  
**Feature**: 003-frontend-webauthn  
**Status**: Complete

## Research Summary

このドキュメントは、フロントエンドWebAuthn統合に必要な技術的調査結果をまとめたものです。

---

## 1. WebAuthn API のベストプラクティス（React/Next.js）

### Decision
ブラウザネイティブの WebAuthn API（`navigator.credentials.create()` / `navigator.credentials.get()`）を直接使用する。ライブラリは使用しない。

### Rationale
- WebAuthn Level 2 は主要ブラウザで十分にサポートされている
- SimpleWebAuthn等のライブラリは @polkadot/api と同様に抽象化レイヤーが増える
- COSE公開鍵の直接取得・処理が必要なため、生のAPIが適切
- バンドルサイズの削減

### Alternatives Considered
| ライブラリ | 却下理由 |
|-----------|---------|
| @simplewebauthn/browser | サーバーサイド検証前提の設計、オンチェーン検証に不適 |
| webauthn-json | CBOR解析が不完全、COSE形式の直接取得が困難 |
| fido2-lib | Node.js専用、フロントエンドで使用不可 |

### Implementation Notes
```typescript
// 登録
const credential = await navigator.credentials.create({
  publicKey: {
    challenge: crypto.getRandomValues(new Uint8Array(32)),
    rp: { name: "Anarchy", id: window.location.hostname },
    user: { id: userId, name: userName, displayName: userName },
    pubKeyCredParams: [{ alg: -7, type: "public-key" }], // ES256 (P-256)
    authenticatorSelection: {
      authenticatorAttachment: "platform", // プラットフォーム認証器優先
      userVerification: "required",
      residentKey: "required", // パスキー（discoverable credential）
    },
    attestation: "none", // プライバシー保護
  },
});

// 認証
const assertion = await navigator.credentials.get({
  publicKey: {
    challenge: challengeBytes,
    rpId: window.location.hostname,
    allowCredentials: [], // discoverable credentialなので空
    userVerification: "required",
  },
});
```

---

## 2. WYSIWYS Challenge の生成方法

### Decision
コンテンツのSHA-256ハッシュをBase64URLエンコードしてchallengeに埋め込む。

### Rationale
- Post Pallet の `verify_wysiwys_challenge()` は challenge から content_hash を抽出して検証
- SHA-256 はフロントエンド（Web Crypto API）とバックエンド（post pallet）の両方で利用可能
- Base64URL は WebAuthn challenge の標準形式

### Implementation Notes
```typescript
// コンテンツハッシュの生成
const contentBytes = new TextEncoder().encode(content);
const hashBuffer = await crypto.subtle.digest("SHA-256", contentBytes);
const hashArray = new Uint8Array(hashBuffer);

// challenge形式: prefix(16bytes) + content_hash(32bytes) + suffix(16bytes)
// Note: verify_wysiwys_challengeの期待する形式に合わせる
const challenge = new Uint8Array(64);
crypto.getRandomValues(challenge.subarray(0, 16));  // prefix (ランダム)
challenge.set(hashArray, 16);                        // content_hash
crypto.getRandomValues(challenge.subarray(48, 64)); // suffix (ランダム)
```

バックエンド（webauthn.rs）の検証ロジック:
```rust
pub fn verify_wysiwys_challenge(challenge: &[u8], expected_content_hash: &[u8; 32]) -> bool {
    // challenge[16..48] がコンテンツハッシュ
    challenge.len() >= 48 && challenge[16..48] == *expected_content_hash
}
```

---

## 3. ブラウザ互換性

### Decision
Chrome 67+, Safari 14+, Firefox 60+, Edge 79+ をサポート対象とする。

### Rationale
- これらのバージョンで WebAuthn Level 1/2 が完全サポート
- プラットフォーム認証器（Touch ID, Face ID, Windows Hello）が利用可能
- 市場シェアの98%以上をカバー

### Browser Feature Matrix
| Feature | Chrome | Safari | Firefox | Edge |
|---------|--------|--------|---------|------|
| WebAuthn L2 | 67+ | 14+ | 60+ | 79+ |
| Platform Authenticator | ✅ | ✅ | ✅ | ✅ |
| Discoverable Credentials | 108+ | 16+ | 114+ | 108+ |
| Conditional UI | 108+ | 16+ | 119+ | 108+ |

### Feature Detection
```typescript
const isWebAuthnSupported = (): boolean => {
  return (
    window.PublicKeyCredential !== undefined &&
    typeof window.PublicKeyCredential === "function"
  );
};

const isPlatformAuthenticatorAvailable = async (): Promise<boolean> => {
  if (!isWebAuthnSupported()) return false;
  return await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable();
};
```

---

## 4. テスト戦略

### Decision
Vitest + Testing Library（単体）+ Playwright（E2E）を使用。

### Rationale
- Vitest は Next.js 14 と相性が良く、高速
- Testing Library は React コンポーネントのユーザー中心テストに最適
- Playwright は WebAuthn の E2E テスト（Virtual Authenticator）をサポート
- Jest より Vitest のほうが ESM サポートが優れている

### Test Levels
| レベル | ツール | 対象 |
|-------|-------|-----|
| Unit | Vitest | hooks, utils |
| Component | Vitest + Testing Library | React コンポーネント |
| E2E | Playwright | WebAuthn フロー全体 |

### Mock Strategy
```typescript
// WebAuthn APIのモック（Vitest）
vi.stubGlobal("navigator", {
  credentials: {
    create: vi.fn(),
    get: vi.fn(),
  },
});

// Playwright Virtual Authenticator（E2E）
const cdpSession = await page.context().newCDPSession(page);
await cdpSession.send("WebAuthn.enable");
await cdpSession.send("WebAuthn.addVirtualAuthenticator", {
  options: {
    protocol: "ctap2",
    transport: "internal",
    hasResidentKey: true,
    hasUserVerification: true,
    isUserVerified: true,
  },
});
```

---

## 5. State Management

### Decision
React Context + useReducer で WebAuthn 状態を管理。

### Rationale
- シンプルなフロー（登録 → 認証 → 投稿）には React の組み込み機能で十分
- Redux/Zustand は過剰
- Context により props drilling を回避

### State Structure
```typescript
interface WebAuthnState {
  // 登録状態
  registrationStatus: "idle" | "authenticating" | "submitting" | "success" | "error";
  // 署名状態
  signingStatus: "idle" | "generating" | "authenticating" | "submitting" | "success" | "error";
  // 保存されたクレデンシャル情報
  credentialId: string | null;
  // Identity ID (ブロックチェーン)
  identityId: number | null;
  // エラー情報
  error: Error | null;
}
```

---

## 6. COSE 公開鍵の取得

### Decision
`attestationObject` から CBOR デコードして COSE 公開鍵を直接取得。

### Rationale
- Identity Pallet は COSE 形式の公開鍵を期待
- attestationObject の authData 内に COSE 公開鍵が含まれる
- cbor-x（軽量CBOR実装）を使用

### Implementation Notes
```typescript
import { decode } from "cbor-x";

function extractCosePublicKey(attestationObject: ArrayBuffer): Uint8Array {
  const decoded = decode(new Uint8Array(attestationObject));
  const authData = decoded.authData;
  
  // authData構造:
  // - rpIdHash (32 bytes)
  // - flags (1 byte)
  // - signCount (4 bytes)
  // - attestedCredentialData (variable, if AT flag set)
  //   - aaguid (16 bytes)
  //   - credentialIdLength (2 bytes, big-endian)
  //   - credentialId (credentialIdLength bytes)
  //   - credentialPublicKey (COSE_Key, CBOR encoded)
  
  const flags = authData[32];
  const hasAttestedCredential = (flags & 0x40) !== 0;
  
  if (!hasAttestedCredential) {
    throw new Error("No attested credential data");
  }
  
  const credIdLength = (authData[53] << 8) | authData[54];
  const coseKeyOffset = 55 + credIdLength;
  
  // COSE公開鍵部分を抽出（CBORエンコードされたまま）
  return authData.slice(coseKeyOffset);
}
```

---

## 7. Dependencies

### New Dependencies (to add)
| Package | Version | Purpose |
|---------|---------|---------|
| cbor-x | ^1.5.0 | CBOR encoding/decoding for COSE keys |
| @noble/hashes | ^1.3.0 | SHA-256 (Web Crypto API fallback) |

### Dev Dependencies (to add)
| Package | Version | Purpose |
|---------|---------|---------|
| vitest | ^1.2.0 | Unit/Integration testing |
| @testing-library/react | ^14.0.0 | React component testing |
| @vitejs/plugin-react | ^4.2.0 | Vitest React support |
| playwright | ^1.40.0 | E2E testing |

### Existing Dependencies (reuse)
| Package | Version | Purpose |
|---------|---------|---------|
| polkadot-api | ^1.23.3 | Blockchain interaction |
| react | ^18.2.0 | UI framework |
| next | ^14.1.0 | React framework |

---

## 8. Security Considerations

### rpId (Relying Party ID)
- 本番環境: ドメイン名を使用（例: `anarchy.example.com`）
- 開発環境: `localhost` を使用
- rpId は変更不可（クレデンシャルが無効になる）

### attestation: "none"
- アテステーションは要求しない（プライバシー保護）
- デバイス認証は不要（ユーザー認証のみ）

### User Verification
- `userVerification: "required"` を常に使用
- パスキー使用時は生体認証/PIN が必須

### Challenge Freshness
- 署名時の challenge はその場で生成
- challenge の再利用は不可（リプレイ攻撃防止）

---

## Resolved Questions

| Question | Resolution |
|----------|------------|
| WebAuthn ライブラリを使うか？ | 使わない。ネイティブ API を直接使用 |
| テストフレームワーク | Vitest + Testing Library + Playwright |
| State management | React Context + useReducer |
| COSE公開鍵の取得方法 | cbor-x で attestationObject から抽出 |
| challenge 形式 | prefix(16) + hash(32) + suffix(16) = 64 bytes |

# Data Model: フロントエンドWebAuthn統合

**Date**: 2026-02-07  
**Feature**: 003-frontend-webauthn  
**Status**: Complete

---

## Entities

### 1. WebAuthnCredential

ブラウザの WebAuthn API から取得したクレデンシャル情報。

```typescript
interface WebAuthnCredential {
  /** Base64URL encoded credential ID */
  id: string;
  /** Raw credential ID bytes */
  rawId: Uint8Array;
  /** Credential type (always "public-key") */
  type: "public-key";
  /** Authenticator response */
  response: AuthenticatorResponse;
}

// 登録時のレスポンス
interface AuthenticatorAttestationResponse {
  /** CBOR encoded attestation object containing COSE public key */
  attestationObject: ArrayBuffer;
  /** JSON encoded client data */
  clientDataJSON: ArrayBuffer;
}

// 認証時（署名時）のレスポンス
interface AuthenticatorAssertionResponse {
  /** Authenticator data (rpIdHash + flags + signCount) */
  authenticatorData: ArrayBuffer;
  /** JSON encoded client data with challenge */
  clientDataJSON: ArrayBuffer;
  /** ECDSA signature over authenticatorData + SHA256(clientDataJSON) */
  signature: ArrayBuffer;
  /** User handle (not used) */
  userHandle: ArrayBuffer | null;
}
```

**Relationships**:
- 1 WebAuthnCredential → 1 Passkey (on-chain)
- N WebAuthnCredential → 1 Identity (multiple devices)

---

### 2. PasskeyRegistration

パスキー登録フローの状態管理。

```typescript
interface PasskeyRegistration {
  /** Registration flow status */
  status: RegistrationStatus;
  /** COSE public key bytes (extracted from attestationObject) */
  cosePublicKey: Uint8Array | null;
  /** Passkey ID (Blake2-256 hash of public key) */
  passkeyId: Uint8Array | null;
  /** Device name (optional) */
  deviceName: string | null;
  /** Error information */
  error: RegistrationError | null;
  /** Timestamp of registration attempt */
  startedAt: Date | null;
}

type RegistrationStatus =
  | "idle"           // 初期状態
  | "authenticating" // WebAuthn ダイアログ表示中
  | "extracting"     // COSE公開鍵抽出中
  | "submitting"     // ブロックチェーン送信中
  | "confirming"     // トランザクション確認中
  | "success"        // 完了
  | "error";         // エラー

interface RegistrationError {
  code: RegistrationErrorCode;
  message: string;
  details?: unknown;
}

type RegistrationErrorCode =
  | "WEBAUTHN_NOT_SUPPORTED"     // ブラウザ非対応
  | "USER_CANCELLED"             // ユーザーがキャンセル
  | "AUTHENTICATOR_ERROR"        // 認証器エラー
  | "EXTRACTION_FAILED"          // COSE公開鍵抽出失敗
  | "TRANSACTION_FAILED"         // トランザクション失敗
  | "PASSKEY_ALREADY_REGISTERED" // 既に登録済み
  | "NETWORK_ERROR";             // ネットワークエラー
```

**Validation Rules**:
- `cosePublicKey.length <= 256` (Identity Pallet制約)
- `deviceName.length <= 64` (Identity Pallet制約)

---

### 3. SigningRequest

WebAuthn署名リクエストの状態管理。

```typescript
interface SigningRequest {
  /** Signing flow status */
  status: SigningStatus;
  /** Content to sign */
  content: string;
  /** SHA-256 hash of content */
  contentHash: Uint8Array | null;
  /** WebAuthn challenge (64 bytes: prefix + hash + suffix) */
  challenge: Uint8Array | null;
  /** Authenticator data from assertion */
  authenticatorData: Uint8Array | null;
  /** Client data JSON bytes */
  clientDataJSON: Uint8Array | null;
  /** ECDSA signature */
  signature: Uint8Array | null;
  /** Error information */
  error: SigningError | null;
  /** Timestamp */
  startedAt: Date | null;
}

type SigningStatus =
  | "idle"           // 初期状態
  | "hashing"        // コンテンツハッシュ計算中
  | "authenticating" // WebAuthn ダイアログ表示中
  | "submitting"     // ブロックチェーン送信中
  | "confirming"     // トランザクション確認中
  | "success"        // 完了
  | "error";         // エラー

interface SigningError {
  code: SigningErrorCode;
  message: string;
  details?: unknown;
}

type SigningErrorCode =
  | "WEBAUTHN_NOT_SUPPORTED"
  | "USER_CANCELLED"
  | "AUTHENTICATOR_ERROR"
  | "CREDENTIAL_NOT_FOUND"      // 登録されたクレデンシャルが見つからない
  | "SIGNATURE_INVALID"          // オンチェーン検証失敗
  | "CHALLENGE_MISMATCH"         // WYSIWYS検証失敗
  | "INSUFFICIENT_BALANCE"       // Moral残高不足
  | "CONTENT_TOO_LONG"           // コンテンツ長超過
  | "TRANSACTION_FAILED"
  | "NETWORK_ERROR";
```

**Validation Rules**:
- `content.length <= 10000` (Post Pallet MaxContentLength)
- `challenge.length === 64`
- `contentHash.length === 32`

---

### 4. IdentityState

ブロックチェーン上のIdentity情報のフロントエンド表現。

```typescript
interface IdentityState {
  /** Identity ID on-chain */
  identityId: number | null;
  /** Passkeys associated with this identity */
  passkeys: PasskeyInfo[];
  /** Is identity loaded? */
  isLoaded: boolean;
  /** Loading error */
  error: Error | null;
}

interface PasskeyInfo {
  /** Passkey ID (32 bytes Blake2-256 hash) */
  id: Uint8Array;
  /** Device name if set */
  deviceName: string | null;
  /** Registration timestamp (block number) */
  registeredAt: number;
  /** Is this the current device's passkey? */
  isCurrentDevice: boolean;
}
```

**State Transitions**:
```
null → identityId (after registration)
passkeys[] → passkeys + new (after add_passkey)
passkeys → passkeys - removed (after remove_passkey)
```

---

### 5. WebAuthnContext (React Context)

アプリケーション全体で共有するWebAuthn状態。

```typescript
interface WebAuthnContextValue {
  // State
  identity: IdentityState;
  registration: PasskeyRegistration;
  signing: SigningRequest;
  
  // Feature detection
  isSupported: boolean;
  hasPlatformAuthenticator: boolean;
  
  // Actions
  checkSupport: () => Promise<void>;
  registerPasskey: (deviceName?: string) => Promise<void>;
  signContent: (content: string, identityId: number, passkeyId: Uint8Array) => Promise<void>;
  addPasskey: (identityId: number, deviceName?: string) => Promise<void>;
  loadIdentity: (identityId: number) => Promise<void>;
  reset: () => void;
}
```

---

## Storage

### LocalStorage (Browser)

```typescript
interface LocalWebAuthnData {
  /** Last used Identity ID */
  lastIdentityId: number | null;
  /** Credential IDs for quick discovery (Base64URL) */
  credentialIds: string[];
  /** Device preferences */
  preferences: {
    preferredAuthenticator: "platform" | "cross-platform" | "any";
  };
}
```

**Key**: `anarchy:webauthn`

**Note**: 秘密情報は一切保存しない。公開情報のみ。

---

## On-Chain Data (Reference)

Identity Palletのストレージ構造（参照用）:

```rust
// Identity storage
Identities: Map<u64, Identity>

struct Identity {
    id: u64,
    passkeys: BoundedVec<Passkey, MaxPasskeysPerIdentity>,
    created_at: BlockNumber,
}

struct Passkey {
    id: [u8; 32],        // Blake2-256(public_key)
    public_key: Vec<u8>, // COSE format
    device_name: Option<Vec<u8>>,
    added_at: BlockNumber,
}
```

---

## Entity Relationships Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        Browser (Client)                          │
├─────────────────────────────────────────────────────────────────┤
│  WebAuthnContext                                                 │
│  ├── IdentityState ─────────────────────┐                       │
│  │     ├── identityId (u64)             │                       │
│  │     └── passkeys[] ──────────────────┼─ mirrors on-chain     │
│  │                                       │                       │
│  ├── PasskeyRegistration                │                       │
│  │     ├── status                        │                       │
│  │     ├── cosePublicKey ───────────────┼─► register_identity   │
│  │     └── passkeyId                     │                       │
│  │                                       │                       │
│  └── SigningRequest                     │                       │
│        ├── content ─────────────────────┼─► WYSIWYS challenge   │
│        ├── authenticatorData ───────────┼─► extrinsic param     │
│        ├── clientDataJSON ──────────────┼─► extrinsic param     │
│        └── signature ───────────────────┼─► extrinsic param     │
│                                          │                       │
│  LocalStorage                            │                       │
│  └── lastIdentityId, credentialIds      │                       │
└─────────────────────────────────────────┼───────────────────────┘
                                           │
                                           ▼ PAPI
┌─────────────────────────────────────────────────────────────────┐
│                      Blockchain (On-Chain)                       │
├─────────────────────────────────────────────────────────────────┤
│  Identity Pallet                                                 │
│  └── Identities: Map<u64, Identity>                             │
│        └── passkeys: Vec<Passkey>                               │
│              ├── id: [u8; 32]                                   │
│              └── public_key: Vec<u8> (COSE)                     │
│                                                                  │
│  Post Pallet                                                     │
│  └── create_post_with_webauthn(                                 │
│        identity_id, passkey_id, content,                        │
│        authenticator_data, client_data_json, signature          │
│      )                                                           │
└─────────────────────────────────────────────────────────────────┘
```

# API Contract: WebAuthn Hooks

**Date**: 2026-02-07  
**Feature**: 003-frontend-webauthn  
**Type**: React Hooks

---

## useWebAuthn Hook

WebAuthn機能全体を管理するメインフック。

### Interface

```typescript
interface UseWebAuthnOptions {
  /** PAPI client API */
  api: any;
  /** Signer for transactions (temporary, until gasless) */
  signer: PolkadotSigner | null;
}

interface UseWebAuthnResult {
  // Feature Detection
  isSupported: boolean;
  hasPlatformAuthenticator: boolean | null;
  
  // Identity State
  identity: IdentityState | null;
  
  // Registration
  registrationStatus: RegistrationStatus;
  registerPasskey: (deviceName?: string) => Promise<RegisterResult>;
  
  // Signing
  signingStatus: SigningStatus;
  signAndPost: (content: string, parentId?: number) => Promise<PostResult>;
  
  // Multi-device
  addPasskey: (deviceName?: string) => Promise<AddPasskeyResult>;
  
  // Utilities
  loadIdentityById: (identityId: number) => Promise<void>;
  reset: () => void;
  error: Error | null;
}
```

### Usage Example

```typescript
function PostWithPasskey() {
  const { api, signer } = useApi();
  const { 
    isSupported,
    identity,
    registrationStatus,
    registerPasskey,
    signAndPost,
    error 
  } = useWebAuthn({ api, signer });

  if (!isSupported) {
    return <div>WebAuthn非対応ブラウザです</div>;
  }

  if (!identity) {
    return (
      <button 
        onClick={() => registerPasskey("MacBook")}
        disabled={registrationStatus !== "idle"}
      >
        パスキーで登録
      </button>
    );
  }

  const handlePost = async (content: string) => {
    const result = await signAndPost(content);
    if (result.success) {
      console.log("Posted:", result.postId);
    }
  };

  return <PostForm onSubmit={handlePost} />;
}
```

---

## useWebAuthnRegistration Hook

パスキー登録フローに特化したフック。

### Interface

```typescript
interface UseWebAuthnRegistrationOptions {
  api: any;
  signer: PolkadotSigner | null;
  onSuccess?: (result: RegisterResult) => void;
  onError?: (error: RegistrationError) => void;
}

interface UseWebAuthnRegistrationResult {
  status: RegistrationStatus;
  register: (deviceName?: string) => Promise<RegisterResult>;
  reset: () => void;
  error: RegistrationError | null;
}

interface RegisterResult {
  success: boolean;
  identityId?: number;
  passkeyId?: Uint8Array;
  error?: RegistrationError;
}
```

### Flow

```
1. register() called
   └── status: "idle" → "authenticating"

2. navigator.credentials.create() called
   └── Browser shows WebAuthn dialog
   
3. User completes authentication
   └── status: "authenticating" → "extracting"

4. Extract COSE public key from attestationObject
   └── status: "extracting" → "submitting"

5. Call api.tx.Identity.register_identity()
   └── status: "submitting" → "confirming"

6. Transaction finalized
   └── status: "confirming" → "success"
   
Error at any step:
   └── status: → "error"
```

---

## useWebAuthnSigning Hook

WebAuthn署名付き投稿に特化したフック。

### Interface

```typescript
interface UseWebAuthnSigningOptions {
  api: any;
  signer: PolkadotSigner | null;
  identityId: number;
  passkeyId: Uint8Array;
  onSuccess?: (result: PostResult) => void;
  onError?: (error: SigningError) => void;
}

interface UseWebAuthnSigningResult {
  status: SigningStatus;
  sign: (content: string, parentId?: number) => Promise<PostResult>;
  estimateCost: (content: string) => number;
  reset: () => void;
  error: SigningError | null;
}

interface PostResult {
  success: boolean;
  postId?: number;
  txHash?: string;
  moralSpent?: bigint;
  error?: SigningError;
}
```

### Flow

```
1. sign(content) called
   └── status: "idle" → "hashing"

2. Calculate SHA-256(content)
   └── Generate challenge = prefix(16) + hash(32) + suffix(16)
   └── status: "hashing" → "authenticating"

3. navigator.credentials.get({ challenge }) called
   └── Browser shows WebAuthn dialog

4. User completes authentication
   └── status: "authenticating" → "submitting"

5. Call api.tx.Post.create_post_with_webauthn()
   └── Parameters: identity_id, passkey_id, content,
       authenticatorData, clientDataJSON, signature
   └── status: "submitting" → "confirming"

6. Transaction finalized
   └── status: "confirming" → "success"
```

---

## useWebAuthnSupport Hook

WebAuthn機能検出に特化した軽量フック。

### Interface

```typescript
interface UseWebAuthnSupportResult {
  /** WebAuthn API available */
  isSupported: boolean;
  /** Platform authenticator available (Touch ID, Face ID, Windows Hello) */
  hasPlatformAuthenticator: boolean | null;
  /** Conditional UI available (autofill) */
  hasConditionalUI: boolean | null;
  /** Check performed */
  isChecked: boolean;
  /** Check in progress */
  isChecking: boolean;
  /** Recheck */
  recheck: () => Promise<void>;
}
```

### Usage

```typescript
function WebAuthnGate({ children }: { children: ReactNode }) {
  const { isSupported, hasPlatformAuthenticator, isChecking } = useWebAuthnSupport();

  if (isChecking) {
    return <Loading />;
  }

  if (!isSupported) {
    return <UnsupportedBrowserMessage />;
  }

  if (!hasPlatformAuthenticator) {
    return <NoAuthenticatorMessage />;
  }

  return children;
}
```

---

## Utility Functions

### extractCosePublicKey

```typescript
/**
 * Extract COSE public key from attestation object
 * @param attestationObject - Raw attestation object from WebAuthn
 * @returns COSE encoded public key bytes
 * @throws If attestation object is invalid or no credential data
 */
function extractCosePublicKey(attestationObject: ArrayBuffer): Uint8Array;
```

### generateWysiwysChallenge

```typescript
/**
 * Generate WYSIWYS challenge for signing
 * @param content - Content string to sign
 * @returns 64-byte challenge with embedded content hash
 */
async function generateWysiwysChallenge(content: string): Promise<Uint8Array>;
```

### derivePasskeyId

```typescript
/**
 * Derive passkey ID from COSE public key (matches Identity Pallet)
 * @param cosePublicKey - COSE encoded public key
 * @returns 32-byte Blake2-256 hash
 */
function derivePasskeyId(cosePublicKey: Uint8Array): Uint8Array;
```

### base64UrlEncode / base64UrlDecode

```typescript
/**
 * Base64URL encode/decode for WebAuthn data
 */
function base64UrlEncode(data: Uint8Array): string;
function base64UrlDecode(str: string): Uint8Array;
```

---

## Error Codes

| Code | Description | User Message |
|------|-------------|--------------|
| `WEBAUTHN_NOT_SUPPORTED` | Browser doesn't support WebAuthn | このブラウザはパスキーに対応していません |
| `USER_CANCELLED` | User cancelled the operation | 操作がキャンセルされました |
| `AUTHENTICATOR_ERROR` | Authenticator returned error | 認証に失敗しました |
| `CREDENTIAL_NOT_FOUND` | No matching credential | パスキーが見つかりません |
| `EXTRACTION_FAILED` | Failed to extract public key | 公開鍵の取得に失敗しました |
| `TRANSACTION_FAILED` | Blockchain transaction failed | トランザクションが失敗しました |
| `PASSKEY_ALREADY_REGISTERED` | Public key already registered | このパスキーは既に登録されています |
| `SIGNATURE_INVALID` | On-chain signature verification failed | 署名の検証に失敗しました |
| `CHALLENGE_MISMATCH` | WYSIWYS challenge doesn't match | 署名内容が一致しません |
| `INSUFFICIENT_BALANCE` | Not enough $moral | $moral残高が不足しています |
| `CONTENT_TOO_LONG` | Content exceeds limit | 投稿内容が長すぎます |
| `NETWORK_ERROR` | Network connectivity issue | ネットワークエラーが発生しました |

---

## Events (for analytics/debugging)

```typescript
type WebAuthnEvent =
  | { type: "registration_started"; deviceName?: string }
  | { type: "registration_authenticating" }
  | { type: "registration_submitting"; txHash: string }
  | { type: "registration_success"; identityId: number; passkeyId: string }
  | { type: "registration_error"; code: RegistrationErrorCode }
  | { type: "signing_started"; contentLength: number }
  | { type: "signing_authenticating" }
  | { type: "signing_submitting"; txHash: string }
  | { type: "signing_success"; postId: number; moralSpent: string }
  | { type: "signing_error"; code: SigningErrorCode };

// Optional event handler
interface UseWebAuthnOptions {
  onEvent?: (event: WebAuthnEvent) => void;
}
```

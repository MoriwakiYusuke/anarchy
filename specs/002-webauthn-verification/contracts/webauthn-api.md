# WebAuthn API Contract

## Substrate Pallet Extrinsics

### Identity Pallet（拡張）

#### `register_identity_with_webauthn`

WebAuthn公開鍵を使用して新規Identityを登録する。

```rust
#[pallet::call_index(3)]
#[pallet::weight(T::WeightInfo::register_identity_with_webauthn())]
pub fn register_identity_with_webauthn(
    origin: OriginFor<T>,
    /// COSE形式のWebAuthn公開鍵
    cose_public_key: Vec<u8>,
    /// authenticatorData（attestation用）
    authenticator_data: Vec<u8>,
    /// clientDataJSON
    client_data_json: Vec<u8>,
    /// デバイス名（オプション）
    device_name: Option<Vec<u8>>,
) -> DispatchResult;
```

**Events**:
- `IdentityCreatedWithWebAuthn { identity_id: u64, passkey_id: PasskeyId }`

**Errors**:
- `InvalidCoseKey` - COSE公開鍵のパースに失敗
- `UnsupportedAlgorithm` - ES256以外のアルゴリズム
- `InvalidPublicKey` - 公開鍵が曲線上の有効な点でない
- `PasskeyAlreadyRegistered` - 公開鍵が既に登録済み

---

### Post Pallet（拡張）

#### `create_post_with_webauthn`

WebAuthn署名付きで投稿を作成する。WYSIWYS（What You See Is What You Sign）を保証。

```rust
#[pallet::call_index(1)]
#[pallet::weight(T::WeightInfo::create_post_with_webauthn())]
pub fn create_post_with_webauthn(
    origin: OriginFor<T>,
    /// 投稿者のIdentity ID
    identity_id: u64,
    /// 使用するPasskeyのID
    passkey_id: [u8; 32],
    /// 投稿コンテンツ
    content: Vec<u8>,
    /// WebAuthn署名データ
    webauthn_signature: WebAuthnSignatureData,
    /// 親投稿ID（リプライの場合）
    parent_id: Option<u64>,
) -> DispatchResult;
```

**署名データ構造**:
```rust
pub struct WebAuthnSignatureData {
    /// authenticatorData（生バイト列）
    pub authenticator_data: Vec<u8>,
    /// clientDataJSON（UTF-8文字列）
    pub client_data_json: Vec<u8>,
    /// ECDSA署名（DER形式またはraw形式）
    pub signature: Vec<u8>,
}
```

**Events**:
- `PostCreatedWithWebAuthn { post_id: u64, identity_id: u64, content_hash: [u8; 32] }`

**Errors**:
- `IdentityNotFound` - 指定されたIdentityが存在しない
- `PasskeyNotFound` - 指定されたPasskeyがIdentityに紐付いていない
- `InvalidSignature` - 署名検証に失敗
- `ChallengeMismatch` - challengeが投稿ハッシュと一致しない
- `RpIdMismatch` - rpIdHashが期待値と一致しない
- `UserNotPresent` - userPresentフラグが立っていない
- `InvalidClientDataType` - clientDataのtypeが"webauthn.get"でない
- `InsufficientMoral` - $moral残高不足

---

## Internal Module APIs

### webauthn.rs

```rust
/// WebAuthn署名を検証する
pub fn verify_signature(
    public_key: &WebAuthnPublicKey,
    authenticator_data: &[u8],
    client_data_json: &[u8],
    signature: &[u8],
) -> Result<(), WebAuthnError>;

/// authenticatorDataをパースする
pub fn parse_authenticator_data(
    data: &[u8],
) -> Result<AuthenticatorData, WebAuthnError>;

/// clientDataJSONをパースする
pub fn parse_client_data_json(
    json: &[u8],
) -> Result<ClientData, WebAuthnError>;

/// 署名フォーマットを検出してraw形式に変換する
pub fn normalize_signature(
    signature: &[u8],
) -> Result<[u8; 64], WebAuthnError>;

/// challengeがコンテンツハッシュを含むか検証する（WYSIWYS）
pub fn verify_wysiwys_challenge(
    challenge: &[u8],
    content_hash: &[u8; 32],
) -> Result<(), WebAuthnError>;
```

### cose.rs

```rust
/// COSE公開鍵をパースしてP-256公開鍵を抽出する
pub fn parse_cose_key(
    cose_bytes: &[u8],
) -> Result<WebAuthnPublicKey, CoseError>;

/// 公開鍵がP-256曲線上の有効な点かを検証する
pub fn validate_public_key(
    public_key: &WebAuthnPublicKey,
) -> Result<(), CoseError>;
```

---

## Error Types

```rust
pub enum WebAuthnError {
    /// authenticatorDataが短すぎる
    AuthenticatorDataTooShort,
    /// rpIdHashが一致しない
    RpIdHashMismatch,
    /// userPresentフラグが立っていない
    UserNotPresent,
    /// clientDataJSONのパースに失敗
    InvalidClientDataJson,
    /// clientDataのtypeが不正
    InvalidClientDataType,
    /// challengeが一致しない
    ChallengeMismatch,
    /// 署名フォーマットが不正
    InvalidSignatureFormat,
    /// 署名検証に失敗
    SignatureVerificationFailed,
    /// WYSIWYSチャレンジが不正
    InvalidWysiywsChallenge,
}

pub enum CoseError {
    /// COSE公開鍵のパースに失敗
    InvalidCoseFormat,
    /// サポートされていないキータイプ
    UnsupportedKeyType,
    /// サポートされていないアルゴリズム
    UnsupportedAlgorithm,
    /// サポートされていない曲線
    UnsupportedCurve,
    /// X座標が不正
    InvalidXCoordinate,
    /// Y座標が不正
    InvalidYCoordinate,
    /// 公開鍵が曲線上にない
    PointNotOnCurve,
}
```

---

## Challenge Format (WYSIWYS)

投稿時のchallengeは以下の形式を使用:

```
challenge = base64url(PREFIX || content_hash || timestamp)

PREFIX = "anarchy:post:" (13 bytes)
content_hash = SHA256(content) (32 bytes)
timestamp = unix_timestamp (8 bytes, big-endian)
```

フロントエンドはこのchallengeを生成し、WebAuthn APIの`publicKey.challenge`に渡す。
バックエンド（オンチェーン）は署名検証時に、challengeから`content_hash`を抽出し、実際の投稿内容のハッシュと照合する。

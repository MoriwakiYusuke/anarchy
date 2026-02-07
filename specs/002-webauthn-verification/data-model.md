# Data Model: WebAuthn署名検証

## Entities

### 1. WebAuthnPublicKey

WebAuthnから返されるCOSE形式の公開鍵から抽出されたP-256公開鍵データ。

```rust
/// P-256公開鍵（65バイト: 0x04 || x || y）
pub struct WebAuthnPublicKey {
    /// X座標（32バイト）
    pub x: [u8; 32],
    /// Y座標（32バイト）
    pub y: [u8; 32],
}
```

**制約**:
- `x`と`y`はP-256曲線上の有効な点でなければならない
- COSE形式から抽出時に検証される

### 2. WebAuthnSignature

WebAuthn認証で生成されるECDSA署名データ。

```rust
/// ECDSA署名（64バイト: r || s）
pub struct WebAuthnSignature {
    /// r値（32バイト）
    pub r: [u8; 32],
    /// s値（32バイト）
    pub s: [u8; 32],
}
```

**制約**:
- DER形式またはraw形式で受け取り、内部でraw形式に正規化
- r, s は P-256曲線の位数より小さい正の整数

### 3. AuthenticatorData

WebAuthn認証応答に含まれるauthenticatorData構造体。

```rust
/// authenticatorData（最小37バイト）
pub struct AuthenticatorData {
    /// rpIdHash（32バイト）- SHA-256(rpId)
    pub rp_id_hash: [u8; 32],
    /// フラグ（1バイト）
    pub flags: AuthenticatorFlags,
    /// 署名カウンタ（4バイト、big-endian）
    pub sign_count: u32,
    /// 拡張データ（オプション）
    pub extensions: Option<Vec<u8>>,
}

/// 認証フラグ
pub struct AuthenticatorFlags {
    /// User Present (bit 0)
    pub user_present: bool,
    /// User Verified (bit 2)
    pub user_verified: bool,
    /// Attested credential data included (bit 6)
    pub attested_credential_data: bool,
    /// Extension data included (bit 7)
    pub extension_data: bool,
}
```

**制約**:
- `user_present`フラグは必須（trueでなければエラー）
- `rp_id_hash`は設定されたrpIdのSHA-256と一致すること

### 4. ClientDataJSON

WebAuthn認証応答に含まれるclientDataJSON構造体。

```rust
/// clientDataJSON（パース済み）
pub struct ClientData {
    /// type: "webauthn.get"（認証）or "webauthn.create"（登録）
    pub type_: OperationType,
    /// challenge: base64urlエンコードされたチャレンジ
    pub challenge: Vec<u8>,
    /// origin: リクエスト元のオリジン
    pub origin: Vec<u8>,
    /// crossOrigin: クロスオリジンフラグ（オプション）
    pub cross_origin: Option<bool>,
}

pub enum OperationType {
    WebAuthnGet,    // "webauthn.get"
    WebAuthnCreate, // "webauthn.create"
}
```

**制約**:
- 署名検証時は`type_ == WebAuthnGet`であること
- `challenge`はデコード後、期待されるチャレンジと一致すること

### 5. WebAuthnCredential（既存Identity Palletの拡張）

```rust
/// 拡張されたPasskey構造体
pub struct Passkey<MaxPublicKeyLength, MaxDeviceNameLength> {
    pub id: PasskeyId,
    /// COSE公開鍵を格納（x, y座標が抽出可能）
    pub public_key: BoundedVec<u8, MaxPublicKeyLength>,
    pub registered_at: u64,
    pub last_used_at: u64,
    pub device_name: Option<BoundedVec<u8, MaxDeviceNameLength>>,
}
```

## Relationships

```
Identity
   |
   +-- 1:N ---> Passkey (WebAuthnPublicKey)
                   |
                   +-- verifies --> WebAuthnSignature
                                          |
                                          +-- contains --> AuthenticatorData
                                          +-- contains --> ClientDataJSON
```

## Validation Rules

### 公開鍵登録時
1. COSE形式のバリデーション
2. kty=2（EC2）、alg=-7（ES256）、crv=1（P-256）の検証
3. x, y座標が32バイトであることの確認
4. 公開鍵が曲線上の有効な点であることの検証

### 署名検証時
1. authenticatorDataのパース（最小37バイト）
2. rpIdHashの一致確認
3. userPresentフラグの確認
4. clientDataJSONのパース
5. typeが"webauthn.get"であることの確認
6. challengeの一致確認（投稿ハッシュを含む）
7. 署名の検証: `ECDSA.verify(publicKey, SHA256(authData || SHA256(clientDataJSON)), signature)`

## Storage Schema

### 既存（変更なし）
- `Identities<T>`: Identity ID → Identity
- `PasskeyOwner<T>`: PasskeyId → Identity ID

### 新規追加（オプション）
- `RpIdHash`: 設定されたrpIdのSHA-256ハッシュ（Config定数として保持可能）

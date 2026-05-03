# API Contract: Identity Pallet

**Date**: 2026-02-07  
**Feature**: 001-identity-pallet  
**Pallet Name**: `pallet-identity`

---

## Extrinsics

### register_identity

新規Identityを作成し、最初のPasskeyを登録する。

| Parameter | Type | Description |
|-----------|------|-------------|
| `public_key` | `Vec<u8>` | COSE形式のWebAuthn公開鍵 |
| `device_name` | `Option<Vec<u8>>` | デバイス名（オプション、最大64バイト） |

**Returns**: `DispatchResult`

**Events**:
- `IdentityCreated { identity_id, passkey_id }`

**Errors**:
- `EmptyPublicKey`: 公開鍵が空
- `PublicKeyTooLong`: 公開鍵が256バイトを超過
- `PasskeyAlreadyRegistered`: 公開鍵が既に別のIdentityで登録済み

**Example**:
```rust
// フロントエンドから（PAPI経由）
const tx = api.tx.Identity.register_identity({
    public_key: cosePublicKeyBytes,
    device_name: "MacBook Pro"
});
await tx.signAndSubmit(signer);
```

---

### add_passkey

既存のIdentityに新しいPasskeyを追加する。

| Parameter | Type | Description |
|-----------|------|-------------|
| `identity_id` | `u64` | 追加先のIdentity ID |
| `public_key` | `Vec<u8>` | 新しいCOSE形式WebAuthn公開鍵 |
| `device_name` | `Option<Vec<u8>>` | デバイス名（オプション） |

**Returns**: `DispatchResult`

**Events**:
- `PasskeyAdded { identity_id, passkey_id }`

**Errors**:
- `IdentityNotFound`: 指定されたIdentityが存在しない
- `Unauthorized`: 認証失敗（将来のWebAuthn検証）
- `EmptyPublicKey`: 公開鍵が空
- `PublicKeyTooLong`: 公開鍵が256バイトを超過
- `PasskeyAlreadyRegistered`: 公開鍵が既に登録済み
- `TooManyPasskeys`: Passkey数が上限（10）に達している

**Note**: 現時点では `origin` が既存Passkeyで署名されていることを前提とする。WebAuthn検証（Phase 1.4）実装後に完全な認証を追加。

---

### remove_passkey

IdentityからPasskeyを削除する。

| Parameter | Type | Description |
|-----------|------|-------------|
| `identity_id` | `u64` | 対象のIdentity ID |
| `passkey_id` | `[u8; 32]` | 削除するPasskeyのID |

**Returns**: `DispatchResult`

**Events**:
- `PasskeyRemoved { identity_id, passkey_id }`

**Errors**:
- `IdentityNotFound`: 指定されたIdentityが存在しない
- `Unauthorized`: 認証失敗
- `PasskeyNotFound`: 指定されたPasskeyがIdentityに存在しない
- `CannotRemoveLastPasskey`: 最後のPasskeyは削除できない

---

## Storage Queries

### get_identity

Identity情報を取得する。

```typescript
// PAPI経由
const identity = await api.query.Identity.Identities.getValue(identityId);
// Returns: Identity | undefined
```

### get_identity_by_passkey

PasskeyIdからIdentityを逆引きする。

```typescript
// PAPI経由
const identityId = await api.query.Identity.PasskeyOwner.getValue(passkeyId);
const identity = identityId 
    ? await api.query.Identity.Identities.getValue(identityId)
    : undefined;
```

### get_all_identities

全Identityを取得する。

```typescript
// PAPI経由
const entries = await api.query.Identity.Identities.getEntries();
// Returns: Array<[identityId, Identity]>
```

---

## Runtime Constants

```rust
// Config で定義
pub const MaxPasskeys: u32 = 10;
pub const MaxPublicKeyLength: u32 = 256;
pub const MaxDeviceNameLength: u32 = 64;
```

```typescript
// PAPI経由でアクセス
const maxPasskeys = await api.constants.Identity.MaxPasskeys();
```

---

## Integration with Existing Pallets

### Post Pallet連携（将来）

現在の Post Pallet は `AccountId` で投稿者を識別している。Identity Pallet導入後:

```rust
// 現在
pub struct Post<T: Config> {
    pub author: T::AccountId,  // Substrate AccountId
    ...
}

// 将来（Phase 1.4以降で検討）
pub struct Post<T: Config> {
    pub author_identity: u64,  // Identity ID
    ...
}
```

### Moral Pallet連携

Moral残高は AccountId に紐付いている。Identity → AccountId のマッピングは本スコープ外（アカウント抽象化で対応予定）。

---

## Security Considerations

1. **公開鍵の検証**: 形式検証のみ（長さ、非空）。COSEフォーマット検証はPhase 1.4で追加。

2. **認証**: 現時点では `origin` の署名のみで認証。WebAuthn署名検証（challenge埋め込み）はPhase 1.4で追加。

3. **重複防止**: PasskeyOwner ストレージで公開鍵のグローバル一意性を保証。

4. **DoS対策**: BoundedVec による上限設定（最大10 Passkeys/Identity、最大256バイト/公開鍵）。

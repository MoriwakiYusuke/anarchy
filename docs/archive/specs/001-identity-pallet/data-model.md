# Data Model: Identity Pallet

**Date**: 2026-02-07  
**Feature**: 001-identity-pallet

---

## Entities

### Identity

ユーザーを一意に識別するエンティティ。

| Field | Type | Description |
|-------|------|-------------|
| `id` | `u64` | 一意識別子（シーケンシャル発行） |
| `created_at` | `BlockNumber` | 作成時のブロック番号 |
| `passkeys` | `BoundedVec<Passkey, 10>` | 紐付けられたパスキー一覧 |

```rust
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Identity<T: Config> {
    pub created_at: BlockNumberFor<T>,
    pub passkeys: BoundedVec<Passkey<T>, T::MaxPasskeys>,
}
```

### Passkey

WebAuthn公開鍵情報。

| Field | Type | Description |
|-------|------|-------------|
| `id` | `[u8; 32]` | PasskeyId（公開鍵のBlake2b-256ハッシュ） |
| `public_key` | `BoundedVec<u8, 256>` | COSE形式の公開鍵データ |
| `registered_at` | `BlockNumber` | 登録時のブロック番号 |
| `last_used_at` | `BlockNumber` | 最終使用時のブロック番号 |
| `device_name` | `Option<BoundedVec<u8, 64>>` | デバイス名（オプション） |

```rust
pub type PasskeyId = [u8; 32];

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Passkey<T: Config> {
    pub id: PasskeyId,
    pub public_key: BoundedVec<u8, T::MaxPublicKeyLength>,
    pub registered_at: BlockNumberFor<T>,
    pub last_used_at: BlockNumberFor<T>,
    pub device_name: Option<BoundedVec<u8, T::MaxDeviceNameLength>>,
}
```

---

## Storage

### Identities

Identity ID から Identity データへのマッピング。

```rust
#[pallet::storage]
pub type Identities<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    u64,                    // Identity ID
    Identity<T>,            // Identity data
    OptionQuery,
>;
```

### NextIdentityId

次に発行する Identity ID。

```rust
#[pallet::storage]
pub type NextIdentityId<T: Config> = StorageValue<_, u64, ValueQuery>;
```

### PasskeyOwner

PasskeyId から Identity ID への逆引き。重複登録防止に使用。

```rust
#[pallet::storage]
pub type PasskeyOwner<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    PasskeyId,              // Passkey ID (hash of public key)
    u64,                    // Identity ID
    OptionQuery,
>;
```

---

## Configuration Traits

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    /// イベント型
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

    /// 1つのIdentityに紐付けられる最大Passkey数
    #[pallet::constant]
    type MaxPasskeys: Get<u32>;

    /// 公開鍵の最大バイト長
    #[pallet::constant]
    type MaxPublicKeyLength: Get<u32>;

    /// デバイス名の最大バイト長
    #[pallet::constant]
    type MaxDeviceNameLength: Get<u32>;
}
```

**推奨デフォルト値**:
- `MaxPasskeys`: 10
- `MaxPublicKeyLength`: 256
- `MaxDeviceNameLength`: 64

---

## Events

```rust
#[pallet::event]
#[pallet::generate_deposit(pub(super) fn deposit_event)]
pub enum Event<T: Config> {
    /// Identity が作成された
    IdentityCreated {
        identity_id: u64,
        passkey_id: PasskeyId,
    },

    /// Passkey が追加された
    PasskeyAdded {
        identity_id: u64,
        passkey_id: PasskeyId,
    },

    /// Passkey が削除された
    PasskeyRemoved {
        identity_id: u64,
        passkey_id: PasskeyId,
    },
}
```

---

## Errors

```rust
#[pallet::error]
pub enum Error<T> {
    /// Identity が存在しない
    IdentityNotFound,

    /// Passkey が既に登録されている（別のIdentityで使用中）
    PasskeyAlreadyRegistered,

    /// Passkey が見つからない
    PasskeyNotFound,

    /// Passkey の最大数に達した
    TooManyPasskeys,

    /// 最後の Passkey は削除できない
    CannotRemoveLastPasskey,

    /// 公開鍵が空
    EmptyPublicKey,

    /// 公開鍵が長すぎる
    PublicKeyTooLong,

    /// 認証されていない（将来のWebAuthn検証用）
    Unauthorized,
}
```

---

## Relationships

```
┌─────────────────────────────────────────────────────────────┐
│                        Storage                               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   NextIdentityId ──────► [u64]                              │
│                                                              │
│   Identities ──────────► Identity ID ──► Identity           │
│                                           │                  │
│                                           └─► passkeys[]     │
│                                               │               │
│   PasskeyOwner ────────► PasskeyId ──────────┘               │
│                          (reverse lookup)                    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**制約**:
- 1つの公開鍵（PasskeyId）は1つのIdentityにのみ紐付く
- 1つのIdentityは最大10個のPasskeyを持てる
- Identityには最低1つのPasskeyが必要（最後の1つは削除不可）

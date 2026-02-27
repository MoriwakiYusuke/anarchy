# Contract: Nickname Pallet

**Version**: 1.0.0  
**Date**: 2026-02-25  
**Type**: Substrate Pallet

## Overview

軽量なニックネーム管理パレット。ユーザーが自身のAccountIdに表示名を設定・変更・削除できる。

## Storage

### Nicknames

AccountIdからニックネームへのマッピング

```rust
StorageMap<AccountId, Option<BoundedVec<u8, 128>>>
```

| Key | Value | Description |
|-----|-------|-------------|
| `AccountId` | `Option<BoundedVec<u8, 128>>` | ニックネーム（UTF-8、最大128バイト） |

## Extrinsics

### set_nickname

ニックネームを設定または変更する

```rust
#[pallet::call_index(0)]
#[pallet::weight(T::DbWeight::get().reads_writes(0, 1))]
pub fn set_nickname(
    origin: OriginFor<T>,
    nickname: Vec<u8>,
) -> DispatchResult
```

**Parameters**:
| Name | Type | Description | Validation |
|------|------|-------------|------------|
| `origin` | `OriginFor<T>` | 署名済みオリジン | 必須 |
| `nickname` | `Vec<u8>` | ニックネーム | 1-128バイト、UTF-8 |

**Errors**:
| Error | Description |
|-------|-------------|
| `NicknameTooLong` | 128バイトを超過 |
| `InvalidUtf8` | 無効なUTF-8バイト列 |

**Events**:
```rust
NicknameSet { who: AccountId, nickname: Vec<u8> }
```

**Frontend Usage (PAPI)**:
```typescript
const api = client.getUnsafeApi()
const tx = api.tx.Nickname.set_nickname({
  nickname: new TextEncoder().encode("alice_anarchy")
})
await tx.signSubmitAndWatch(signer)
```

---

### clear_nickname

ニックネームを削除する

```rust
#[pallet::call_index(1)]
#[pallet::weight(T::DbWeight::get().reads_writes(0, 1))]
pub fn clear_nickname(origin: OriginFor<T>) -> DispatchResult
```

**Parameters**:
| Name | Type | Description |
|------|------|-------------|
| `origin` | `OriginFor<T>` | 署名済みオリジン |

**Events**:
```rust
NicknameCleared { who: AccountId }
```

**Frontend Usage (PAPI)**:
```typescript
const api = client.getUnsafeApi()
const tx = api.tx.Nickname.clear_nickname()
await tx.signSubmitAndWatch(signer)
```

---

## Runtime Query

### Query Nickname

```typescript
// Single account
const nickname = await api.query.Nickname.nicknames(accountId)
if (nickname) {
  const name = new TextDecoder().decode(nickname)
  console.log(name)  // "alice_anarchy"
}

// Multiple accounts (batch)
const accountIds = [alice, bob, charlie]
const nicknames = await api.query.Nickname.nicknames.multi(accountIds)
```

---

## Configuration

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    /// Maximum nickname length in bytes
    #[pallet::constant]
    type MaxNicknameLength: Get<u32>;  // default: 128
}
```

## Runtime Integration

```rust
// runtime/src/lib.rs
impl pallet_nickname::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxNicknameLength = ConstU32<128>;
}

construct_runtime! {
    // ...
    Nickname: pallet_nickname,
}
```

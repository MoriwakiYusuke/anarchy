# Wasm Stealth Module API Contract

## Overview

`wasm-engine` の stealth モジュールは、クライアントサイドで実行されるステルスアドレス暗号処理を提供する。

---

## Exported Functions

### 1. generate_stealth_keys

新しいステルス鍵ペアを生成する。

```rust
#[wasm_bindgen]
pub fn generate_stealth_keys() -> StealthKeyPairJs;
```

#### Returns

```typescript
interface StealthKeyPairJs {
  /** Spend秘密鍵 (32 bytes) */
  spendKey: Uint8Array;
  
  /** View秘密鍵 (32 bytes) */
  viewKey: Uint8Array;
  
  /** Spend公開鍵 (32 bytes) */
  spendPubkey: Uint8Array;
  
  /** View公開鍵 (32 bytes) */
  viewPubkey: Uint8Array;
  
  /** メタアドレス文字列 (st:anarchy:...) */
  metaAddress: string;
}
```

#### Example

```typescript
import { generate_stealth_keys } from 'anarchy-wasm-engine';

const keys = generate_stealth_keys();
console.log('Meta-Address:', keys.metaAddress);
// st:anarchy:5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY...
```

---

### 2. derive_stealth_address

送信者がメタアドレスからワンタイムステルスアドレスを導出する。

```rust
#[wasm_bindgen]
pub fn derive_stealth_address(meta_address: &str) -> Result<StealthAddressResult, JsError>;
```

#### Parameters

| Name | Type | Description |
|------|------|-------------|
| meta_address | &str | 受信者のメタアドレス (st:anarchy:...) |

#### Returns

```typescript
interface StealthAddressResult {
  /** ステルスアドレス (SS58) */
  stealthAddress: string;
  
  /** エフェメラル公開鍵 (32 bytes) */
  ephemeralPubkey: Uint8Array;
}
```

#### Errors

| Error | Condition |
|-------|-----------|
| `InvalidMetaAddress` | メタアドレス形式が不正 |
| `DecodingError` | Base58デコード失敗 |

#### Example

```typescript
import { derive_stealth_address } from 'anarchy-wasm-engine';

const result = derive_stealth_address('st:anarchy:5Grw...');
console.log('Stealth Address:', result.stealthAddress);
// 5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw
```

---

### 3. scan_transaction

View鍵を使ってトランザクションが自分宛かどうかを判定する。

```rust
#[wasm_bindgen]
pub fn scan_transaction(
    view_key: &[u8],
    ephemeral_pubkey: &[u8],
    stealth_address: &str,
    spend_pubkey: &[u8],
) -> bool;
```

#### Parameters

| Name | Type | Description |
|------|------|-------------|
| view_key | &[u8] | 自分のView秘密鍵 (32 bytes) |
| ephemeral_pubkey | &[u8] | トランザクションのエフェメラル公開鍵 (32 bytes) |
| stealth_address | &str | トランザクションの宛先ステルスアドレス (SS58) |
| spend_pubkey | &[u8] | 自分のSpend公開鍵 (32 bytes) |

#### Returns

- `true` - 自分宛のトランザクション
- `false` - 他人宛のトランザクション

#### Example

```typescript
import { scan_transaction } from 'anarchy-wasm-engine';

const isOurs = scan_transaction(
  myViewKey,           // Uint8Array(32)
  ephemeralPubkey,     // Uint8Array(32) from on-chain
  stealthAddress,      // SS58 string from on-chain
  mySpendPubkey        // Uint8Array(32)
);

if (isOurs) {
  console.log('Received stealth payment!');
}
```

---

### 4. derive_stealth_private_key

検出されたステルスアドレスの秘密鍵を導出する（支出用）。

```rust
#[wasm_bindgen]
pub fn derive_stealth_private_key(
    spend_key: &[u8],
    view_key: &[u8],
    ephemeral_pubkey: &[u8],
) -> Result<Uint8Array, JsError>;
```

#### Parameters

| Name | Type | Description |
|------|------|-------------|
| spend_key | &[u8] | 自分のSpend秘密鍵 (32 bytes) |
| view_key | &[u8] | 自分のView秘密鍵 (32 bytes) |
| ephemeral_pubkey | &[u8] | トランザクションのエフェメラル公開鍵 (32 bytes) |

#### Returns

- `Uint8Array(32)` - ステルスアドレスの秘密鍵

#### Errors

| Error | Condition |
|-------|-----------|
| `InvalidSpendKey` | Spend鍵が不正 |
| `InvalidViewKey` | View鍵が不正 |
| `InvalidEphemeralKey` | エフェメラル公開鍵が不正 |

#### Example

```typescript
import { derive_stealth_private_key } from 'anarchy-wasm-engine';

const stealthPrivateKey = derive_stealth_private_key(
  mySpendKey,
  myViewKey,
  ephemeralPubkey
);

// Use stealthPrivateKey to sign transactions from stealth address
```

---

### 5. encrypt_backup

鍵ペアをパスワードで暗号化してバックアップ用バイナリを生成する。

```rust
#[wasm_bindgen]
pub fn encrypt_backup(
    spend_key: &[u8],
    view_key: &[u8],
    password: &str,
) -> Result<Uint8Array, JsError>;
```

#### Parameters

| Name | Type | Description |
|------|------|-------------|
| spend_key | &[u8] | Spend秘密鍵 (32 bytes) |
| view_key | &[u8] | View秘密鍵 (32 bytes) |
| password | &str | 暗号化パスワード |

#### Returns

- `Uint8Array` - 暗号化されたバックアップデータ (JSON形式)

#### Implementation

- KDF: PBKDF2-SHA256 (100,000 iterations)
- Cipher: AES-256-GCM
- Salt: 16 bytes random
- Nonce: 12 bytes random

---

### 6. decrypt_backup

暗号化されたバックアップから鍵ペアを復元する。

```rust
#[wasm_bindgen]
pub fn decrypt_backup(
    encrypted: &[u8],
    password: &str,
) -> Result<StealthKeyPairJs, JsError>;
```

#### Parameters

| Name | Type | Description |
|------|------|-------------|
| encrypted | &[u8] | 暗号化されたバックアップデータ |
| password | &str | 復号パスワード |

#### Returns

- `StealthKeyPairJs` - 復元された鍵ペア

#### Errors

| Error | Condition |
|-------|-----------|
| `InvalidBackupFormat` | バックアップ形式が不正 |
| `DecryptionFailed` | パスワードが間違っている |
| `ChecksumMismatch` | データが破損している |

---

### 7. parse_meta_address

メタアドレス文字列をパースして公開鍵を抽出する。

```rust
#[wasm_bindgen]
pub fn parse_meta_address(meta_address: &str) -> Result<MetaAddressParts, JsError>;
```

#### Returns

```typescript
interface MetaAddressParts {
  /** Spend公開鍵 (32 bytes) */
  spendPubkey: Uint8Array;
  
  /** View公開鍵 (32 bytes) */
  viewPubkey: Uint8Array;
}
```

---

### 8. format_meta_address

公開鍵からメタアドレス文字列を生成する。

```rust
#[wasm_bindgen]
pub fn format_meta_address(
    spend_pubkey: &[u8],
    view_pubkey: &[u8],
) -> String;
```

#### Returns

- `String` - メタアドレス (st:anarchy:...)

---

## Internal Cryptography

### EIP-5564 Implementation

```rust
// 送信者側: ステルスアドレス導出
fn derive_stealth_address_internal(
    spend_pubkey: &PublicKey,
    view_pubkey: &PublicKey,
) -> (AccountId, [u8; 32]) {
    // 1. ランダムなエフェメラル秘密鍵を生成
    let ephemeral_secret = StaticSecret::random_from_rng(OsRng);
    let ephemeral_pubkey = PublicKey::from(&ephemeral_secret);
    
    // 2. 共有シークレットを計算: s = r * K_view
    let shared_secret = ephemeral_secret.diffie_hellman(view_pubkey);
    
    // 3. ハッシュ化: h = H(s)
    let h = blake2b_256(shared_secret.as_bytes());
    
    // 4. ステルス公開鍵を計算: P_stealth = K_spend + h * G
    let stealth_pubkey = spend_pubkey.add_scalar(&h);
    
    // 5. AccountId (SS58) に変換
    let stealth_address = pubkey_to_account_id(&stealth_pubkey);
    
    (stealth_address, ephemeral_pubkey.to_bytes())
}

// 受信者側: スキャン判定
fn scan_transaction_internal(
    view_key: &StaticSecret,
    spend_pubkey: &PublicKey,
    ephemeral_pubkey: &PublicKey,
    expected_stealth_address: &AccountId,
) -> bool {
    // 1. 共有シークレットを計算: s' = k_view * R
    let shared_secret = view_key.diffie_hellman(ephemeral_pubkey);
    
    // 2. ハッシュ化: h' = H(s')
    let h = blake2b_256(shared_secret.as_bytes());
    
    // 3. 期待されるステルス公開鍵を計算: P'_stealth = K_spend + h' * G
    let expected_stealth_pubkey = spend_pubkey.add_scalar(&h);
    
    // 4. アドレスを比較
    let computed_address = pubkey_to_account_id(&expected_stealth_pubkey);
    computed_address == *expected_stealth_address
}

// 受信者側: 秘密鍵導出
fn derive_stealth_private_key_internal(
    spend_key: &StaticSecret,
    view_key: &StaticSecret,
    ephemeral_pubkey: &PublicKey,
) -> StaticSecret {
    // 1. 共有シークレットを計算
    let shared_secret = view_key.diffie_hellman(ephemeral_pubkey);
    
    // 2. ハッシュ化
    let h = blake2b_256(shared_secret.as_bytes());
    
    // 3. ステルス秘密鍵を計算: p_stealth = k_spend + h
    spend_key.add_scalar(&h)
}
```

---

## Module Structure

```
packages/wasm-engine/src/stealth/
├── mod.rs          # Module exports
├── keys.rs         # Key generation, parsing
├── address.rs      # Stealth address derivation
├── scan.rs         # Transaction scanning
├── backup.rs       # Backup encryption/decryption
└── tests.rs        # Unit tests
```

---

## Build Integration

### Cargo.toml additions

```toml
[dependencies]
x25519-dalek = { version = "2.0", features = ["static_secrets"] }
aes-gcm = "0.10"
pbkdf2 = "0.12"
sha2 = "0.10"
rand_core = { version = "0.6", features = ["getrandom"] }
```

### wasm-bindgen exports

```rust
// packages/wasm-engine/src/lib.rs
pub mod stealth;

pub use stealth::{
    generate_stealth_keys,
    derive_stealth_address,
    scan_transaction,
    derive_stealth_private_key,
    encrypt_backup,
    decrypt_backup,
    parse_meta_address,
    format_meta_address,
};
```

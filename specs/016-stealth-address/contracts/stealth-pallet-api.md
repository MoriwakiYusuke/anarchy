# Stealth Pallet API Contract

## Overview

`pallet-stealth` はステルスアドレス宛の送金とエフェメラル公開鍵の記録を担当する軽量パレット。

---

## Extrinsics

### 1. send_to_stealth

ステルスアドレスへの送金を実行し、エフェメラル公開鍵をオンチェーンに記録する。

```rust
#[pallet::call_index(0)]
#[pallet::weight(T::WeightInfo::send_to_stealth())]
pub fn send_to_stealth(
    origin: OriginFor<T>,
    stealth_address: T::AccountId,
    ephemeral_pubkey: [u8; 32],
    amount: BalanceOf<T>,
) -> DispatchResult;
```

#### Parameters

| Name | Type | Description |
|------|------|-------------|
| origin | OriginFor<T> | 送信者アカウント (Signed) |
| stealth_address | T::AccountId | ワンタイムステルスアドレス |
| ephemeral_pubkey | [u8; 32] | 送信者が生成したエフェメラル公開鍵 |
| amount | BalanceOf<T> | 送金額 (MORAL最小単位) |

#### Returns

- `Ok(())` - 成功
- `Err(DispatchError)` - 失敗

#### Errors

| Error | Condition |
|-------|-----------|
| `InsufficientBalance` | 送信者の残高不足 |
| `TooManyEntriesInBlock` | 当ブロックのエントリ上限超過 |
| `ZeroAmount` | amount == 0 |

#### Events

```rust
#[pallet::event]
pub enum Event<T: Config> {
    /// ステルス送金が実行された
    StealthTransfer {
        sender: T::AccountId,
        stealth_address: T::AccountId,
        amount: BalanceOf<T>,
    },
}
```

#### Example (PAPI TypeScript)

```typescript
import { createClient } from 'polkadot-api';
import { getWsProvider } from 'polkadot-api/ws-provider/node';

const client = createClient(getWsProvider('ws://127.0.0.1:9944'));
const api = client.getUnsafeApi();

// ステルスアドレスへ送金
const tx = api.tx.stealthPallet.sendToStealth(
  stealthAddress,      // SS58 address
  ephemeralPubkey,     // Uint8Array(32)
  amount               // bigint (MORAL最小単位)
);

await tx.signAndSubmit(signer);
```

---

## Storage

### 1. EphemeralKeys

ブロック番号ごとのエフェメラル公開鍵リスト。

```rust
#[pallet::storage]
#[pallet::getter(fn ephemeral_keys)]
pub type EphemeralKeys<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BlockNumberFor<T>,
    BoundedVec<EphemeralKeyEntry<T::AccountId>, T::MaxEntriesPerBlock>,
    ValueQuery,
>;
```

#### Query (PAPI TypeScript)

```typescript
// 特定ブロックのエフェメラル公開鍵を取得
const entries = await api.query.stealthPallet.ephemeralKeys(blockNumber);

// entries: Array<{ ephemeralPubkey: Uint8Array, stealthAddress: string }>
for (const entry of entries) {
  console.log('Ephemeral:', entry.ephemeralPubkey);
  console.log('Stealth:', entry.stealthAddress);
}
```

---

## Types

### EphemeralKeyEntry

```rust
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EphemeralKeyEntry<AccountId> {
    pub ephemeral_pubkey: [u8; 32],
    pub stealth_address: AccountId,
}
```

---

## Configuration

### Config Trait

```rust
#[pallet::config]
pub trait Config: frame_system::Config + pallet_balances::Config {
    /// Runtime event type
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    
    /// Currency type for transfers
    type Currency: Currency<Self::AccountId>;
    
    /// Maximum ephemeral key entries per block
    #[pallet::constant]
    type MaxEntriesPerBlock: Get<u32>;
    
    /// Weight information
    type WeightInfo: WeightInfo;
}
```

### Recommended Constants

| Constant | Value | Rationale |
|----------|-------|-----------|
| MaxEntriesPerBlock | 1000 | ~64KB/block storage |

---

## Runtime Integration

### Add to runtime/src/lib.rs

```rust
// Import
pub use pallet_stealth;

// Configure
impl pallet_stealth::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MaxEntriesPerBlock = ConstU32<1000>;
    type WeightInfo = pallet_stealth::weights::SubstrateWeight<Runtime>;
}

// Add to construct_runtime!
construct_runtime!(
    pub enum Runtime {
        // ...existing pallets...
        StealthPallet: pallet_stealth,
    }
);
```

---

## Weight Info

### WeightInfo Trait

```rust
pub trait WeightInfo {
    fn send_to_stealth() -> Weight;
}
```

### Benchmark-Based Weights

```rust
impl WeightInfo for SubstrateWeight<Runtime> {
    fn send_to_stealth() -> Weight {
        // Base: balance transfer + storage write
        // Estimated: ~50_000_000 (50μs) + storage costs
        Weight::from_parts(50_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
}
```

---

## Security Considerations

1. **Rate Limiting**: `MaxEntriesPerBlock` でブロックあたりのエントリ数を制限
2. **DOS Prevention**: 送金には実際のトークンが必要なため、スパム攻撃にはコストがかかる
3. **Privacy**: エフェメラル公開鍵から受信者のView鍵は逆算不可能
4. **Replay Protection**: 通常のSubstrateトランザクションノンスで保護

---

## Frontend Integration Summary

### 送金フロー

1. 受信者のメタアドレス取得
2. `wasm-engine` でステルスアドレス + エフェメラル公開鍵を導出
3. `sendToStealth` extrinsic を送信
4. イベント `StealthTransfer` を確認

### スキャンフロー

1. ブロック範囲を決定 (genesis または last scanned)
2. 各ブロックの `ephemeralKeys` をクエリ
3. `wasm-engine` で view key による所有権チェック
4. 検出されたらセッションに追加

### 支出フロー

1. 検出済みステルス残高を選択
2. `wasm-engine` でステルス秘密鍵を導出
3. 通常の `balances.transfer` を署名・送信

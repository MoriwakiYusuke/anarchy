# Faucet Pallet API Contract

**Feature**: 007-pow-faucet  
**Date**: 2026-02-09  
**Status**: Complete

## Pallet Interface

### Extrinsics

#### `claim`

PoW解を提出してFaucet報酬を請求する。

```rust
#[pallet::call_index(0)]
#[pallet::weight(T::WeightInfo::claim())]
pub fn claim(
    origin: OriginFor<T>,
    block_number: BlockNumberFor<T>,
    nonce: u64,
) -> DispatchResult;
```

**Parameters**:
| Name | Type | Description |
|------|------|-------------|
| origin | OriginFor<T> | 署名済みオリジン（請求者） |
| block_number | BlockNumberFor<T> | チャレンジ生成に使用したブロック番号 |
| nonce | u64 | 計算されたPoW解（nonce値） |

**Errors**:
| Error | Description |
|-------|-------------|
| `AlreadyClaimed` | このアカウントは既にFaucetを利用済み |
| `ChallengeExpired` | チャレンジの有効期限切れ（block_numberが古すぎる） |
| `InvalidProof` | PoW解が無効（難易度条件を満たしていない） |
| `BlockNotFound` | 指定されたブロック番号が存在しない |

**Events**:
```rust
FaucetClaimed {
    who: T::AccountId,
    amount: BalanceOf<T>,
    block_number: BlockNumberFor<T>,
}
```

**Weight**: `O(1)` - blake2_256ハッシュ1回 + ストレージ読み書き

---

### Storage

#### `FaucetClaims`

アカウントごとのFaucet利用記録。

```rust
#[pallet::storage]
#[pallet::getter(fn faucet_claims)]
pub type FaucetClaims<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    FaucetClaimRecord<T>,
    OptionQuery,
>;
```

**Key**: `AccountId`  
**Value**: `FaucetClaimRecord<T>` または `None`

#### `TotalClaims`

Faucet利用済みアカウント総数。動的難易度調整に使用。

```rust
#[pallet::storage]
#[pallet::getter(fn total_claims)]
pub type TotalClaims<T> = StorageValue<_, u64, ValueQuery>;
```

**Type**: `u64`  
**Default**: `0`

---

### Runtime Constants

#### `BaseDifficulty`

初期PoW難易度（先頭0ビット数）。

```rust
#[pallet::constant]
type BaseDifficulty: Get<u8>;
```

**Type**: `u8`  
**Default**: `18`  
**Range**: `1..=64`

#### `DifficultyScalingFactor`

難易度+1に必要なアカウント数の倍率。

```rust
#[pallet::constant]
type DifficultyScalingFactor: Get<u64>;
```

**Type**: `u64`  
**Default**: `1000`

#### `MaxDifficulty`

難易度上限。

```rust
#[pallet::constant]
type MaxDifficulty: Get<u8>;
```

**Type**: `u8`  
**Default**: `28`

#### `RewardAmount`

Faucet報酬量（planck単位）。

```rust
#[pallet::constant]
type RewardAmount: Get<BalanceOf<Self>>;
```

**Type**: `Balance` (u128)  
**Default**: `100_000_000_000_000` (100 MORAL)

#### `ChallengeValidity`

チャレンジの有効期限（ブロック数）。

```rust
#[pallet::constant]
type ChallengeValidity: Get<BlockNumberFor<Self>>;
```

**Type**: `BlockNumber` (u32)  
**Default**: `100`

---

### Types

#### `FaucetClaimRecord`

```rust
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[scale_info(skip_type_params(T))]
pub struct FaucetClaimRecord<T: Config> {
    pub block_number: BlockNumberFor<T>,
    pub amount: BalanceOf<T>,
}
```

---

## RPC Interface（将来的な拡張）

現時点では標準のextrinsic submitのみ。将来的にカスタムRPCを追加する場合：

```rust
// faucet_getChallenge(account: AccountId) -> Challenge
// faucet_getDifficulty() -> u8
// faucet_hasClaimedFaucet(account: AccountId) -> bool
```

---

## Frontend API Usage

### PAPI Integration

```typescript
import { createClient } from 'polkadot-api';
import { getWsProvider } from 'polkadot-api/ws-provider/web';

// 1. チャレンジ情報の取得
const blockHash = await client.getFinalizedBlock();
const blockNumber = blockHash.number;

// 2. チャレンジ計算（クライアント側）
const challenge = computeChallenge(blockHash.hash, accountId);

// 3. PoW計算（Web Worker）
const nonce = await minePoW(challenge, difficulty);

// 4. Claim実行
const tx = client.tx.Faucet.claim({
  block_number: blockNumber,
  nonce: nonce,
});

const result = await tx.signAndSubmit(signer);
```

### Query Examples

```typescript
// Faucet利用済みか確認
const claimed = await client.query.Faucet.FaucetClaims(accountId);
const hasClaimed = claimed !== undefined;

// 現在の難易度を計算
const totalClaims = await client.query.Faucet.TotalClaims();
const baseDifficulty = client.consts.Faucet.BaseDifficulty;
const scalingFactor = client.consts.Faucet.DifficultyScalingFactor;
const maxDifficulty = client.consts.Faucet.MaxDifficulty;

const currentDifficulty = Math.min(
  baseDifficulty + Math.floor(Math.log2(1 + Number(totalClaims) / Number(scalingFactor))),
  maxDifficulty
);

// 報酬量取得
const rewardAmount = client.consts.Faucet.RewardAmount;
```

---

## Verification Algorithm

### Challenge Generation

```
INPUT:  block_hash: H256, account_id: AccountId
OUTPUT: challenge: [u8; 32]

challenge = blake2_256(block_hash ++ scale_encode(account_id))
```

### Proof Verification

```
INPUT:  challenge: [u8; 32], nonce: u64, difficulty: u8
OUTPUT: valid: bool

hash = blake2_256(challenge ++ nonce.to_le_bytes())
leading_zeros = count_leading_zero_bits(hash)
valid = leading_zeros >= difficulty
```

### Leading Zero Bits Count

```rust
fn count_leading_zero_bits(hash: &[u8; 32]) -> u8 {
    let mut count = 0u8;
    for byte in hash.iter() {
        if *byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros() as u8;
            break;
        }
    }
    count
}
```

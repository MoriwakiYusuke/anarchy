# パレット仕様

## Post パレット

### ストレージ
| 名前 | 型 | 説明 |
|------|-----|------|
| `Posts` | `StorageMap<u64, Post<T>>` | 投稿メタデータ |
| `Contents` | `StorageMap<u64, BoundedVec<u8, MaxContentLength>>` | 投稿本文 |
| `NextPostId` | `StorageValue<u64>` | 次の投稿ID |
| `UserPosts` | `StorageMap<AccountId, BoundedVec<u64, 1000>>` | ユーザー別投稿ID |

### Post構造体
```rust
pub struct Post<T: Config> {
    pub author: T::AccountId,
    pub content_hash: [u8; 32],
    pub created_at: BlockNumberFor<T>,
    pub parent_id: Option<u64>,
}
```

### Extrinsic
| 名前 | パラメータ | 説明 |
|------|-----------|------|
| `create_post` | `content: Vec<u8>, parent_id: Option<u64>` | 投稿作成（Moral消費） |

### Config要件
- `pallet_moral::Config` を要求
- `PostCost` と `InitialBalance` は `pallet_moral::Config` で定義

## Moral パレット

### ストレージ
| 名前 | 型 | 説明 |
|------|-----|------|
| `Balances` | `StorageMap<AccountId, Balance>` | ユーザー残高 |
| `TotalSupply` | `StorageValue<Balance>` | 総供給量 |

### Extrinsic
| 名前 | パラメータ | 説明 |
|------|-----------|------|
| `transfer` | `to: AccountId, amount: Balance` | 送金 |
| `burn` | `amount: Balance` | 焼却 |
| `mint` | `to: AccountId, amount: Balance` | 発行（Sudo） |
| `claim_initial` | なし | 初期トークン取得（faucet） |

### 内部関数（他パレットから呼び出し可能）
- `do_transfer(&from, &to, amount)`
- `do_mint(&to, amount)`
- `do_burn(&from, amount)`

## Runtime設定
```rust
impl pallet_post::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxContentLength = ConstU32<10000>; // 約10KB
}

impl pallet_moral::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type PostCost = ConstU128<1_000_000_000_000>; // 1 MORAL
    type InitialBalance = ConstU128<100_000_000_000_000>; // 100 MORAL
}
```

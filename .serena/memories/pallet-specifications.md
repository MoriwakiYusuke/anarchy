# パレット仕様

## Post パレット

### ストレージ
| 名前 | 型 | 説明 |
|------|-----|------|
| Posts | StorageMap<u64, Post> | 投稿メタデータ |
| Contents | StorageMap<u64, BoundedVec> | 投稿本文 |
| NextPostId | StorageValue<u64> | 次の投稿ID |
| UserPosts | StorageMap<AccountId, BoundedVec<u64>> | ユーザー別投稿ID |

### Config定数
- MaxContentLength: 10,000 bytes
- PostBaseCost: 10 MORAL
- PostByteCost: 0.1 MORAL/byte

### 投稿コスト計算
total_cost = PostBaseCost + (content_bytes × PostByteCost)

## Moral パレット

### ストレージ
| 名前 | 型 | 説明 |
|------|-----|------|
| Balances | StorageMap<AccountId, Balance> | ユーザー残高 |
| TotalSupply | StorageValue<Balance> | 総供給量 |

### Extrinsic
- transfer: 送金
- burn: 焼却
- mint: 発行（Sudo）
- claim_initial: faucet

### 内部関数
- do_transfer, do_mint, do_burn

## Runtime設定
PostBaseCost: 10_000_000_000_000 (10 MORAL)
PostByteCost: 100_000_000_000 (0.1 MORAL)
InitialBalance: 100_000_000_000_000 (100 MORAL)

## Genesis: テストアカウントに10,000 MORAL配布
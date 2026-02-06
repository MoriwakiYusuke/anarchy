# 技術的知見・トラブルシューティング

## 重要: Polkadot SDK stable2503 + PAPI

### @polkadot/api は使用不可
- Polkadot SDK stable2503はメタデータv16を使用
- @polkadot/apiはv16に未対応
- エラー: `Invalid Transaction: Transaction has a bad signature`

### 解決策: PAPI (polkadot-api) を使用
```bash
npm install polkadot-api @polkadot-labs/hdkd @polkadot-labs/hdkd-helpers
```

### PAPIの使い方
```typescript
import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'
import { sr25519CreateDerive } from '@polkadot-labs/hdkd'
import { DEV_PHRASE, entropyToMiniSecret, mnemonicToEntropy } from '@polkadot-labs/hdkd-helpers'
import { getPolkadotSigner } from 'polkadot-api/signer'
import { Binary } from 'polkadot-api'

// クライアント作成
const client = createClient(getWsProvider('ws://127.0.0.1:9944'))
const api = client.getUnsafeApi()

// Alice署名者作成
const entropy = mnemonicToEntropy(DEV_PHRASE)
const miniSecret = entropyToMiniSecret(entropy)
const derive = sr25519CreateDerive(miniSecret)
const aliceKeyPair = derive('//Alice')
const signer = getPolkadotSigner(aliceKeyPair.publicKey, 'Sr25519', aliceKeyPair.sign)

// トランザクション送信
const tx = api.tx.Post.create_post({
  content: Binary.fromText('Hello'),
  parent_id: undefined
})
await tx.signAndSubmit(signer)

// ストレージ読み取り
const entries = await api.query.Post.Posts.getEntries()
```

## ⚠️ 未解決: PAPI Constants アクセス問題

### 症状
- `unsafeApi.constants.Post.PostBaseCost()` を呼び出すと:
- エラー: `Runtime entry Constant(Post.PostBaseCost) not found`

### 状況
- Rust側で `#[pallet::constant]` 属性は付与済み
- ビルド・再起動後もエラー継続
- メタデータに定数が公開されていない可能性

### 現在の対処
- フロントエンドでフォールバック値を使用
- 実際のコスト計算はオンチェーンで正しく行われる

## パレット間連携

### PostパレットからMoralを消費
```rust
pub trait Config: frame_system::Config + pallet_moral::Config {
    #[pallet::constant]
    type PostBaseCost: Get<pallet_moral::BalanceOf<Self>>;
    
    #[pallet::constant]
    type PostByteCost: Get<pallet_moral::BalanceOf<Self>>;
}
```

## Moral Token 精度
- 12 decimals（1 MORAL = 1_000_000_000_000 units）
- PostBaseCost = 10_000_000_000_000 (10 MORAL)
- PostByteCost = 100_000_000_000 (0.1 MORAL)
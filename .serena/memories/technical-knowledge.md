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

// ストレージ読み取り（getValue()ではなくgetEntries()を使用）
const entries = await api.query.Post.Posts.getEntries()
```

### ストレージ読み取りの注意
- `getValue(key)` はジェネリック型構造体で型不一致エラーになることがある
- `getEntries()` を使用して全エントリを取得する方が安全

## パレット間連携

### PostパレットからMoralを消費
```rust
// Config で pallet_moral を要求
pub trait Config: frame_system::Config + pallet_moral::Config {
    // ...
}

// create_post 内で消費
pallet_moral::Pallet::<T>::do_burn(&who, T::PostCost::get())
    .map_err(|_| Error::<T>::InsufficientMoralBalance)?;
```

### Cargo.toml依存関係
```toml
[dependencies]
pallet-moral = { path = "../moral", default-features = false }

[features]
std = [
    # ...
    "pallet-moral/std",
]
```

## ブロックチェーン起動コマンド
```bash
# 開発モード（一時ストレージ）
./target/release/anarchy-node --dev --tmp

# ビルド
cargo build --release

# テスト
cargo test -p pallet-post
cargo test -p pallet-moral
```

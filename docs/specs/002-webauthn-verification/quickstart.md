# Quickstart: WebAuthn署名検証

## 前提条件

- Rust 1.75+
- Polkadot SDK stable2503
- 既存のAnarchyノードが動作していること

## セットアップ

### 1. 依存クレートの追加

`apps/blockchain/pallets/identity/Cargo.toml`:

```toml
[dependencies]
# 既存の依存関係...

# WebAuthn検証用
p256 = { version = "0.13", default-features = false, features = ["ecdsa-core", "alloc"] }
ecdsa = { version = "0.16", default-features = false, features = ["verifying"] }
sha2 = { version = "0.10", default-features = false }

[features]
std = [
    # 既存のfeatures...
    "p256/std",
    "ecdsa/std",
    "sha2/std",
]
```

### 2. モジュール構成

```
pallets/identity/src/
├── lib.rs           # パレットメイン（エクストリンシック定義）
├── webauthn.rs      # WebAuthn検証ロジック
├── cose.rs          # COSE公開鍵パーサー
└── tests.rs         # ユニットテスト
```

## 実装の流れ

### Phase 1: COSEパーサー実装

```rust
// cose.rs
use sp_std::vec::Vec;

pub struct WebAuthnPublicKey {
    pub x: [u8; 32],
    pub y: [u8; 32],
}

pub fn parse_cose_key(cose_bytes: &[u8]) -> Result<WebAuthnPublicKey, CoseError> {
    // 1. CBORマップをパース
    // 2. kty=2, alg=-7, crv=1を検証
    // 3. x, y座標を抽出
    // 4. 曲線上の点かを検証
}
```

### Phase 2: WebAuthn検証ロジック

```rust
// webauthn.rs
use sha2::{Sha256, Digest};
use p256::ecdsa::{VerifyingKey, signature::Verifier};

pub fn verify_signature(
    public_key: &WebAuthnPublicKey,
    authenticator_data: &[u8],
    client_data_json: &[u8],
    signature: &[u8],
) -> Result<(), WebAuthnError> {
    // 1. authenticatorDataをパース、フラグ検証
    // 2. clientDataJSONをパース、type/challenge検証
    // 3. 署名対象メッセージを構築: SHA256(authData || SHA256(clientDataJSON))
    // 4. 署名形式を正規化（DER → raw）
    // 5. ECDSA検証を実行
}
```

### Phase 3: エクストリンシック統合

```rust
// lib.rs
#[pallet::call]
impl<T: Config> Pallet<T> {
    #[pallet::call_index(3)]
    #[pallet::weight(T::WeightInfo::register_identity_with_webauthn())]
    pub fn register_identity_with_webauthn(
        origin: OriginFor<T>,
        cose_public_key: Vec<u8>,
        // ...
    ) -> DispatchResult {
        // COSEパース → 検証 → 保存
    }
}
```

## テスト実行

```bash
# ユニットテスト
cd apps/blockchain
cargo test -p pallet-identity

# 特定のテスト
cargo test -p pallet-identity webauthn_verification

# 全テスト
cargo test --workspace
```

## テストデータの生成

WebAuthnテストデータは以下のツールで生成可能:

```javascript
// ブラウザコンソールで実行
const credential = await navigator.credentials.get({
    publicKey: {
        challenge: new Uint8Array(32),
        rpId: "localhost",
        allowCredentials: [{
            type: "public-key",
            id: credentialId
        }],
        userVerification: "preferred"
    }
});

// 署名データを16進数で出力
console.log({
    authenticatorData: Array.from(new Uint8Array(credential.response.authenticatorData)).map(b => b.toString(16).padStart(2, '0')).join(''),
    clientDataJSON: new TextDecoder().decode(credential.response.clientDataJSON),
    signature: Array.from(new Uint8Array(credential.response.signature)).map(b => b.toString(16).padStart(2, '0')).join('')
});
```

## トラブルシューティング

### コンパイルエラー: `alloc` not found

`Cargo.toml`で`alloc`フィーチャーを有効にしてください:

```toml
p256 = { version = "0.13", default-features = false, features = ["ecdsa-core", "alloc"] }
```

### 署名検証失敗

1. 署名形式を確認（DER vs raw）
2. メッセージ構築順序を確認（authData || SHA256(clientDataJSON)）
3. 公開鍵のバイト順序を確認（x, yが正しい順序か）

### rpIdHashの不一致

設定されたrpIdとclientDataJSON.originが一致しているか確認してください。

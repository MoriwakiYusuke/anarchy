# Quickstart: Identity Pallet

**Date**: 2026-02-07  
**Feature**: 001-identity-pallet

---

## Prerequisites

- Rust 1.75+
- Polkadot SDK stable2503 (既存環境で構築済み)
- 実行中のAnarchyノード (`./target/release/anarchy-node --dev --tmp`)

---

## 1. パレット作成

```bash
cd apps/blockchain/pallets
mkdir -p identity/src
```

### Cargo.toml

`apps/blockchain/pallets/identity/Cargo.toml`:

```toml
[package]
name = "pallet-identity"
version = "0.1.0"
edition = "2021"

[dependencies]
codec = { package = "parity-scale-codec", version = "3.6", default-features = false, features = ["derive"] }
scale-info = { version = "2.10", default-features = false, features = ["derive"] }
frame-support = { git = "https://github.com/paritytech/polkadot-sdk", tag = "stable2503", default-features = false }
frame-system = { git = "https://github.com/paritytech/polkadot-sdk", tag = "stable2503", default-features = false }
sp-runtime = { git = "https://github.com/paritytech/polkadot-sdk", tag = "stable2503", default-features = false }
sp-core = { git = "https://github.com/paritytech/polkadot-sdk", tag = "stable2503", default-features = false }
sp-std = { git = "https://github.com/paritytech/polkadot-sdk", tag = "stable2503", default-features = false }

[features]
default = ["std"]
std = [
    "codec/std",
    "scale-info/std",
    "frame-support/std",
    "frame-system/std",
    "sp-runtime/std",
    "sp-core/std",
    "sp-std/std",
]
```

---

## 2. Runtime統合

### workspace Cargo.toml に追加

`apps/blockchain/Cargo.toml`（workspaceメンバー追加）:

```toml
[workspace]
members = [
    # ...existing members...
    "pallets/identity",
]
```

### Runtime に追加

`apps/blockchain/runtime/src/lib.rs`:

```rust
// パレットインポート
pub use pallet_identity;

// Config実装
impl pallet_identity::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxPasskeys = ConstU32<10>;
    type MaxPublicKeyLength = ConstU32<256>;
    type MaxDeviceNameLength = ConstU32<64>;
}

// construct_runtime! に追加
construct_runtime!(
    pub enum Runtime {
        // ...existing pallets...
        Identity: pallet_identity,
    }
);
```

### Runtime Cargo.toml に依存追加

`apps/blockchain/runtime/Cargo.toml`:

```toml
[dependencies]
pallet-identity = { path = "../pallets/identity", default-features = false }

[features]
std = [
    # ...existing...
    "pallet-identity/std",
]
```

---

## 3. ビルド & テスト

```bash
# パレット単体テスト
cd apps/blockchain
cargo test -p pallet-identity

# ランタイムビルド
cargo build --release

# ノード起動
./target/release/anarchy-node --dev --tmp
```

---

## 4. フロントエンド連携（PAPI）

### Identity作成

```typescript
import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'

const client = createClient(getWsProvider('ws://127.0.0.1:9944'))
const api = client.getUnsafeApi()

// WebAuthnで取得した公開鍵（COSE形式）
const publicKey = new Uint8Array([/* COSE key bytes */])

// Identity作成トランザクション
const tx = api.tx.Identity.register_identity({
    public_key: Array.from(publicKey),
    device_name: "My Device"
})

// 署名して送信
await tx.signAndSubmit(signer)

// イベントから identity_id を取得
```

### Identity照会

```typescript
// 全Identity取得
const entries = await api.query.Identity.Identities.getEntries()
for (const [id, identity] of entries) {
    console.log(`Identity ${id}:`, identity)
}

// PasskeyIdからIdentity逆引き
const passkeyId = new Uint8Array([/* 32 bytes */])
const identityId = await api.query.Identity.PasskeyOwner.getValue(passkeyId)
```

---

## 5. テストケース概要

### 単体テスト（tests.rs）

| テスト名 | 検証内容 |
|----------|----------|
| `register_identity_works` | 正常にIdentity作成 |
| `register_identity_duplicate_passkey_fails` | 重複公開鍵で失敗 |
| `add_passkey_works` | Passkey追加成功 |
| `add_passkey_max_limit` | 10個以上で失敗 |
| `remove_passkey_works` | Passkey削除成功 |
| `remove_last_passkey_fails` | 最後の1つは削除不可 |

### 統合テスト（将来）

- マルチノードでのIdentity同期
- フロントエンドE2Eテスト

---

## 6. 次のステップ

1. `/speckit.tasks` でタスク分解
2. テストファースト実装
3. WebAuthn署名検証（Phase 1.4）との統合準備

---

## References

- [spec.md](./spec.md) - 機能仕様
- [data-model.md](./data-model.md) - データモデル
- [contracts/identity-pallet.md](./contracts/identity-pallet.md) - API仕様
- [research.md](./research.md) - 技術調査

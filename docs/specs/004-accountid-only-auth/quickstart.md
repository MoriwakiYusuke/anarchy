# Quickstart: AccountIdのみによる認証への移行

**Feature**: 004-accountid-only-auth  
**Date**: 2026-02-08

## 概要

このドキュメントでは、WebAuthn認証からAccountIdベース認証への移行手順を説明します。

---

## 1. 前提条件

### 1.1 開発環境

- Rust 1.75+
- Node.js 18+
- pnpm
- Polkadot.js Extension（テスト用）

### 1.2 現在のブランチ

```bash
git checkout 004-accountid-only-auth
```

---

## 2. Step 1: WebAuthnコードの削除（Identity Pallet）

### 2.1 ファイル削除

```bash
cd apps/blockchain

# WebAuthn関連モジュール削除
rm pallets/identity/src/cose.rs
rm pallets/identity/src/webauthn.rs
```

### 2.2 lib.rsの更新

`pallets/identity/src/lib.rs` から以下を削除：

```rust
// 削除: モジュール宣言
pub mod cose;
pub mod webauthn;

// 削除: Passkey関連の型定義
pub type PasskeyId = [u8; 32];
pub struct Passkey<...> { ... }
pub type PasskeyOf<T> = ...;

// 削除: Identity構造体のpasskeysフィールド
pub struct Identity<...> {
    // passkeys: BoundedVec<...>  ← 削除
}

// 削除: Config項目
type MaxPasskeys: Get<u32>;
type MaxPublicKeyLength: Get<u32>;
type MaxDeviceNameLength: Get<u32>;

// 削除: ストレージ
PasskeyOwner<T>
NextIdentityId<T>

// 削除: Passkey関連エクストリンシック
add_passkey(...)
remove_passkey(...)

// 削除: Passkey関連イベント・エラー
```

### 2.3 Cargo.toml更新

`pallets/identity/Cargo.toml` から削除：

```toml
# 削除
p256 = { workspace = true }
ecdsa = { workspace = true }

# features/std からも削除
# "p256/std",
# "ecdsa/std",
```

---

## 3. Step 2: WebAuthnコードの削除（Post Pallet）

### 3.1 lib.rsの更新

`pallets/post/src/lib.rs` から以下を削除：

```rust
// 削除: WebAuthn importの削除
use pallet_identity::webauthn::{...};
use sha2::{Digest, Sha256};  // ← sha2は残す場合はコンテンツハッシュ用のみ

// 削除: WebAuthnSignatureData構造体
pub struct WebAuthnSignatureData { ... }

// 削除: WebAuthn関連エクストリンシック
pub fn create_post_with_webauthn(...) { ... }

// 削除: WebAuthn関連イベント
PostCreatedWithWebAuthn { ... }

// 削除: WebAuthn関連エラー
IdentityNotFound,
PasskeyNotFound,
InvalidSignature,
// etc.
```

### 3.2 tests.rsの更新

`pallets/post/src/tests.rs` からWebAuthn関連テストを削除。

---

## 4. Step 3: Identity Palletの削除

WebAuthn廃止によりIdentity Palletの存在意義（複数パスキー管理）がなくなったため、完全に削除します。

### 4.1 Identity Palletディレクトリ削除

```bash
cd apps/blockchain

# Identity Pallet全体を削除
rm -rf pallets/identity/
```

### 4.2 Post PalletからIdentity依存を削除

`pallets/post/Cargo.toml` から削除：

```toml
# 削除
pallet-identity = { path = "../identity", ... }
```

`pallets/post/src/lib.rs` から削除：

```rust
// 削除: Identity Palletへの参照
use pallet_identity::*;
```

### 4.3 RuntimeからIdentity Palletを削除

`runtime/Cargo.toml` から削除：

```toml
# 削除
pallet-identity = { path = "../pallets/identity", ... }
```

`runtime/src/lib.rs` から削除：

```rust
// 削除: implブロック
impl pallet_identity::Config for Runtime {
    // ...
}

// 削除: construct_runtime!内の行
Identity: pallet_identity,
```

### 4.4 ワークスペースから削除

`apps/blockchain/Cargo.toml` の `[workspace.members]` から削除：

```toml
# 削除
"pallets/identity",
```

---

---

## 5. Step 4: フロントエンド更新

### 5.1 WebAuthn関連コードの削除

WebAuthn API呼び出しを削除し、シードフレーズ入力方式に置き換える。

### 5.2 シードフレーズ管理の実装

```typescript
// hooks/useSeedPhrase.ts
import { mnemonicGenerate, mnemonicValidate } from '@polkadot/util-crypto';
import { Keyring } from '@polkadot/keyring';
import { useState, useCallback, useMemo } from 'react';

export function useSeedPhrase() {
  const [seedPhrase, setSeedPhrase] = useState<string>('');
  const [isValid, setIsValid] = useState(false);

  const keyring = useMemo(() => new Keyring({ type: 'sr25519' }), []);

  const keyPair = useMemo(() => {
    if (!seedPhrase || !isValid) return null;
    try {
      return keyring.addFromUri(seedPhrase);
    } catch {
      return null;
    }
  }, [seedPhrase, isValid, keyring]);

  const accountId = useMemo(() => keyPair?.address ?? null, [keyPair]);

  const generate = useCallback(() => {
    const mnemonic = mnemonicGenerate(12);
    setSeedPhrase(mnemonic);
    setIsValid(true);
    return mnemonic;
  }, []);

  const validate = useCallback((phrase: string) => {
    const valid = mnemonicValidate(phrase);
    setSeedPhrase(phrase);
    setIsValid(valid);
    return valid;
  }, []);

  const sign = useCallback((message: Uint8Array) => {
    if (!keyPair) throw new Error('No key pair');
    return keyPair.sign(message);
  }, [keyPair]);

  return { seedPhrase, accountId, isValid, generate, validate, sign, keyPair };
}
```

---

## 6. ビルドとテスト

### 6.1 コンパイル確認

```bash
cd apps/blockchain

# クリーンビルド
cargo clean -p pallet-post
cargo build --release
```

### 6.2 テスト実行

```bash
# Post Palletテスト
cargo test -p pallet-post

# 全テスト
cargo test
```

### 6.3 期待される結果

- WebAuthn関連のテストが削除されている
- Identity Palletが完全に削除されている
- 残りのテストが全てパス
- コンパイルエラーなし

---

## 7. 手動テスト

### 7.1 ノード起動

```bash
cd apps/blockchain
./target/release/anarchy-node --dev
```

### 7.2 フロントエンドテスト

1. フロントエンドを起動: `pnpm dev`
2. 「新規生成」ボタンをクリック
3. 表示されたシードフレーズをコピー（バックアップ用）
4. AccountIdが表示されることを確認

### 7.3 投稿作成テスト

Identity Pallet削除後は、登録ステップなしで即座に投稿できます。

1. シードフレーズを入力（または新規生成）
2. 投稿内容を入力
3. 投稿ボタンをクリック
4. ブロックチェーンに記録されることを確認

### 7.4 Polkadot.js Appsでの確認（オプション）

1. https://polkadot.js.org/apps/ を開く
2. `Development` → `Local Node` に接続
3. `post` → `posts` ストレージで投稿を確認

---

## 8. チェックリスト

実装完了時に確認：

- [ ] `pallets/identity/` ディレクトリが削除されている
- [ ] `p256`, `ecdsa` 依存が削除されている
- [ ] RuntimeからIdentity Pallet参照が削除されている
- [ ] Post Palletが単独でコンパイルできる
- [ ] 全テストがパスする
- [ ] ノードが起動できる
- [ ] シードフレーズ入力後、登録なしで即座に投稿できる
- [ ] WASMランタイムサイズが削減されている

---

## 9. トラブルシューティング

### 9.1 コンパイルエラー: 未解決のインポート

```
error: unresolved import `pallet_identity`
```

**解決策**: Post Pallet, RuntimeからIdentity Palletへの参照を全て削除。

### 9.2 コンパイルエラー: ワークスペースメンバーが見つからない

```
error: failed to load manifest for workspace member `pallets/identity`
```

**解決策**: `apps/blockchain/Cargo.toml` の `[workspace.members]` から `"pallets/identity"` を削除。

### 9.3 ランタイムエラー: ストレージデコード失敗

```
error decoding storage
```

**解決策**: 既存のチェーンデータを削除して新規開始。

```bash
rm -rf data/alice data/bob data/charlie
```

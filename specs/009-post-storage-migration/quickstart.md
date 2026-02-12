# Quickstart: Post Storage Migration

**Feature**: 009-post-storage-migration  
**Date**: 2026-02-10

本ドキュメントは、Post Storage Migrationの実装を開始するための最小限の手順を示す。

---

## 前提条件

- Rust 1.87+ (stable2503)
- Node.js 20+
- pnpm
- 既存のAnarchy開発環境がセットアップ済み

---

## 1. Wasm暗号エンジンのセットアップ

### 1.1 パッケージ作成

```bash
cd /home/moriwaki-y/self/anarchy
mkdir -p packages/wasm-engine
cd packages/wasm-engine
```

### 1.2 Cargo.toml

```toml
[package]
name = "anarchy-wasm-engine"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
sharks = "0.5"
rs_merkle = "1.4"
blake2 = "0.10"
wasm-bindgen = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde-wasm-bindgen = "0.6"
js-sys = "0.3"

[dev-dependencies]
wasm-bindgen-test = "0.3"
```

### 1.3 最小実装テスト

```rust
// src/lib.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}
```

```bash
cargo test
wasm-pack build --target web
```

---

## 2. pallet-post の修正

### 2.1 PostContent構造体追加

```rust
// apps/blockchain/pallets/post/src/lib.rs

/// 分散ストレージ参照情報
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct PostContent {
    pub root: [u8; 32],
    pub k: u32,
    pub n: u32,
    pub size: u64,
}

/// ContentRefs Storage追加
#[pallet::storage]
pub type ContentRefs<T: Config> = StorageMap<_, Blake2_128Concat, u64, PostContent>;
```

### 2.2 create_post 変更

```rust
#[pallet::call_index(0)]
#[pallet::weight(T::DbWeight::get().reads_writes(3, 3))]
pub fn create_post(
    origin: OriginFor<T>,
    merkle_root: [u8; 32],
    k: u32,
    n: u32,
    total_size: u64,
    parent_id: Option<u64>,
) -> DispatchResult {
    let who = ensure_signed(origin)?;
    
    // k/n バリデーション
    ensure!(k > 0 && k <= n, Error::<T>::InvalidParameters);
    
    // コスト計算（基本料金 + サイズ係数）
    // ...
    
    let post_id = NextPostId::<T>::get();
    
    // Post メタデータ保存
    Posts::<T>::insert(post_id, Post {
        author: who.clone(),
        content_hash: merkle_root,
        created_at: frame_system::Pallet::<T>::block_number(),
        parent_id,
    });
    
    // ContentRefs 保存
    ContentRefs::<T>::insert(post_id, PostContent {
        root: merkle_root,
        k,
        n,
        size: total_size,
    });
    
    // ...
    Ok(())
}
```

### 2.3 テスト実行

```bash
cd apps/blockchain
cargo test -p pallet-post
```

---

## 3. Blockchain Node カスタムRPC

### 3.1 RPC モジュール追加

```rust
// apps/blockchain/node/src/rpc/storage.rs

use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use sp_core::Bytes;

#[rpc(server)]
pub trait StorageApi {
    #[method(name = "storage_uploadFragment")]
    async fn upload_fragment(
        &self,
        post_id: String,
        index: u32,
        data: Bytes,
        proof: Bytes,
        merkle_root: Bytes,
        leaf_count: u32,
    ) -> RpcResult<UploadResult>;

    #[method(name = "storage_getFragment")]
    async fn get_fragment(
        &self,
        post_id: String,
        index: u32,
    ) -> RpcResult<Option<Bytes>>;
}
```

### 3.2 service.rs でRPC登録

```rust
// Full RPC construction
let rpc_extensions_builder = {
    // ...
    move |deny_unsafe, subscription_task_executor| {
        let deps = rpc::FullDeps {
            client: client.clone(),
            pool: pool.clone(),
            // storage_network: network.clone(), // TODO: Add
        };
        rpc::create_full(deps).map_err(Into::into)
    }
};
```

---

## 4. フロントエンド統合

### 4.1 Wasm Worker

```typescript
// apps/frontend/src/workers/crypto.ts
import init, { split_data, recover_data } from 'anarchy-wasm-engine';

let initialized = false;

self.onmessage = async (e) => {
  if (!initialized) {
    await init();
    initialized = true;
  }
  
  const { type, payload } = e.data;
  
  if (type === 'split') {
    const { data, k, n } = payload;
    const result = split_data(new Uint8Array(data), k, n);
    self.postMessage({ type: 'split_result', payload: result });
  }
  
  if (type === 'recover') {
    const { fragments } = payload;
    const result = recover_data(fragments);
    self.postMessage({ type: 'recover_result', payload: result });
  }
};
```

### 4.2 useStorage Hook

```typescript
// apps/frontend/src/hooks/useStorage.ts
import { useApi } from './useApi';

export function useStorage() {
  const { api } = useApi();
  
  const uploadFragment = async (
    postId: string,
    index: number,
    data: Uint8Array,
    proof: Uint8Array,
    merkleRoot: Uint8Array,
    leafCount: number,
  ) => {
    return api.rpc.storage.uploadFragment(
      postId,
      index,
      toHex(data),
      toHex(proof),
      toHex(merkleRoot),
      leafCount,
    );
  };
  
  const getFragment = async (postId: string, index: number) => {
    return api.rpc.storage.getFragment(postId, index);
  };
  
  return { uploadFragment, getFragment };
}
```

---

## 5. 検証手順

### 5.1 ローカル環境起動

```bash
# ターミナル1: Blockchain Node
cd apps/blockchain
cargo run --release -- --dev

# ターミナル2: Storage Node
cd apps/storage-node
cargo run -- --config config.example.toml

# ターミナル3: Frontend
cd apps/frontend
pnpm dev
```

### 5.2 手動テスト

1. フロントエンドで投稿を作成
2. ブラウザDevToolsでNetwork確認:
   - `storage_uploadFragment` RPC呼び出し x n回
   - `post.create_post` extrinsic
3. 投稿がタイムラインに表示されることを確認

### 5.3 統合テスト

```bash
cd apps/blockchain
cargo test -p anarchy-integration-tests
```

---

## トラブルシューティング

| 問題 | 解決策 |
|------|--------|
| Wasm build失敗 | `wasm-pack --version`確認、`rustup target add wasm32-unknown-unknown` |
| RPC not found | service.rsでカスタムRPCがmergeされているか確認 |
| Storage Node接続失敗 | libp2pブートストラップノード設定確認 |
| MerkleProof検証失敗 | Wasmエンジンとノード側のBlake2b実装が一致しているか確認 |

---

## 次のステップ

1. `/speckit.tasks` でタスク分解
2. テスト駆動で各コンポーネント実装
3. 統合テストで全体動作確認

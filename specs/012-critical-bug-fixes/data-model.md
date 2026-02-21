# Data Model: Critical Bug Fixes (HIGH Priority 13 Issues)

**Date**: 2026-02-21  
**Feature**: [spec.md](spec.md) | [research.md](research.md)

## Overview

本機能はバグ修正であり、新規エンティティの追加は最小限。既存データ構造の修正と新規エラー型の追加が主な変更点。

---

## 1. Pallet Storage (Rust/FRAME)

### 新規エラー型

```rust
#[pallet::error]
pub enum Error<T> {
    // 既存エラー...
    
    /// Issue 1: チャレンジ発行者が登録済みストレージノードでない
    IssuerNotRegisteredNode,
}
```

### 既存構造の修正

#### ProofRecord 

```rust
// 変更前
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct ProofRecord<Balance, BlockNumber> {
    pub pending_reward: Balance,  // ← 削除対象
    // ...
}

// 変更後 (Issue 3)
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct ProofRecord<BlockNumber> {
    // pending_reward フィールド削除
    // ...
}
```

### 新規ストレージ（オプション）

```rust
/// Issue 2: チャレンジを deadline でインデックス（効率的な期限切れ処理）
#[pallet::storage]
pub type ChallengesByDeadline<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BlockNumberFor<T>,        // deadline block
    BoundedVec<ChallengeId, T::MaxChallengesPerBlock>,
    ValueQuery,
>;
```

---

## 2. Node Gossip (Rust)

### 新規定数

```rust
// apps/blockchain/node/src/gossip/mod.rs

/// Issue 6: 最大同時接続数
pub const MAX_CONNECTIONS: usize = 128;

/// Issue 7: レジストリ最大エントリ数
pub const MAX_REGISTRY_SIZE: usize = 10_000;
```

### 既存構造の修正

```rust
/// StorageNodeRegistry に LRU サポート追加
pub struct StorageNodeRegistry {
    pub nodes: HashMap<PeerId, RegisteredStorageNode>,
    /// 追加: 登録順序を追跡（LRU用）
    pub insertion_order: VecDeque<PeerId>,
}

impl StorageNodeRegistry {
    /// LRU方式で最も古いエントリを削除
    pub fn evict_oldest(&mut self) {
        if let Some(oldest_peer) = self.insertion_order.pop_front() {
            self.nodes.remove(&oldest_peer);
        }
    }
}
```

---

## 3. Wasm Engine (Rust/WASM)

### 新規エラー型

```rust
// packages/wasm-engine/src/kzg/key_sss.rs

#[derive(Debug, Clone)]
pub enum KeySssError {
    /// Issue 8: RNG初期化失敗
    RngFailed,
    /// 既存エラー...
    InvalidThreshold,
    InvalidShareCount,
}

// packages/wasm-engine/src/kzg/proof.rs

#[derive(Debug, Clone)]
pub enum KzgError {
    /// Issue 9: コミットメント不整合
    CommitmentMismatch,
    /// 既存エラー...
    InvalidInput,
    ProofGenerationFailed,
}
```

### 関数シグネチャ変更

```rust
// 変更前
fn sss_split_byte(secret: u8, k: u8, n: u8) -> Vec<(u8, u8)>

// 変更後 (Issue 8)
fn sss_split_byte(secret: u8, k: u8, n: u8) -> Result<Vec<(u8, u8)>, KeySssError>

// 呼び出し元も含めて伝播
pub fn key_split(data: &[u8], k: u8, n: u8) -> Result<Vec<Share>, KeySssError>
```

---

## 4. Storage Node (Rust)

### 新規設定

```rust
// apps/storage-node/src/config.rs

#[derive(Debug, Clone, Deserialize)]
pub struct RpcReconnectConfig {
    /// 最大リトライ回数
    pub max_retries: u32,          // Default: 10
    /// 初期待機時間 (ms)
    pub initial_delay_ms: u64,     // Default: 1000
    /// 最大待機時間 (ms)
    pub max_delay_ms: u64,         // Default: 60000
}
```

### 既存構造の修正

```rust
// apps/storage-node/src/chain/mod.rs

pub struct ChainClient {
    // 既存フィールド...
    
    /// 追加: 再接続設定
    reconnect_config: RpcReconnectConfig,
    /// 追加: 現在のリトライカウント
    current_retry_count: AtomicU32,
}
```

---

## 5. Frontend (TypeScript)

### 新規型定義

```typescript
// apps/frontend/src/workers/types.ts

export interface CryptoTask {
  id: string;
  type: 'encrypt' | 'decrypt' | 'split' | 'reconstruct' | 'merkle';
  payload: Uint8Array;
  options?: Record<string, unknown>;
}

export interface CryptoResult {
  id: string;
  success: boolean;
  data?: Uint8Array;
  error?: string;
}

// apps/frontend/src/workers/WorkerPool.ts

export interface WorkerPoolConfig {
  /** ワーカー数（デフォルト: navigator.hardwareConcurrency || 4） */
  size?: number;
  /** 最大ワーカー数 */
  maxSize?: number;  // Default: 8
}
```

### useStorage 分割後の型

```typescript
// apps/frontend/src/hooks/storage/types.ts

export interface StorageWorkerContext {
  pool: WorkerPool;
  execute: (task: CryptoTask) => Promise<CryptoResult>;
}

export interface StorageCryptoContext {
  split: (data: Uint8Array, k: number, n: number) => Promise<Share[]>;
  reconstruct: (shares: Share[], k: number) => Promise<Uint8Array>;
  merkleRoot: (data: Uint8Array) => Promise<string>;
}

export interface StorageRpcContext {
  uploadFragment: (fragment: Fragment) => Promise<void>;
  downloadFragment: (id: string) => Promise<Fragment>;
  queryNodes: () => Promise<StorageNode[]>;
}
```

---

## 6. 共有定数 (新規 Crate)

### packages/kzg-constants

```rust
// packages/kzg-constants/Cargo.toml
[package]
name = "kzg-constants"
version = "0.1.0"

// packages/kzg-constants/src/lib.rs

/// BLS12-381 G2 point for KZG trusted setup
/// Verified to be a valid point on the curve
pub const TAU_G2_BYTES: [u8; 96] = [
    // ... validated 96 bytes ...
];

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bls12_381::G2Affine;
    use ark_serialize::CanonicalDeserialize;
    
    #[test]
    fn tau_g2_is_valid_point() {
        let point = G2Affine::deserialize_compressed(&TAU_G2_BYTES[..])
            .expect("TAU_G2_BYTES must be valid G2 point");
        assert!(point.is_on_curve());
        assert!(point.is_in_correct_subgroup_assuming_on_curve());
    }
}
```

---

## Entity Relationship Summary

```
┌─────────────────────────────────────────────────────────────┐
│                      PALLET STORAGE                          │
├─────────────────────────────────────────────────────────────┤
│  PendingChallenges ────┐                                    │
│        │               │                                    │
│        v               v                                    │
│  ChallengesByDeadline  (新規: deadline インデックス)         │
│        │                                                    │
│        v                                                    │
│  NodeScores ─── on_finalize で期限切れ時に減算              │
│                                                              │
│  ProofRecord                                                │
│    └── pending_reward (削除) → PendingRewards に統合        │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                      NODE GOSSIP                             │
├─────────────────────────────────────────────────────────────┤
│  connected_peers ─── MAX_CONNECTIONS (128) 制限              │
│        │                                                    │
│        v                                                    │
│  StorageNodeRegistry ─── MAX_REGISTRY_SIZE (10,000) 制限    │
│    └── insertion_order (新規: LRU追跡)                      │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                      FRONTEND                                │
├─────────────────────────────────────────────────────────────┤
│  WorkerPool (新規) ─── 共有Workerプール                      │
│        │                                                    │
│        v                                                    │
│  PostItem ─── WorkerPoolContext 経由で利用                  │
│                                                              │
│  useStorage (分割)                                          │
│    ├── useStorageWorker                                     │
│    ├── useStorageCrypto                                     │
│    ├── useStorageRpc                                        │
│    └── useStorageAuth                                       │
└─────────────────────────────────────────────────────────────┘
```

---

## Migration Notes

### Pallet Storage

- **ProofRecord.pending_reward 削除**: ストレージマイグレーション必要
  - 既存データの `pending_reward` は無視（PendingRewards が正）
  - v2 構造体と OnRuntimeUpgrade の実装が必要

### Breaking Changes

- `register_kzg_fragment` extrinsic が削除される
  - 外部から直接呼び出しているケースはエラーになる
  - Post pallet 経由のみ有効

### Frontend

- `useStorage` hook の API は維持
  - 内部実装が分割されるが、公開インターフェースは変更なし

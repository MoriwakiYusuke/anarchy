# Data Model: KZG-VSS 保持証明・報酬システム

**Feature**: 011-kzg-proof-rewards  
**Date**: 2026-02-16

## Overview

本機能のデータモデルは4つのレイヤーにまたがる:
1. **Wasm Engine** - KZG-VSS暗号構造体
2. **Storage Pallet** - オンチェーンストレージ
3. **Storage Node** - オフチェーンデータ
4. **Frontend** - UI状態

---

## 1. Wasm Engine Entities

### VssShare

断片化された秘密のシェア。BLS12-381スカラー体の元。

```rust
pub struct VssShare {
    /// シェアインデックス (1..=n)
    pub index: u8,
    /// シェア値 (BLS12-381 scalar, 32 bytes)
    pub value: [u8; 32],
}
```

**Validation Rules**:
- `index` は 1 以上 n 以下
- `value` は BLS12-381 スカラー体の有効な元（< field modulus）

### KzgCommitment

多項式へのコミットメント。G1点として表現。

```rust
pub struct KzgCommitment {
    /// Compressed G1 point (48 bytes)
    pub bytes: [u8; 48],
}
```

**Validation Rules**:
- `bytes` は有効な圧縮G1点（曲線上の点）
- 無効なバイト列は `InvalidCommitment` エラー

### KzgProof

特定点における評価値の正当性証明。G1点として表現。

```rust
pub struct KzgProof {
    /// Compressed G1 point (48 bytes)
    pub bytes: [u8; 48],
}
```

**Validation Rules**:
- `bytes` は有効な圧縮G1点

### VssSplitResult

`vss_split` の出力。

```rust
pub struct VssSplitResult {
    /// KZGコミットメント
    pub commitment: KzgCommitment,
    /// 生成されたシェア (n個)
    pub shares: Vec<VssShare>,
    /// 各シェアの証明 (n個)
    pub proofs: Vec<KzgProof>,
    /// 圧縮使用フラグ
    pub compressed: bool,
}
```

---

## 2. Storage Pallet Entities (On-chain)

### Fragment

オンチェーンに保存される断片メタデータ。

```rust
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct Fragment<AccountId, BlockNumber> {
    /// 投稿者アカウント
    pub owner: AccountId,
    /// KZGコミットメント (48 bytes)
    pub commitment: BoundedVec<u8, ConstU32<48>>,
    /// データサイズ (bytes)
    pub data_size: u32,
    /// 断片数 (n)
    pub fragment_count: u8,
    /// 閾値 (k)
    pub threshold: u8,
    /// 作成ブロック
    pub created_at: BlockNumber,
    /// アクティブなシェア保持者
    pub holders: BoundedVec<AccountId, ConstU32<16>>,
}
```

**Storage Map**:
```rust
#[pallet::storage]
pub type Fragments<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ContentHash,  // H256
    Fragment<T::AccountId, BlockNumberFor<T>>,
>;
```

### Challenge

チェーンが発行する保持証明要求。

```rust
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct Challenge<AccountId, BlockNumber> {
    /// 対象コンテンツハッシュ
    pub content_hash: H256,
    /// 対象シェアインデックス
    pub share_index: u8,
    /// チャレンジされたノード
    pub challenged_node: AccountId,
    /// 発行ブロック
    pub issued_at: BlockNumber,
    /// 期限ブロック
    pub deadline: BlockNumber,
}
```

**Storage Map**:
```rust
#[pallet::storage]
pub type PendingChallenges<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    (ContentHash, u8),  // (content_hash, share_index)
    Challenge<T::AccountId, BlockNumberFor<T>>,
>;
```

### ProofRecord

保持証明の記録。

```rust
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct ProofRecord<BlockNumber> {
    /// 最後の証明成功ブロック
    pub last_proved_at: BlockNumber,
    /// 連続成功回数
    pub success_count: u32,
    /// 連続失敗回数
    pub failure_count: u32,
    /// 累積報酬（未クレーム）
    pub pending_reward: u128,
}
```

**Storage DoubleMap**:
```rust
#[pallet::storage]
pub type ProofRecords<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    ContentHash,
    Blake2_128Concat,
    T::AccountId,  // holder
    ProofRecord<BlockNumberFor<T>>,
>;
```

### RewardPool

報酬プール。

```rust
#[pallet::storage]
pub type RewardPoolBalance<T: Config> = StorageValue<_, u128, ValueQuery>;
```

### ScoreCache

スコアキャッシュ（外部システムから取得）。

```rust
#[pallet::storage]
pub type ScoreCache<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ContentHash,
    u64,  // score
>;
```

---

## 3. Storage Node Entities (Off-chain)

### StoredShare

ノードが保持するシェア。

```rust
pub struct StoredShare {
    /// コンテンツハッシュ
    pub content_hash: H256,
    /// シェアインデックス
    pub index: u8,
    /// シェア値 (暗号化済み)
    pub encrypted_value: Vec<u8>,
    /// KZG proof（チャレンジ応答用）
    pub proof: KzgProof,
    /// 受信タイムスタンプ
    pub received_at: u64,
    /// 最終報酬受領タイムスタンプ
    pub last_rewarded_at: Option<u64>,
}
```

**Database Schema** (SQLite/RocksDB):
```sql
CREATE TABLE shares (
    content_hash BLOB PRIMARY KEY,
    share_index INTEGER NOT NULL,
    encrypted_value BLOB NOT NULL,
    proof BLOB NOT NULL,
    received_at INTEGER NOT NULL,
    last_rewarded_at INTEGER,
    gc_scheduled_at INTEGER
);
```

---

## 4. Entity Relationships

```
┌─────────────────────────────────────────────────────────────────┐
│                         On-chain                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Fragment ─────────┬─────────────> ProofRecord                   │
│    │               │                   │                          │
│    │ 1:N           │ 1:N               │ N:1                       │
│    ▼               ▼                   ▼                          │
│  holders[]     Challenges         AccountId (holder)              │
│                                                                  │
│  RewardPoolBalance <──── 90% of post fee                          │
│         │                                                         │
│         └───────────────> claim_reward ────> holder balance       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                        Off-chain                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  StorageNode                                                     │
│       │                                                          │
│       └───> StoredShare ───> content_hash (links to Fragment)    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. State Transitions

### Fragment Lifecycle

```
       create_post_v2
           │
           ▼
    ┌──────────────┐
    │   Active     │◄─────────────────────────────┐
    │  (score ≥ θ) │                              │
    └──────┬───────┘                              │
           │ score < θ                            │ score ≥ θ
           ▼                                      │
    ┌──────────────┐                              │
    │  LowScore    │──────────────────────────────┘
    │  (reward=0)  │
    └──────┬───────┘
           │ 7+ days, holders < k
           ▼
    ┌──────────────┐
    │   Forgotten  │
    │ (unrecoverable)│
    └──────────────┘
```

### Challenge-Response State Machine

```
                    issue_challenge
                          │
                          ▼
   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
   │   Issued    │──│  Responded  │──│  Verified   │
   │             │  │             │  │  (success)  │
   └──────┬──────┘  └─────────────┘  └──────┬──────┘
          │                                  │
          │ deadline passed                  │ update ProofRecord
          ▼                                  ▼
   ┌─────────────┐                    ┌─────────────┐
   │   Expired   │                    │  Rewarded   │
   │ (no response)│                   │             │
   └──────┬──────┘                    └─────────────┘
          │
          │ failure_count++
          ▼
   ┌─────────────┐
   │   Warning   │ (failure_count ≥ 3)
   │   Flagged   │
   └─────────────┘
```

### Reward Distribution Flow

```
                    Post Created
                          │
                          ▼
              ┌──────────────────────┐
              │  Post Fee (100%)     │
              └──────────┬───────────┘
                        │
            ┌───────────┴───────────┐
            │                       │
            ▼                       ▼
    ┌───────────────┐       ┌───────────────┐
    │ RewardPool    │       │    Burn       │
    │    (90%)      │       │    (10%)      │
    └───────┬───────┘       └───────────────┘
            │
            │ 24h batch
            ▼
    ┌───────────────────────────────────────┐
    │  For each holder with successful     │
    │  proof and score ≥ threshold:        │
    │                                       │
    │  reward = base_per_byte × data_size  │
    │  pending_reward += reward            │
    └───────────────────────────────────────┘
            │
            │ claim_reward extrinsic
            ▼
    ┌───────────────┐
    │ Holder Wallet │
    │   Balance     │
    └───────────────┘
```

---

## 6. Encoding/Serialization

### On-chain (SCALE)
全てのオンチェーンデータは SCALE エンコーディング。

### Off-chain API (JSON-RPC)
```json
{
  "submit_proof": {
    "content_hash": "0x...",
    "share_index": 1,
    "share_value": "base64...",
    "proof": "base64..."
  }
}
```

### Wasm Bindings (serde + wasm-bindgen)
```typescript
interface VssSplitResult {
  commitment: Uint8Array;  // 48 bytes
  shares: VssShare[];
  proofs: Uint8Array[];    // each 48 bytes
  compressed: boolean;
}
```

---

## 7. Validation Summary

| Entity | Field | Rule |
|--------|-------|------|
| VssShare | index | 1 ≤ index ≤ n |
| VssShare | value | < BLS12-381 field modulus |
| KzgCommitment | bytes | Valid compressed G1 point |
| KzgProof | bytes | Valid compressed G1 point |
| Fragment | threshold | 1 ≤ k ≤ n |
| Fragment | fragment_count | n ≤ 16 (bounded) |
| Challenge | deadline | > issued_at |
| ProofRecord | failure_count | Flag at ≥ 3 |
| StoredShare | gc_scheduled_at | NULL or > received_at + 7 days |

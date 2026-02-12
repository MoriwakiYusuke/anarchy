# Data Model: Post Storage Migration

**Feature**: 009-post-storage-migration  
**Date**: 2026-02-10

## 概要

投稿コンテンツをオンチェーンからオフチェーン分散ストレージへ移行するためのデータモデル変更。

---

## 1. オンチェーンデータ（pallet-post）

### 変更前（現行）

```rust
/// 投稿メタデータ
struct Post<T: Config> {
    author: T::AccountId,
    content_hash: [u8; 32],
    created_at: BlockNumberFor<T>,
    parent_id: Option<u64>,
}

/// コンテンツ本文（削除対象）
type Contents<T> = StorageMap<_, Blake2_128Concat, u64, BoundedVec<u8, T::MaxContentLength>>;
```

### 変更後（V2）

```rust
/// 投稿メタデータ（変更なし）
struct Post<T: Config> {
    author: T::AccountId,
    content_hash: [u8; 32],  // MerkleRootと同一
    created_at: BlockNumberFor<T>,
    parent_id: Option<u64>,
}

/// 分散ストレージ参照情報（新規追加）
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct PostContent {
    /// MerkleRoot（断片ハッシュのマークルツリールート）
    pub root: [u8; 32],
    /// 復元に必要な最小断片数（しきい値）
    pub k: u32,
    /// 総断片数
    pub n: u32,
    /// 元データサイズ（バイト）
    pub size: u64,
}

/// コンテンツ参照ストレージ（Contents<T>を置き換え）
type ContentRefs<T> = StorageMap<_, Blake2_128Concat, u64, PostContent>;
```

### Storage変更サマリ

| Storage | 変更 | 説明 |
|---------|------|------|
| `Posts<T>` | 維持 | メタデータはそのまま |
| `Contents<T>` | **削除** | オンチェーン本文保存を廃止 |
| `ContentRefs<T>` | **新規** | MerkleRoot + k/n/sizeのみ保存 |
| `UserPosts<T>` | 維持 | 変更なし |
| `NextPostId<T>` | 維持 | 変更なし |

---

## 2. オフチェーンデータ（Storage Node）

### Fragment（断片）

```rust
/// 断片識別子
#[derive(Clone, Encode, Decode, Serialize, Deserialize)]
pub struct FragmentId {
    /// 投稿ID
    pub post_id: u64,
    /// 断片インデックス（0 〜 n-1）
    pub index: u32,
}

/// 断片データ（ディスク保存形式）
#[derive(Clone, Serialize, Deserialize)]
pub struct StoredFragment {
    /// 断片ID
    pub id: FragmentId,
    /// SSSシェアデータ
    pub data: Vec<u8>,
    /// Blake2bハッシュ（検証用）
    pub hash: [u8; 32],
    /// 保存タイムスタンプ
    pub stored_at: u64,
}
```

### ディスクレイアウト

```
{storage_root}/
└── fragments/
    └── {post_id}/
        ├── 0.bin    # Fragment index 0
        ├── 1.bin    # Fragment index 1
        ├── ...
        └── {n-1}.bin
```

---

## 3. Wasm Engine データ構造

### 入力（フロントエンド → Wasm）

```typescript
interface SplitInput {
  data: Uint8Array;  // 元コンテンツ
  k: number;         // しきい値
  n: number;         // 分割数
}
```

### 出力（Wasm → フロントエンド）

```typescript
interface SplitOutput {
  fragments: Fragment[];
  merkleRoot: Uint8Array;  // [u8; 32]
  proofs: MerkleProof[];
}

interface Fragment {
  index: number;
  data: Uint8Array;
  hash: Uint8Array;  // Blake2b hash
}

interface MerkleProof {
  index: number;
  siblings: Uint8Array[];  // 兄弟ハッシュの配列
}
```

---

## 4. RPC データ構造

### storage_uploadFragment

```typescript
// Request
{
  postId: string,     // hex encoded u64
  index: number,      // 0 to n-1
  data: string,       // hex encoded bytes
  proof: string,      // hex encoded MerkleProof
  merkleRoot: string, // hex encoded [u8; 32]
}

// Response
{
  success: boolean,
  error?: string,
}
```

### storage_getFragment

```typescript
// Request
{
  postId: string,
  index: number,
}

// Response
{
  data?: string,  // hex encoded bytes (null if not found)
  error?: string,
}
```

### storage_listHolders

```typescript
// Request
{
  postId: string,
  index: number,
}

// Response
{
  holders: string[],  // PeerIds of Storage Nodes holding this fragment
}
```

---

## 5. libp2p プロトコルメッセージ

### 既存プロトコル（変更なし）

`/anarchy/fragment/1.0.0` - Storage Node間で既に実装済み

```rust
pub enum FragmentRequest {
    Get { fragment_id: FragmentId },
    Put { fragment_id: FragmentId, data: Vec<u8> },
}

pub enum FragmentResponse {
    Data(Option<Vec<u8>>),
    Ack { success: bool, error: Option<String> },
}
```

### Blockchain Node → Storage Node 通信

同じプロトコルを使用。追加フィールドなし。

---

## 6. フロントエンドキャッシュ

### IndexedDB スキーマ

```typescript
interface CachedPost {
  postId: string;        // Primary key
  content: Uint8Array;   // 復元済みコンテンツ
  merkleRoot: string;    // 検証用
  cachedAt: number;      // timestamp
  accessCount: number;   // LRU用
}
```

### キャッシュポリシー

| 設定 | 値 |
|------|---|
| 最大容量 | 50MB |
| エントリ上限 | 1000件 |
| TTL | 7日 |
| 削除ポリシー | LRU (accessCount × recency) |

---

## 7. マイグレーション

### 戦略: 破棄 + 新規作成

開発環境のため、既存データは破棄してV2形式のみをサポート：

1. `Contents<T>` StorageMapを削除（on_runtime_upgrade）
2. `ContentRefs<T>` StorageMapを追加
3. 既存の投稿は`ContentRefs`がないため表示不可（意図的）

```rust
fn on_runtime_upgrade() -> Weight {
    // Contents<T>の全エントリを削除
    let _ = Contents::<T>::clear(u32::MAX, None);
    T::DbWeight::get().writes(1)
}
```

---

## 8. エンティティ関係図

```
┌─────────────────────────────────────────────────────────────────┐
│                         On-Chain (Substrate)                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐     1:1      ┌──────────────────┐            │
│  │    Post      │─────────────▶│  ContentRefs     │            │
│  │  (metadata)  │              │  (merkle info)   │            │
│  └──────────────┘              └──────────────────┘            │
│        │                              │                        │
│        │ N:1                          │ reference              │
│        ▼                              ▼                        │
│  ┌──────────────┐              ┌──────────────────┐            │
│  │  UserPosts   │              │  MerkleRoot      │            │
│  │  (index)     │              │  (32 bytes)      │            │
│  └──────────────┘              └──────────────────┘            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                                        │
                                        │ verify
                                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Off-Chain (Storage Nodes)                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐     1:N      ┌──────────────────┐            │
│  │   PostId     │─────────────▶│   Fragment       │            │
│  │              │              │   (SSS share)    │            │
│  └──────────────┘              └──────────────────┘            │
│                                        │                        │
│                                        │ distributed            │
│                                        ▼                        │
│                               ┌──────────────────┐              │
│                               │  Storage Node    │              │
│                               │  (holds subset)  │              │
│                               └──────────────────┘              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

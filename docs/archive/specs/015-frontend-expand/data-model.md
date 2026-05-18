# Data Model: フロントエンド拡充

**Feature**: 015-frontend-expand  
**Date**: 2026-02-25  
**Source**: spec.md Key Entities

## Overview

この機能で扱う主要エンティティとその関係を定義する。

```
┌─────────────────┐     ┌─────────────────┐
│   AccountId     │────▶│    Nickname     │
│   (Identity)    │     │  (Optional)     │
└─────────────────┘     └─────────────────┘
        │
        │ sender/recipient
        ▼
┌─────────────────┐
│ TransferRequest │
│   (送金要求)     │
└─────────────────┘

┌─────────────────┐     ┌─────────────────┐
│   Post (V2)     │────▶│   MediaRef[]    │
│   (投稿)        │     │ (メディア参照)   │
└─────────────────┘     └─────────────────┘
        │
        │ storage
        ▼
┌─────────────────┐
│  HybridShard[]  │
│ (分散シャード)   │
└─────────────────┘
```

---

## Entities

### 1. TransferRequest

**Purpose**: MORAL送金リクエストの一時的な状態管理

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `sender` | `AccountId` | 送金元 | 現在のログインユーザー |
| `recipient` | `AccountId` | 送金先 | SS58形式、有効なアドレス |
| `amount` | `bigint` | 金額 (planck単位) | > 0, <= 残高 |
| `status` | `TransferStatus` | 状態 | enum値 |
| `txHash` | `string?` | トランザクションハッシュ | 送信後に設定 |
| `error` | `string?` | エラーメッセージ | 失敗時に設定 |

```typescript
// apps/frontend/src/types/transfer.ts
export interface TransferRequest {
  sender: string        // AccountId (SS58)
  recipient: string     // AccountId (SS58)
  amount: bigint        // 1 MORAL = 1_000_000_000_000n
  status: TransferStatus
  txHash?: string
  error?: string
}

export type TransferStatus = 'idle' | 'confirming' | 'pending' | 'success' | 'error'
```

**State Transitions**:
```
idle → confirming (ユーザーが送金ボタンを押す)
confirming → pending (確認ダイアログでOK)
confirming → idle (確認ダイアログでキャンセル)
pending → success (トランザクション成功)
pending → error (トランザクション失敗)
error → idle (再試行準備)
```

---

### 2. AddressDisplay

**Purpose**: AccountIdの表示用データ

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `full` | `string` | フルAccountId | SS58形式 |
| `short` | `string` | 短縮表示 | `{先頭6文字}...{末尾4文字}` |
| `nickname` | `string?` | ニックネーム | オンチェーンから取得 |

```typescript
// apps/frontend/src/types/address.ts
export interface AddressDisplay {
  full: string         // "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
  short: string        // "5Grwva...utQY"
  nickname?: string    // "alice_anarchy"
}

// Helper function
export function formatAddress(accountId: string): AddressDisplay {
  return {
    full: accountId,
    short: `${accountId.slice(0, 6)}...${accountId.slice(-4)}`,
    nickname: undefined  // 別途ニックネームクエリで設定
  }
}
```

---

### 3. Nickname (On-chain)

**Purpose**: AccountIdに紐づく表示名

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `accountId` | `AccountId` | 所有者 | 一意のキー |
| `name` | `BoundedVec<u8, 128>` | ニックネーム | 最大128バイト (UTF-8) |

```rust
// apps/blockchain/pallets/nickname/src/lib.rs

/// Nickname storage type: AccountId → Nickname
#[pallet::storage]
pub type Nicknames<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<u8, ConstU32<128>>,
    OptionQuery
>;

/// Nickname events
#[pallet::event]
pub enum Event<T: Config> {
    /// Nickname set or updated
    NicknameSet { who: T::AccountId, nickname: Vec<u8> },
    /// Nickname cleared
    NicknameCleared { who: T::AccountId },
}

/// Nickname errors
#[pallet::error]
pub enum Error<T> {
    /// Nickname exceeds maximum length (128 bytes)
    NicknameTooLong,
    /// Nickname contains invalid UTF-8
    InvalidUtf8,
}
```

**Validation Rules**:
- 最大128バイト（UTF-8）
- 空文字列は許可（削除と同等に扱う）
- ユニーク制約なし
- 変更可能（上書き）
- 削除可能（clear_nickname）

---

### 4. MediaFile

**Purpose**: アップロード対象のメディアファイル（クライアントサイド）

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `id` | `string` | 一時識別子 | UUID |
| `file` | `File` | 元ファイル | ブラウザFile API |
| `type` | `MediaType` | 種別 | image / video |
| `size` | `number` | サイズ (bytes) | 画像100MB, 動画1GB以下 |
| `preview` | `string?` | プレビューURL | blob: URL |
| `uploadProgress` | `number` | 進捗 (0-100) | パーセント |
| `status` | `MediaUploadStatus` | 状態 | enum値 |
| `merkleRoot` | `string?` | 分散ストレージ参照 | アップロード完了後 |

```typescript
// apps/frontend/src/types/media.ts
export interface MediaFile {
  id: string                    // crypto.randomUUID()
  file: File
  type: MediaType
  size: number
  preview?: string              // URL.createObjectURL(file)
  uploadProgress: number        // 0-100
  status: MediaUploadStatus
  merkleRoot?: string           // hex encoded
  width?: number                // 画像の場合
  height?: number
}

export type MediaType = 'image' | 'video'
export type MediaUploadStatus = 'pending' | 'splitting' | 'uploading' | 'complete' | 'error'

export interface MediaConstraints {
  image: {
    maxSize: 100 * 1024 * 1024,  // 100MB
    formats: ['image/jpeg', 'image/png', 'image/gif', 'image/webp']
  },
  video: {
    maxSize: 1000 * 1024 * 1024, // 1GB
    formats: ['video/mp4', 'video/webm']
  }
}
```

**State Transitions**:
```
pending → splitting (Web Workerでhybrid_split開始)
splitting → uploading (分割完了、シャードアップロード開始)
uploading → complete (全シャードアップロード完了)
any → error (エラー発生)
```

---

### 5. MediaRef (On-chain)

**Purpose**: 投稿に紐づくメディア参照（オンチェーン）

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `merkleRoot` | `[u8; 32]` | 分散ストレージ参照 | Merkle Root |
| `mediaType` | `MediaType` | 種別 | 0=Image, 1=Video |
| `sizeBytes` | `u32` | 元サイズ | > 0 |
| `width` | `u16` | 幅 | 画像の場合 |
| `height` | `u16` | 高さ | 画像の場合 |

```rust
// apps/blockchain/pallets/post/src/lib.rs (拡張)

#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct MediaRef {
    pub merkle_root: [u8; 32],
    pub media_type: MediaType,
    pub size_bytes: u32,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum MediaType {
    Image = 0,
    Video = 1,
}
```

---

### 6. HybridSplitResult

**Purpose**: hybrid_split()の結果（wasm-engine）

| Field | Type | Description |
|-------|------|-------------|
| `shards` | `HybridShard[]` | 分割されたシャード |
| `originalLen` | `number` | 元データサイズ |
| `compressed` | `boolean` | 圧縮されたか |
| `ciphertextLen` | `number` | 暗号化後サイズ |
| `shardSize` | `number` | シャードサイズ |
| `threshold` | `number` | 復元に必要なシャード数 (k) |
| `totalShards` | `number` | 総シャード数 (n) |

```typescript
// from wasm-engine
export interface HybridSplitResult {
  shards: HybridShard[]
  originalLen: number
  compressed: boolean
  ciphertextLen: number
  shardSize: number
  threshold: number       // k
  totalShards: number     // n
  merkleRoot: Uint8Array  // 32 bytes
}

export interface HybridShard {
  index: number
  chunk: Uint8Array       // ≤ 256KB
  chunkHash: Uint8Array   // Blake2b hash
  keyShare: Uint8Array    // SSS key share
  kzgCommitment?: Uint8Array
}
```

---

## Relationships

| From | To | Relationship | Description |
|------|-----|--------------|-------------|
| Post | MediaRef | 1:N | 投稿は0〜4個のメディアを持つ |
| MediaRef | HybridShard | 1:N | メディア1個は複数シャードに分割 |
| AccountId | Nickname | 1:0..1 | アカウントは0または1個のニックネームを持つ |
| TransferRequest | AccountId | N:2 | 送金は送信者と受信者の2つのAccountIdを参照 |

---

## Indexes & Queries

### Frontend Queries
- `getNickname(accountId)`: ニックネーム取得
- `getBalance(accountId)`: 残高取得
- `listPosts(offset, limit)`: 投稿一覧

### On-chain Storage Queries
- `Nickname.nicknames(accountId)`: ニックネーム
- `Balances.account(accountId)`: 残高情報
- `Post.posts(postId)`: 投稿詳細

### Storage Node Queries
- `storage_getFragment(merkleRoot, index)`: シャード取得
- `storage_storeKzgShard(params)`: シャード保存

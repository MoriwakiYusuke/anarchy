# Data Model: 自己修復プロトコル

**Feature**: 013-slashing-repair  
**Created**: 2026-02-24

## Overview

本機能で追加・拡張するデータエンティティの定義。既存のpallet-storageストレージを拡張し、新規ストレージを最小限に抑える設計。

---

## Entities

### 1. ProofRecord（拡張）

**既存ストレージ**: `ProofRecords<T>: StorageDoubleMap<ContentHash, AccountId, ProofRecord>`

**拡張後の構造体**:
```rust
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, Default)]
pub struct ProofRecord<BlockNumber: Default> {
    /// 最終成功証明ブロック
    pub last_proved_at: BlockNumber,
    /// 連続成功回数
    pub success_count: u32,
    /// 連続失敗回数（スラッシュ判定に使用）
    pub failure_count: u32,
    /// スラッシュ済みフラグ（GC優先度に影響）
    pub slashed: bool,              // 新規
    /// 保持しているシェアのindex（1-255, 0=未設定）
    pub share_index: u8,            // 新規
}
```

**フィールド説明**:
| フィールド | 型 | 説明 | 更新タイミング |
|-----------|------|------|--------------|
| `last_proved_at` | BlockNumber | 最終成功証明ブロック | prove_holding_kzg成功時 |
| `success_count` | u32 | 連続成功回数 | prove_holding_kzg成功時 |
| `failure_count` | u32 | 連続失敗回数 | チャレンジ期限切れ時 |
| `slashed` | bool | スラッシュ済みフラグ | slash_node実行時 |
| `share_index` | u8 | 保持シェアのindex | declare_holding時 |

**マイグレーション**:
- 既存レコードは `slashed = false`, `share_index = 0` で初期化

---

### 2. FragmentState（新規）

**新規ストレージ**: `FragmentStates<T>: StorageMap<FragmentId, FragmentState>`

```rust
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
pub struct FragmentState<BlockNumber> {
    /// 現在の状態
    pub kind: FragmentStateKind,
    /// 状態変更ブロック（タイムアウト計算用）
    pub changed_at: BlockNumber,
}

#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, Default)]
pub enum FragmentStateKind {
    #[default]
    Active,
    AtRisk,
    Repairing,
    Lost,
}
```

**状態遷移ルール**:
| 現状態 | トリガー | 次状態 | 条件 |
|--------|---------|--------|------|
| Active | ホルダー減少 | AtRisk | holder_count ≤ 4 |
| AtRisk | confirm_repair | Repairing | 修復開始 |
| AtRisk | ホルダー減少 | Lost | holder_count ≤ 2 |
| Repairing | confirm_repair成功 | Active | holder_count = 5 |
| Repairing | タイムアウト | AtRisk | 60分経過 |
| Lost | 30日経過 | (削除) | GCで物理削除 |

---

### 3. RepairRewardPool（新規）

**新規ストレージ**: `RepairRewardPools<T>: StorageMap<FragmentId, Balance>`

```rust
/// 断片ごとの修復報酬プール
/// スラッシュで没収した金額が積み立てられ、修復協力者に分配
#[pallet::storage]
pub type RepairRewardPools<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    [u8; 32],  // content_hash (FragmentId)
    u128,      // Balance
    ValueQuery,
>;
```

**更新タイミング**:
- 増加: `slash_node` でノードからペナルティ没収時
- 減少: `confirm_repair` で修復協力者に報酬分配時

---

### 4. PendingRewards（既存・再利用）

**既存ストレージ**: `PendingRewards<T>: StorageMap<AccountId, Balance>`

**変更点**:
- 構造変更なし
- `claim_rewards`に引き出し下限チェック（500 MORAL）を追加
- スラッシング時に減額

**仕様書マッピング**:
- 仕様書の「AccruedRewards」 = 既存の「PendingRewards」

---

### 5. VssShare（wasm-engine、既存）

```rust
/// VSS Share（変更なし）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VssShare {
    /// Share index (1..=255)
    pub index: u8,
    /// Share value (BLS12-381 scalar, 32 bytes)
    pub value: [u8; 32],
}
```

**新規関数で使用**:
- `regenerate_share(shares, threshold, new_index, commitment)` → `(VssShare, KzgProof)`

---

## Relationships

```
┌─────────────────┐
│    PostInfo     │ (既存)
│  content_hash   │
└────────┬────────┘
         │ 1:1
         ▼
┌─────────────────┐
│ FragmentState   │ (新規)
│  kind, changed  │
└────────┬────────┘
         │ 1:N
         ▼
┌─────────────────┐       1:N      ┌─────────────────┐
│ FragmentHolders │ ─────────────▶ │  ProofRecord    │ (拡張)
│  [AccountId]    │                │  slashed, index │
└────────┬────────┘                └─────────────────┘
         │                                  │
         │                                  │ N:1
         │                                  ▼
         │                         ┌─────────────────┐
         │                         │ PendingRewards  │ (既存)
         │                         │    Balance      │
         │                         └─────────────────┘
         │ 1:1
         ▼
┌─────────────────┐
│RepairRewardPool │ (新規)
│    Balance      │
└─────────────────┘
```

---

## Validation Rules

### ProofRecord
- `share_index`: 1〜255の範囲、同一断片内で重複不可
- `failure_count`: 3回でスラッシュ発動
- `slashed`: trueの場合、報酬付与なし、GC優先度最低

### FragmentState
- `kind`: Active以外への遷移はイベント発行必須
- `changed_at`: 状態変更時に現在ブロックで更新

### RepairRewardPool
- 残高が0でもエントリは保持（過去のスラッシュ履歴）
- 分配時は均等割り（複数協力者の場合）

### PendingRewards（引き出し下限）
- `claim_rewards`は500 MORAL以上でのみ実行可能
- スラッシュ時は下限関係なく没収

---

## Migration Strategy

### ProofRecordのマイグレーション

```rust
// runtime upgrade時に実行
fn migrate_proof_records<T: Config>() {
    ProofRecords::<T>::translate::<OldProofRecord<BlockNumberFor<T>>, _>(|_k1, _k2, old| {
        Some(ProofRecord {
            last_proved_at: old.last_proved_at,
            success_count: old.success_count,
            failure_count: old.failure_count,
            slashed: false,      // 新規フィールド初期値
            share_index: 0,      // 新規フィールド初期値（未設定）
        })
    });
}
```

### share_indexの後方互換性

- `share_index = 0` は「未設定」を意味
- 既存のdeclare_holdingで設定されていない場合、次回のdeclare_holding時に設定
- GC優先度計算では `share_index = 0` を「古いシェア」として扱う

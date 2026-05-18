# Runtime API Contracts: 自己修復プロトコル

**Feature**: 013-slashing-repair  
**Created**: 2026-02-24

## Overview

ストレージノードがチェーンから情報を取得するためのRuntime API定義。
既存の`StorageRuntimeApi`を拡張。

---

## API Definitions

### 1. get_eviction_candidates

削除優先度順にソートされたホルダー候補を取得。

**Request**:
```rust
fn get_eviction_candidates(
    fragment_id: [u8; 32],  // content_hash
) -> Vec<EvictionCandidate>;
```

**Response**:
```rust
#[derive(Clone, Encode, Decode, TypeInfo)]
pub struct EvictionCandidate {
    /// ノードアカウント
    pub account_id: AccountId,
    /// 削除優先度スコア（低いほど優先的に削除）
    pub priority_score: u32,
    /// シェアindex
    pub share_index: u8,
    /// スラッシュ済みか
    pub is_slashed: bool,
    /// 最終証明ブロック
    pub last_proved_at: BlockNumber,
}
```

**優先度スコア計算**:
```
priority_score = 
    (is_slashed ? 0 : 1000) +      // スラッシュ済みは最低優先度
    (share_index <= 5 ? 0 : 100) + // 古いindex（1-5）は低優先度
    min(last_proved_at / 100, 500) // 最終証明が古いほど低優先度
```

**使用シナリオ**:
- ストレージノードがGC対象を判断
- `evict_stale_holder`呼び出し前の候補確認

---

### 2. get_fragments_with_excess_holders

ホルダー数が上限（n=5）を超えている断片一覧を取得。

**Request**:
```rust
fn get_fragments_with_excess_holders() -> Vec<(FragmentId, u32)>;
```

**Response**:
- `Vec<(fragment_id, holder_count)>` - 超過断片のリスト

**使用シナリオ**:
- ストレージノードの定期GCチェック
- ダッシュボードでの超過状態表示

---

### 3. get_fragment_state

指定断片の状態を取得。

**Request**:
```rust
fn get_fragment_state(
    fragment_id: [u8; 32],
) -> Option<FragmentStateInfo>;
```

**Response**:
```rust
#[derive(Clone, Encode, Decode, TypeInfo)]
pub struct FragmentStateInfo {
    /// 状態種別
    pub kind: FragmentStateKind,
    /// 状態変更ブロック
    pub changed_at: BlockNumber,
    /// 現在のホルダー数
    pub holder_count: u32,
    /// 修復報酬プール残高
    pub repair_pool_balance: u128,
}
```

**使用シナリオ**:
- HealthMonitorでのAtRisk検出
- ダッシュボードでの状態表示

---

### 4. get_at_risk_fragments

AtRisk状態の断片一覧を取得。

**Request**:
```rust
fn get_at_risk_fragments() -> Vec<AtRiskFragmentInfo>;
```

**Response**:
```rust
#[derive(Clone, Encode, Decode, TypeInfo)]
pub struct AtRiskFragmentInfo {
    /// 断片ID
    pub fragment_id: [u8; 32],
    /// 現在のホルダー数
    pub holder_count: u32,
    /// 必要な追加ホルダー数
    pub needed_count: u32,
    /// 修復報酬プール残高
    pub repair_pool_balance: u128,
    /// AtRisk状態になったブロック
    pub at_risk_since: BlockNumber,
}
```

**使用シナリオ**:
- HealthMonitorでの修復対象検出
- 修復優先度の判断

---

## Runtime API Declaration

```rust
sp_api::decl_runtime_apis! {
    /// Extended Storage Runtime API for self-repair protocol
    pub trait StorageRepairApi {
        /// Get eviction candidates for a fragment (holders sorted by priority)
        fn get_eviction_candidates(
            fragment_id: [u8; 32],
        ) -> Vec<EvictionCandidate>;
        
        /// Get fragments with excess holders (count > 5)
        fn get_fragments_with_excess_holders() -> Vec<([u8; 32], u32)>;
        
        /// Get fragment state
        fn get_fragment_state(
            fragment_id: [u8; 32],
        ) -> Option<FragmentStateInfo>;
        
        /// Get all AtRisk fragments
        fn get_at_risk_fragments() -> Vec<AtRiskFragmentInfo>;
    }
}
```

---

## RPC Endpoints

Runtime APIに対応するJSON-RPC エンドポイント:

| Method | Params | Returns |
|--------|--------|---------|
| `storage_getEvictionCandidates` | `fragment_id: H256` | `Vec<EvictionCandidate>` |
| `storage_getFragmentsWithExcessHolders` | (none) | `Vec<(H256, u32)>` |
| `storage_getFragmentState` | `fragment_id: H256` | `Option<FragmentStateInfo>` |
| `storage_getAtRiskFragments` | (none) | `Vec<AtRiskFragmentInfo>` |

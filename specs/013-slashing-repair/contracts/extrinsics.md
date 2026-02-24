# Extrinsic Contracts: 自己修復プロトコル

**Feature**: 013-slashing-repair  
**Created**: 2026-02-24

## Overview

pallet-storageに追加するエクストリンシック（トランザクション）の定義。

---

## Extrinsics

### 1. claim_rewards（拡張）

**既存機能**: ノードの保留報酬を引き出し

**拡張内容**: 引き出し下限チェック（500 MORAL）を追加

```rust
#[pallet::call_index(X)]
#[pallet::weight(...)]
pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResult {
    let claimer = ensure_signed(origin)?;
    
    let pending = PendingRewards::<T>::get(&claimer);
    
    // 新規: 引き出し下限チェック
    ensure!(
        pending >= T::MinWithdrawalAmount::get(),
        Error::<T>::InsufficientAccruedRewards
    );
    
    // ... 既存の引き出しロジック ...
}
```

**Parameters**:
- (none - caller's rewards)

**Events**:
- `RewardsClaimed { who: AccountId, amount: Balance }`

**Errors**:
- `InsufficientAccruedRewards` - 積み立て報酬が500 MORAL未満

---

### 2. confirm_repair（新規）

**目的**: 修復完了を報告し、報酬を受け取る

```rust
#[pallet::call_index(Y)]
#[pallet::weight(...)]
pub fn confirm_repair(
    origin: OriginFor<T>,
    fragment_id: [u8; 32],
    new_holder: T::AccountId,
    new_share_index: u8,
    kzg_proof: Vec<u8>,
) -> DispatchResult {
    let reporter = ensure_signed(origin)?;
    
    // 1. 断片がAtRiskまたはRepairing状態であることを確認
    let state = FragmentStates::<T>::get(fragment_id);
    ensure!(
        matches!(state.kind, FragmentStateKind::AtRisk | FragmentStateKind::Repairing),
        Error::<T>::FragmentNotAtRisk
    );
    
    // 2. KZG proofを検証（新シェアが正当であることを確認）
    let commitment = PostInfos::<T>::get(fragment_id)
        .ok_or(Error::<T>::PostNotFound)?
        .kzg_commitment;
    ensure!(
        Self::verify_share_proof(&commitment, new_share_index, &kzg_proof)?,
        Error::<T>::InvalidKzgProof
    );
    
    // 3. 新ホルダーをFragmentHoldersに追加
    FragmentHolders::<T>::try_mutate(fragment_id, |holders| {
        holders.try_push(new_holder.clone())
            .map_err(|_| Error::<T>::TooManyHolders)
    })?;
    
    // 4. ProofRecordを作成
    ProofRecords::<T>::insert(fragment_id, &new_holder, ProofRecord {
        share_index: new_share_index,
        ..Default::default()
    });
    
    // 5. 状態をRepairingに更新（まだActiveでない場合）
    if FragmentHolders::<T>::get(fragment_id).len() >= 5 {
        FragmentStates::<T>::mutate(fragment_id, |state| {
            state.kind = FragmentStateKind::Active;
            state.changed_at = frame_system::Pallet::<T>::block_number();
        });
    } else {
        FragmentStates::<T>::mutate(fragment_id, |state| {
            if state.kind == FragmentStateKind::AtRisk {
                state.kind = FragmentStateKind::Repairing;
                state.changed_at = frame_system::Pallet::<T>::block_number();
            }
        });
    }
    
    // 6. 修復報酬を分配（reporterに）
    let reward = RepairRewardPools::<T>::take(fragment_id);
    if reward > 0 {
        PendingRewards::<T>::mutate(&reporter, |pending| {
            *pending = pending.saturating_add(reward);
        });
    }
    
    Self::deposit_event(Event::RepairCompleted {
        fragment_id,
        new_holder,
        reporter,
        reward,
    });
    
    Ok(())
}
```

**Parameters**:
| Name | Type | Description |
|------|------|-------------|
| `fragment_id` | `[u8; 32]` | 修復対象の断片ID |
| `new_holder` | `AccountId` | 新しいホルダーのアカウント |
| `new_share_index` | `u8` | 新シェアのindex（6以上推奨） |
| `kzg_proof` | `Vec<u8>` | 新シェアの正当性を証明するKZG proof |

**Events**:
- `RepairCompleted { fragment_id, new_holder, reporter, reward }`

**Errors**:
- `FragmentNotAtRisk` - 断片がAtRisk/Repairing状態でない
- `PostNotFound` - 断片が存在しない
- `InvalidKzgProof` - KZG proofの検証失敗
- `TooManyHolders` - ホルダー数が上限を超過

---

### 3. evict_stale_holder（新規）

**目的**: ホルダー超過時に古いホルダーを削除

```rust
#[pallet::call_index(Z)]
#[pallet::weight(...)]
pub fn evict_stale_holder(
    origin: OriginFor<T>,
    fragment_id: [u8; 32],
    target: T::AccountId,
) -> DispatchResult {
    let _caller = ensure_signed(origin)?;
    
    // 1. ホルダー数が上限を超過していることを確認
    let holders = FragmentHolders::<T>::get(fragment_id);
    ensure!(holders.len() > 5, Error::<T>::NoExcessHolders);
    
    // 2. targetがホルダーであることを確認
    ensure!(holders.contains(&target), Error::<T>::TargetNotHolder);
    
    // 3. targetが本当に最低優先度であることを検証
    let candidates = Self::compute_eviction_candidates(fragment_id);
    ensure!(
        candidates.first().map(|c| &c.account_id) == Some(&target),
        Error::<T>::TargetNotLowestPriority
    );
    
    // 4. ホルダーから削除
    FragmentHolders::<T>::try_mutate(fragment_id, |holders| {
        holders.retain(|h| h != &target);
        Ok::<_, DispatchError>(())
    })?;
    
    // 5. ProofRecordを削除
    ProofRecords::<T>::remove(fragment_id, &target);
    
    Self::deposit_event(Event::HolderEvicted {
        fragment_id,
        evicted: target,
        reason: EvictReason::ExcessHolder,
    });
    
    Ok(())
}
```

**Parameters**:
| Name | Type | Description |
|------|------|-------------|
| `fragment_id` | `[u8; 32]` | 対象の断片ID |
| `target` | `AccountId` | 削除対象のアカウント |

**Events**:
- `HolderEvicted { fragment_id, evicted, reason }`

**Errors**:
- `NoExcessHolders` - ホルダー数が上限以下
- `TargetNotHolder` - 指定アカウントがホルダーでない
- `TargetNotLowestPriority` - 指定アカウントが最低優先度でない

---

## On-chain State Changes (on_finalize)

### スラッシング自動実行

`on_finalize`内でチャレンジ期限切れ処理を拡張:

```rust
fn on_finalize(block: BlockNumberFor<T>) {
    // ... 既存のチャレンジ期限切れ処理 ...
    
    // 追加: failure_count >= 3 のノードをスラッシュ
    for (content_hash, share_index) in expired_challenges.into_iter() {
        if let Some(challenge) = PendingChallenges::<T>::take(content_hash, share_index) {
            ProofRecords::<T>::mutate(content_hash, &challenge.challenged_node, |record| {
                record.failure_count = record.failure_count.saturating_add(1);
                
                // スラッシュ判定
                if record.failure_count >= 3 && !record.slashed {
                    Self::slash_node(&challenge.challenged_node, content_hash);
                    record.slashed = true;
                }
            });
            
            // FragmentState更新
            Self::update_fragment_state(content_hash);
        }
    }
}
```

### FragmentState更新

ホルダー数変更時に状態を更新:

```rust
fn update_fragment_state(fragment_id: [u8; 32]) {
    let holder_count = FragmentHolders::<T>::get(fragment_id).len();
    let current_block = frame_system::Pallet::<T>::block_number();
    
    FragmentStates::<T>::mutate(fragment_id, |state| {
        let new_kind = match holder_count {
            0..=2 => FragmentStateKind::Lost,
            3..=4 => FragmentStateKind::AtRisk,
            _ => FragmentStateKind::Active,
        };
        
        if state.kind != new_kind {
            state.kind = new_kind;
            state.changed_at = current_block;
            
            // イベント発行
            match new_kind {
                FragmentStateKind::AtRisk => {
                    Self::deposit_event(Event::FragmentAtRisk {
                        fragment_id,
                        holder_count: holder_count as u32,
                    });
                }
                FragmentStateKind::Lost => {
                    Self::deposit_event(Event::FragmentLost { fragment_id });
                }
                _ => {}
            }
        }
    });
}
```

---

## Events Summary

| Event | Description |
|-------|-------------|
| `FragmentAtRisk { fragment_id, holder_count }` | 断片がAtRisk状態に遷移 |
| `FragmentLost { fragment_id }` | 断片がLost状態に遷移（復元不可） |
| `RepairCompleted { fragment_id, new_holder, reporter, reward }` | 修復完了 |
| `NodeSlashed { operator, amount, reason }` | ノードがスラッシュされた |
| `HolderEvicted { fragment_id, evicted, reason }` | ホルダーが削除された |

---

## Errors Summary

| Error | Description |
|-------|-------------|
| `InsufficientAccruedRewards` | 積み立て報酬が引き出し下限未満 |
| `FragmentNotAtRisk` | 修復対象でない断片 |
| `InvalidKzgProof` | KZG proof検証失敗 |
| `TooManyHolders` | ホルダー数上限超過 |
| `NoExcessHolders` | ホルダー超過なし |
| `TargetNotHolder` | 削除対象がホルダーでない |
| `TargetNotLowestPriority` | 削除対象が最低優先度でない |

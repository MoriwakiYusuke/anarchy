# Interface Contract: ReactionInterface

**Feature**: 017-reaction-mining  
**Date**: 2026-02-28

このドキュメントはpallet-reactionが他パレット（特にpallet-post）に公開するインターフェースを定義する。

## Trait Definition

```rust
/// pallet-postなど他パレットからreaction関連機能を呼び出すためのインターフェース
pub trait ReactionInterface {
    /// 反応報酬プールにトークンを入金
    ///
    /// pallet-postのcreate_post_v2から呼び出され、
    /// 投稿手数料の10%を報酬プールに蓄積する。
    ///
    /// # Arguments
    /// * `amount` - 入金額（u128 planck単位）
    fn do_deposit_to_reaction_pool(amount: u128);

    /// 投稿の反応統計を取得
    ///
    /// # Arguments
    /// * `post_id` - 投稿ID
    ///
    /// # Returns
    /// * `Option<(u32, u32, u32)>` - (likes, boosts, bads) or None if post not found
    fn get_reaction_counts(post_id: u64) -> Option<(u32, u32, u32)>;

    /// 投稿のBad反応数を取得（ストレージ削除判定用）
    ///
    /// # Arguments
    /// * `post_id` - 投稿ID
    ///
    /// # Returns
    /// * `u32` - Bad反応の数
    fn get_bad_count(post_id: u64) -> u32;
}
```

## Integration with pallet-post

### Post Fee Distribution (post creation)

```rust
// In pallet-post::create_post_v2

impl<T: Config> Pallet<T> {
    pub fn create_post_v2(...) -> DispatchResult {
        // ... existing logic ...
        
        // Calculate cost
        let cost = Self::calculate_cost(content_size);
        
        // Burn cost from author
        T::NativeToken::burn_from(&author, cost, ...)?;
        
        // Distribute to pools:
        // - 90% to storage reward pool (existing)
        // - 10% to reaction reward pool (NEW)
        let storage_share = cost * 90 / 100;
        let reaction_share = cost - storage_share; // 10%
        
        T::Storage::do_deposit_to_reward_pool(storage_share.into());
        T::Reaction::do_deposit_to_reaction_pool(reaction_share.into()); // NEW
        
        // ... rest of logic ...
    }
}
```

### Config Dependency

```rust
// In pallet-post::Config

#[pallet::config]
pub trait Config: frame_system::Config {
    // ... existing ...
    
    /// Reaction Pallet for depositing reaction rewards
    type Reaction: ReactionInterface;
}
```

## Integration with pallet-storage (Future)

### Bad Reaction Threshold for GC

```rust
// Future enhancement: use bad_count for storage GC decisions
// When bad_count exceeds threshold, fragment becomes GC candidate

impl<T: Config> Pallet<T> {
    fn should_gc_fragment(content_hash: &[u8; 32]) -> bool {
        let post_id = Self::get_post_id_by_content_hash(content_hash);
        let bad_count = T::Reaction::get_bad_count(post_id);
        bad_count > T::BadThresholdForGC::get()
    }
}
```

## Implementation in pallet-reaction

```rust
impl<T: Config> ReactionInterface for Pallet<T> {
    fn do_deposit_to_reaction_pool(amount: u128) {
        ReactionRewardPool::<T>::mutate(|balance| {
            *balance = balance.saturating_add(amount);
        });
    }

    fn get_reaction_counts(post_id: u64) -> Option<(u32, u32, u32)> {
        let stats = ReactionStatsStorage::<T>::get(post_id);
        if stats.likes == 0 && stats.boosts == 0 && stats.bads == 0 {
            // Check if post exists
            if !pallet_post::Posts::<T>::contains_key(post_id) {
                return None;
            }
        }
        Some((stats.likes, stats.boosts, stats.bads))
    }

    fn get_bad_count(post_id: u64) -> u32 {
        ReactionStatsStorage::<T>::get(post_id).bads
    }
}
```

## Dependency Graph

```
┌─────────────────┐
│   pallet-post   │
│                 │
│ Config:         │
│  - Reaction: ReactionInterface
│  - Storage: StorageInterface
└────────┬────────┘
         │
         │ do_deposit_to_reaction_pool(10%)
         │ do_deposit_to_reward_pool(90%)
         ▼
┌─────────────────┐     ┌─────────────────┐
│ pallet-reaction │     │ pallet-storage  │
│                 │     │                 │
│ ReactionRewardPool    │ RewardPoolBalance
│ Reactions       │     │ FragmentMetadata
│ ReactionStats   │     │ ...             │
└─────────────────┘     └─────────────────┘
```

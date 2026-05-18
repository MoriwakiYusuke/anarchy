# Data Model: Reaction Mining

**Feature**: 017-reaction-mining  
**Date**: 2026-02-28

## Entities

### 1. ReactionType (Enum)

反応の種類を表す列挙型。

| Variant | Value | Weight | Description |
|---------|-------|--------|-------------|
| Like | 0 | 1 | いいね |
| Boost | 1 | 5 | 拡散（リツイート相当） |
| Bad | 2 | 0 | 低品質報告 |

```rust
#[derive(Clone, Copy, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
pub enum ReactionType {
    Like = 0,
    Boost = 1,
    Bad = 2,
}

impl ReactionType {
    pub fn weight(&self) -> u128 {
        match self {
            ReactionType::Like => 1,
            ReactionType::Boost => 5,
            ReactionType::Bad => 0,
        }
    }
}
```

---

### 2. Reaction (Struct)

投稿への反応を表す構造体。

| Field | Type | Description |
|-------|------|-------------|
| reactor | AccountId | 反応者のアカウントID |
| reaction_type | ReactionType | 反応の種類 |
| pow_nonce | u64 | PoW証明のnonce |
| cpu_power | u64 | 計算パワー指標（ハッシュレート） |
| created_at | BlockNumber | 反応時刻（ブロック番号） |

```rust
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
#[scale_info(skip_type_params(T))]
pub struct Reaction<T: Config> {
    pub reactor: T::AccountId,
    pub reaction_type: ReactionType,
    pub pow_nonce: u64,
    pub cpu_power: u64,
    pub created_at: BlockNumberFor<T>,
}
```

---

### 3. ReactionStats (Struct)

投稿ごとの反応統計。高速カウント取得用。

| Field | Type | Description |
|-------|------|-------------|
| likes | u32 | Like数 |
| boosts | u32 | Boost数 |
| bads | u32 | Bad数 |
| total_weight | u128 | 累計報酬重み |

```rust
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default, PartialEq, Eq)]
pub struct ReactionStats {
    pub likes: u32,
    pub boosts: u32,
    pub bads: u32,
    pub total_weight: u128,
}
```

---

### 4. DifficultyState (Struct)

動的難易度調整用の状態。

| Field | Type | Description |
|-------|------|-------------|
| current | u8 | 現在の難易度（leading zero bits数） |
| last_adjusted | BlockNumber | 最後に調整したブロック |
| recent_count | u32 | 調整ウィンドウ内の反応数 |

```rust
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct DifficultyState<BlockNumber> {
    pub current: u8,
    pub last_adjusted: BlockNumber,
    pub recent_count: u32,
}
```

---

## Storage

### On-Chain Storage (pallet-reaction)

| Storage | Type | Key | Value | Description |
|---------|------|-----|-------|-------------|
| Reactions | StorageDoubleMap | (post_id, reactor) | Reaction | 反応レコード |
| ReactionStats | StorageMap | post_id | ReactionStats | 投稿ごとの統計 |
| UserReactionCount | StorageMap | AccountId | u32 | ユーザーの総反応数 |
| ReactionRewardPool | StorageValue | - | u128 | 報酬プール残高 |
| CurrentDifficulty | StorageValue | - | u8 | 現在のPoW難易度 |
| ReactionHistory | StorageMap | BlockNumber | u32 | ブロックごとの反応数 |
| TotalReactions | StorageValue | - | u64 | 総反応数 |

```rust
/// 反応レコード (post_id, reactor) -> Reaction
#[pallet::storage]
pub type Reactions<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat, u64,          // post_id
    Blake2_128Concat, T::AccountId, // reactor
    Reaction<T>,
    OptionQuery,
>;

/// 投稿ごとの反応統計
#[pallet::storage]
pub type ReactionStatsStorage<T: Config> = StorageMap<
    _,
    Blake2_128Concat, u64,  // post_id
    ReactionStats,
    ValueQuery,
>;

/// ユーザーの総反応数
#[pallet::storage]
pub type UserReactionCount<T: Config> = StorageMap<
    _,
    Blake2_128Concat, T::AccountId,
    u32,
    ValueQuery,
>;

/// 反応報酬プール残高
#[pallet::storage]
pub type ReactionRewardPool<T: Config> = StorageValue<_, u128, ValueQuery>;

/// 現在のPoW難易度
#[pallet::storage]
pub type CurrentDifficulty<T: Config> = StorageValue<_, u8, ValueQuery>;

/// ブロックごとの反応数（難易度調整用）
#[pallet::storage]
pub type ReactionHistory<T: Config> = StorageMap<
    _,
    Blake2_128Concat, BlockNumberFor<T>,
    u32,
    ValueQuery,
>;

/// 総反応数
#[pallet::storage]
pub type TotalReactions<T: Config> = StorageValue<_, u64, ValueQuery>;
```

---

## Relationships

```
┌─────────────────────────────────────────────────────────────────┐
│                         pallet-post                              │
│  ┌─────────────┐                                                 │
│  │    Post     │                                                 │
│  │  post_id    │◀────────────────────┐                          │
│  │  author     │                     │                          │
│  └─────────────┘                     │                          │
└─────────────────────────────────────────────────────────────────┘
                                       │
                                       │ (post_id)
                                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                       pallet-reaction                            │
│  ┌─────────────┐      ┌───────────────────┐                     │
│  │  Reaction   │      │  ReactionStats    │                     │
│  │  reactor    │      │  likes            │                     │
│  │  type       │      │  boosts           │                     │
│  │  pow_nonce  │      │  bads             │                     │
│  │  cpu_power  │      │  total_weight     │                     │
│  │  created_at │      └───────────────────┘                     │
│  └─────────────┘                                                 │
│         │                                                        │
│         │ (reactor)                                              │
│         ▼                                                        │
│  ┌─────────────────────┐    ┌─────────────────────┐             │
│  │ UserReactionCount   │    │ ReactionRewardPool  │             │
│  │ (per user)          │    │ (global)            │             │
│  └─────────────────────┘    └─────────────────────┘             │
└─────────────────────────────────────────────────────────────────┘
                                       │
                                       │ (10% of post fee)
                                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                       pallet-balances                            │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ NativeToken ($moral) - fungible::Mutate for reward payout   ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

---

## State Transitions

### Reaction Lifecycle

```
┌──────────┐     react()      ┌──────────┐
│  None    │ ───────────────▶ │ Recorded │
└──────────┘                  └──────────┘
                                    │
                                    │ (immutable after creation)
                                    ▼
                              No further state changes
                              (reactions are permanent)
```

### Difficulty Adjustment

```
┌─────────────┐
│  Current    │
│  Difficulty │
└─────────────┘
       │
       │ on_finalize() every ADJUSTMENT_INTERVAL blocks
       ▼
┌─────────────────────────────────────────────────────────┐
│ Calculate recent_rate vs target_rate                     │
│ If recent > target: difficulty += adjustment             │
│ If recent < target: difficulty -= adjustment             │
│ Clamp to [MIN_DIFFICULTY, MAX_DIFFICULTY]               │
└─────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────┐
│    New      │
│  Difficulty │
└─────────────┘
```

---

## Validation Rules

| Rule | Description | Error |
|------|-------------|-------|
| V1 | 投稿が存在すること | PostNotFound |
| V2 | 同一ユーザーが同一投稿に未反応であること | AlreadyReacted |
| V3 | PoW証明が難易度を満たすこと | InvalidProof |
| V4 | チャレンジブロックが有効期限内であること | ChallengeExpired |
| V5 | チャレンジブロックが存在すること | BlockNotFound |
| V6 | 反応者が投稿者と異なること (optional) | CannotReactToOwnPost |

---

## Genesis Configuration

```rust
#[pallet::genesis_config]
pub struct GenesisConfig<T: Config> {
    /// 初期報酬プール残高 (10,000,000 MORAL = 10_000_000 * 10^12 planck)
    pub initial_reward_pool: u128,
    /// 初期難易度 (leading zero bits)
    pub initial_difficulty: u8,
}

impl<T: Config> Default for GenesisConfig<T> {
    fn default() -> Self {
        Self {
            initial_reward_pool: 10_000_000_000_000_000_000u128, // 10M MORAL
            initial_difficulty: 16, // ~65536 hashes average
        }
    }
}
```

# Runtime API Contract: Reaction

**Feature**: 017-reaction-mining  
**Date**: 2026-02-28

このドキュメントはpallet-reactionのRuntime APIを定義する。

## Runtime API Definition

```rust
sp_api::decl_runtime_apis! {
    /// Reaction Pallet Runtime API
    ///
    /// RPCから反応情報をクエリするためのAPI
    pub trait ReactionApi {
        /// 投稿の反応統計を取得
        fn get_reaction_stats(post_id: u64) -> Option<ReactionStatsInfo>;

        /// ユーザーが特定の投稿に反応済みかどうかを確認
        fn has_reacted(post_id: u64, account: AccountId) -> bool;

        /// ユーザーの反応履歴を取得（ページネーション付き）
        fn get_user_reactions(
            account: AccountId,
            offset: u32,
            limit: u32,
        ) -> Vec<UserReactionInfo>;

        /// 現在のPoW難易度を取得
        fn get_current_difficulty() -> u8;

        /// 報酬プール残高を取得
        fn get_reward_pool_balance() -> u128;

        /// PoWチャレンジを生成（クライアント用）
        fn generate_challenge(
            block_number: BlockNumber,
            account: AccountId,
        ) -> Option<[u8; 32]>;
    }
}
```

## Response Types

### ReactionStatsInfo

```rust
/// 反応統計情報（Runtime API用）
#[derive(Clone, Debug, Encode, Decode, TypeInfo, PartialEq, Eq)]
pub struct ReactionStatsInfo {
    /// Like数
    pub likes: u32,
    /// Boost数
    pub boosts: u32,
    /// Bad数
    pub bads: u32,
    /// 累計報酬重み
    pub total_weight: u128,
}
```

### UserReactionInfo

```rust
/// ユーザー反応情報（Runtime API用）
#[derive(Clone, Debug, Encode, Decode, TypeInfo, PartialEq, Eq)]
pub struct UserReactionInfo {
    /// 投稿ID
    pub post_id: u64,
    /// 反応種別
    pub reaction_type: ReactionType,
    /// 反応時刻（ブロック番号）
    pub created_at: BlockNumber,
}
```

## RPC Methods

以下のRPCメソッドを実装する（PAPI経由でアクセス）:

| Method | Request | Response | Description |
|--------|---------|----------|-------------|
| `reaction_getStats` | `{ post_id: u64 }` | `ReactionStatsInfo` | 投稿の反応統計 |
| `reaction_hasReacted` | `{ post_id: u64, account: string }` | `boolean` | 反応済みチェック |
| `reaction_getUserReactions` | `{ account: string, offset: u32, limit: u32 }` | `UserReactionInfo[]` | ユーザー反応履歴 |
| `reaction_getDifficulty` | `{}` | `u8` | 現在の難易度 |
| `reaction_getRewardPool` | `{}` | `string` (u128) | 報酬プール残高 |
| `reaction_generateChallenge` | `{ block_number: u32, account: string }` | `string` (hex) | PoWチャレンジ |

## Extrinsics

### react

投稿に反応する。

```rust
#[pallet::call_index(0)]
#[pallet::weight(T::DbWeight::get().reads_writes(4, 4))]
pub fn react(
    origin: OriginFor<T>,
    post_id: u64,
    reaction_type: ReactionType,
    block_number: BlockNumberFor<T>,
    nonce: u64,
    cpu_power: u64,
    reward_dest: Option<T::AccountId>,
) -> DispatchResult;
```

| Field | Type | Description |
|-------|------|-------------|
| `post_id` | u64 | 反応対象の投稿ID |
| `reaction_type` | ReactionType | 反応種別（Like/Boost/Bad） |
| `block_number` | BlockNumber | PoWチャレンジの基準ブロック |
| `nonce` | u64 | PoWマイニング結果 |
| `cpu_power` | u64 | 計算パワー指標（ハッシュレート） |
| `reward_dest` | Option\<AccountId\> | 報酬受取先（ステルスアドレス対応） |

**Events:**
- `Reacted { who, post_id, reaction_type, reward_amount }`

**Errors:**
- `PostNotFound` - 投稿が存在しない
- `AlreadyReacted` - 既に反応済み
- `InvalidProof` - PoW証明が無効
- `ChallengeExpired` - チャレンジが期限切れ
- `BlockNotFound` - チャレンジブロックが存在しない
- `CannotReactToOwnPost` - 自分の投稿には反応不可（optional）

## PAPI Usage Example

```typescript
import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'

const client = createClient(getWsProvider('ws://127.0.0.1:9944'))
const api = client.getUnsafeApi()

// 1. 難易度取得
const difficulty = await api.call.reactionApi.get_current_difficulty()

// 2. チャレンジ生成
const blockNumber = await api.query.System.Number()
const challenge = await api.call.reactionApi.generate_challenge(
  blockNumber,
  userAccount
)

// 3. PoWマイニング（WebWorker）
const { nonce, hashrate } = await mineReaction(challenge, difficulty)

// 4. react送信
const tx = api.tx.Reaction.react(
  postId,
  { type: 'Like' },
  blockNumber,
  BigInt(nonce),
  BigInt(hashrate),
  null // reward_dest (default: author)
)

await tx.signAndSubmit(signer)

// 5. 統計取得
const stats = await api.call.reactionApi.get_reaction_stats(postId)
console.log(`Likes: ${stats.likes}, Boosts: ${stats.boosts}`)
```

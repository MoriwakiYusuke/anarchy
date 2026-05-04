# Pallet Reaction

PoW-based reaction mining for posts in the Anarchy decentralized SNS.

## Overview

This pallet enables users to react to posts (Like/Bad) with client-side PoW proof verification. Post authors receive MORAL token rewards from the reaction reward pool.

## Features

- **Reaction Types**: Like (positive, pays author reward) and Bad (negative, no author reward, still PoW-gated). Both are forwarded to `pallet-popularity` via `PopularityInterface::on_reaction` (Like → `Like`, Bad → `Dislike`) so the popularity score reflects total engagement (Reddit-style N/M model).
- **PoW Verification**: Client mines nonce, pallet verifies leading-zero-bits
- **Dynamic Difficulty**: Adjusts based on network reaction rate
- **Reward Distribution**: Authors receive fixed 1 MORAL per Like reaction (capped by pool balance); Bad reactions do not pay a reward
- **Foreground Enforcement**: Client-side Page Visibility API ensures mining only in active tabs
- **Stealth Recipients**: Planned feature (not yet implemented, awaiting pallet-stealth)

## Configuration

```rust
impl pallet_reaction::Config for Runtime {
    type NativeToken = Balances;
    type BaseDifficulty = ConstU8<16>;       // Initial difficulty
    type MinDifficulty = ConstU8<8>;          // Floor
    type MaxDifficulty = ConstU8<32>;         // Ceiling
    type ChallengeValidity = ConstU64<100>;   // Blocks until challenge expires
    type TargetReactionRate = ConstU32<10>;   // Target reactions per block
    type AdjustmentWindow = ConstU64<100>;    // Blocks for difficulty averaging
    type AdjustmentDivisor = ConstU32<4>;     // Smoothing factor
}
```

## Extrinsics

### `react(post_id, reaction_type, block_number, nonce, cpu_power, stealth_recipient)`

Submit a reaction with PoW proof.

**Arguments:**
- `post_id`: Target post identifier
- `reaction_type`: Like | Bad
- `block_number`: Block used for challenge generation
- `nonce`: PoW nonce satisfying difficulty
- `cpu_power`: Reported hashrate (affects reward calculation)
- `stealth_recipient`: Optional stealth address for reward

**Errors:**
- `AlreadyReacted`: User already reacted to this post
- `ChallengeExpired`: Block number too old (> ChallengeValidity blocks)
- `InvalidProof`: PoW nonce doesn't satisfy difficulty
- `BlockNotFound`: Referenced block doesn't exist

## Storage

| Item | Type | Description |
|------|------|-------------|
| `Reactions` | `DoubleMap<post_id, account> -> Reaction` | Individual reaction records |
| `ReactionStatsStorage` | `Map<post_id> -> ReactionStats` | Aggregated stats per post |
| `ReactionRewardPool` | `u128` | Available rewards (in planck) |
| `CurrentDifficulty` | `u8` | Current PoW difficulty |
| `ReactionHistory` | `Map<block> -> u32` | Reactions per block |
| `TotalReactions` | `u64` | Lifetime reaction count |

## Events

- `ReactionCreated { post_id, reactor, reaction_type, reward_paid }`
- `RewardPoolDeposit { amount }`
- `DifficultyAdjusted { old_difficulty, new_difficulty }`

## Integration

### Depositing to Reward Pool

Other pallets (e.g., pallet-post) can deposit to the reaction reward pool:

```rust
<pallet_reaction::Pallet<T> as pallet_reaction::ReactionInterface>::do_deposit_to_reaction_pool(amount);
```

### Querying Reaction Stats

```rust
let counts = <pallet_reaction::Pallet<T> as pallet_reaction::ReactionInterface>::get_reaction_counts(post_id);
let bad_count = <pallet_reaction::Pallet<T> as pallet_reaction::ReactionInterface>::get_bad_count(post_id);
```

## Client-Side Mining

The frontend performs PoW mining in a Web Worker:

1. Fetch challenge: `getReactionChallenge(client, api, postId, userAddress)`
2. Mine nonce: Web Worker computes `blake2b(challenge || nonce)` until leading zeros ≥ difficulty
3. Submit: `submitReaction(api, signer, { postId, reactionType, nonce, challengeBlock })`

See `apps/frontend/src/workers/crypto.ts` for the mining implementation.

## Testing

```bash
# Run pallet unit tests
cargo test -p pallet-reaction

# Run with all features
cargo test -p pallet-reaction --all-features
```

## License

MIT

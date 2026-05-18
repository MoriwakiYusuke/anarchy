# Anarchy 経済モデル (TSTS) 実装計画

> **対応設計**: [`docs/economic_model_proposal.md`](proposal.md)
> **対象**: M0 (現状) → M1 (TSTS) のフル移行
> **総工数**: 8〜10 営業日 (1.5〜2 週間, single dev)
> **CLAUDE.md Compatibility Policy 準拠**: 既存 testnet データは破棄して chainspec 再生成

---

## 0. マイルストーン全体

| Phase | 内容 | 工数 | ブランチ戦略 |
|---|---|---:|---|
| **P0** | 設計レビュー・パラメータ最終化 | 0.5d | spec only |
| **P1** | Block reward 3-way + tail emission | 0.5d | `feature/econ-block-fanout` |
| **P2** | EIP-1559 base fee | 1.5d | `feature/econ-eip1559` |
| **P3** | Storage reward 式改訂 + 投稿/DM 分配再構成 | 1.0d | `feature/econ-pool-rebalance` |
| **P4** | `pallet_storage_stake` 新規 + slashing 改訂 | 3.0d | `feature/econ-storage-stake` |
| **P5** | Reaction 動的 γ + reactor decay + lock | 1.5d | `feature/econ-reaction-dynamic` |
| **P6** | Stealth reward 配線 | 0.5d | `feature/econ-stealth-wired` |
| **P7** | Faucet cap | 0.25d | `feature/econ-faucet-cap` |
| **P8** | Governance (`pallet_parameters`) | 1.5d | `feature/econ-governance` |
| **P9** | E2E + シミュレーション再検証 + monitoring | 1.0d | `feature/econ-validation` |

各 Phase は **独立 PR** とし、order 依存があるもの (P3 が P1 に依存等) のみマージ順を明示。

---

## P0. 設計レビュー (0.5d)

### 0.1 Decision points (要ユーザ確認)

| 項目 | 推奨 | 代替 |
|---|---|---|
| TX 手数料 0 原則の撤回 | **撤回** (EIP-1559 採用) | 0 維持 (この場合 P2 不要、§5 spam 攻撃解は別途必要) |
| Tail emission 値 | 0.5 MORAL | 0.1 MORAL (より低インフレ) |
| Block reward fanout | 50/30/20 | 60/25/15, 70/20/10 |
| Storage stake 額 | 10 MORAL/GB | 1 / 50 / 100 |
| Reactor lock | 0.1 MORAL × 24h | lock なし (Sybil 攻撃容認) |
| Governance 方式 | multisig → token-weighted | PoW miner top-K vote |

### 0.2 testnet 想定パラメータ

mainnet 推奨値の 1/100 〜 1/10 で testnet を回す:
- Block reward: 0.5 MORAL (mainnet 5)
- Storage stake: 0.1 MORAL/GB (mainnet 10)
- Faucet cap: 1,000 MORAL (mainnet 100,000)

---

## P1. Block Reward 3-way fan-out + Tail Emission (0.5d)

### 1.1 変更ファイル

```
apps/blockchain/pallets/block_reward/src/lib.rs   (改修)
apps/blockchain/runtime/src/lib.rs                 (改修)
apps/blockchain/pallets/block_reward/src/tests.rs  (改修)
```

### 1.2 pallet_block_reward 改修

**追加 Config**:
```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type Currency: Currency<Self::AccountId>;
    #[pallet::constant] type InitialReward: Get<BalanceOf<Self>>;
    #[pallet::constant] type TailEmission: Get<BalanceOf<Self>>;       // 新規
    #[pallet::constant] type HalvingPeriod: Get<BlockNumberFor<Self>>;
    #[pallet::constant] type MaxHalvings: Get<u32>;
    #[pallet::constant] type MinerSharePermill: Get<Permill>;          // 新規
    #[pallet::constant] type StorageSharePermill: Get<Permill>;        // 新規
    #[pallet::constant] type ReactionSharePermill: Get<Permill>;       // 新規

    type AuthorOrigin: FindAuthor<Self::AccountId>;
    type StoragePoolSink: pallet_storage::StorageInterface<...>;       // 新規
    type ReactionPoolSink: pallet_reaction::ReactionInterface;         // 新規
}
```

**`current_reward` 改訂**:
```rust
pub fn current_reward(n: BlockNumberFor<T>) -> BalanceOf<T> {
    let halving_period: u128 = T::HalvingPeriod::get().saturated_into();
    let block_n: u128 = n.saturated_into();
    let halvings = (block_n / halving_period) as u32;

    let initial: u128 = T::InitialReward::get().saturated_into();
    let tail: u128 = T::TailEmission::get().saturated_into();

    let halved = if halvings >= T::MaxHalvings::get() { 0 } else { initial >> halvings };
    halved.max(tail).saturated_into()
}
```

**`on_finalize` を 3-way mint に**:
```rust
fn on_finalize(n: BlockNumberFor<T>) {
    let author = T::AuthorOrigin::find_author(...) else { return };
    let total = Self::current_reward(n);
    if total.is_zero() { return; }

    let miner_share = T::MinerSharePermill::get().mul_floor(total);
    let storage_share = T::StorageSharePermill::get().mul_floor(total);
    let reaction_share = total - miner_share - storage_share;

    T::Currency::deposit_creating(&author, miner_share);
    T::StoragePoolSink::do_deposit_to_reward_pool(storage_share.saturated_into());
    T::ReactionPoolSink::do_deposit_to_reaction_pool(reaction_share.saturated_into());

    Self::deposit_event(Event::BlockRewardSplit { author, miner: miner_share, storage: storage_share, reaction: reaction_share });
}
```

### 1.3 Runtime 設定

```rust
parameter_types! {
    pub const InitialBlockReward: Balance = 5_000_000_000_000;
    pub const TailEmission:       Balance = 500_000_000_000;
    pub const HalvingPeriod: BlockNumber = 4_204_800;
    pub const MaxHalvings: u32 = 64;
    pub MinerShare:    Permill = Permill::from_percent(50);
    pub StorageShare:  Permill = Permill::from_percent(30);
    pub ReactionShare: Permill = Permill::from_percent(20);
}
impl pallet_block_reward::Config for Runtime {
    type Currency = Balances;
    type InitialReward = InitialBlockReward;
    type TailEmission = TailEmission;
    type HalvingPeriod = HalvingPeriod;
    type MaxHalvings = MaxHalvings;
    type MinerSharePermill = MinerShare;
    type StorageSharePermill = StorageShare;
    type ReactionSharePermill = ReactionShare;
    type AuthorOrigin = PowAuthorAdapter;
    type StoragePoolSink = Storage;
    type ReactionPoolSink = Reaction;
}
```

### 1.4 テスト

```
apps/blockchain/pallets/block_reward/src/tests.rs:
  - test_three_way_split_at_block_zero
  - test_tail_emission_after_max_halvings
  - test_halving_then_tail
  - test_miner_no_storage_no_reaction_when_total_zero (ensure no spurious deposits)
```

### 1.5 acceptance

- `cargo test -p pallet-block-reward` 全 pass
- 統合テスト `apps/blockchain/tests/integration/` で 1000 block 経過後の σ_storage / σ_reaction 増加を確認

---

## P2. EIP-1559 Base Fee (1.5d)

### 2.1 戦略

新規 pallet を作らず、`pallet_transaction_payment` を改修する。Substrate の `MultiplierUpdate` インターフェースに EIP-1559 風 multiplier を実装。

### 2.2 変更ファイル

```
apps/blockchain/runtime/src/lib.rs                          (改修)
apps/blockchain/runtime/src/eip1559.rs                       (新規)
apps/blockchain/pallets/post/src/lib.rs                      (extrinsic で base_fee × bytes 焼却)
apps/blockchain/pallets/messaging/src/lib.rs                 (同上)
```

### 2.3 BaseFee 状態

`pallet_transaction_payment::NextFeeMultiplier` を流用するか、独立 `BaseFeeStorage` を持つ。Anarchy 文脈では post/DM が主トラフィックなので、**独立 storage** が clean:

```rust
// runtime/src/eip1559.rs
#[frame_support::pallet]
pub mod base_fee_pallet {
    #[pallet::storage]
    pub type BaseFee<T> = StorageValue<_, Balance, ValueQuery>;

    #[pallet::storage]
    pub type GasUsedThisBlock<T> = StorageValue<_, u32, ValueQuery>;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        #[pallet::constant] type GasTargetBytesPerBlock: Get<u32>;
        #[pallet::constant] type BaseFeeMin: Get<Balance>;
        #[pallet::constant] type BaseFeeMax: Get<Balance>;
        #[pallet::constant] type BaseFeeAdjMaxBumpPermill: Get<Permill>;
    }

    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_: BlockNumberFor<T>) {
            let used = GasUsedThisBlock::<T>::take();
            let target = T::GasTargetBytesPerBlock::get();
            let cur = BaseFee::<T>::get().max(T::BaseFeeMin::get());

            // adj = 1 + (used/target - 1) / 8
            let adj = if used >= target {
                let over = used - target;
                cur.saturating_add(cur * over as u128 / (target as u128 * 8))
            } else {
                let under = target - used;
                cur.saturating_sub(cur * under as u128 / (target as u128 * 8))
            };
            let bumped = clamp(adj, T::BaseFeeMin::get(), T::BaseFeeMax::get());
            BaseFee::<T>::put(bumped);
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn record_gas(bytes: u32) {
            GasUsedThisBlock::<T>::mutate(|g| *g = g.saturating_add(bytes));
        }
        pub fn current_base_fee() -> Balance {
            BaseFee::<T>::get().max(T::BaseFeeMin::get())
        }
    }
}
```

### 2.4 Post pallet で適用

```rust
// pallets/post/src/lib.rs (create_post)
let base_fee = T::BaseFee::current_base_fee();   // 新規 trait
let base_fee_burn = base_fee.saturating_mul(total_size.into());
let storage_tip = T::PostByteTip::get().saturating_mul(total_size.into());
let total_cost = T::PostBaseCost::get() + storage_tip + base_fee_burn;

// burn full cost
T::NativeToken::burn_from(&who, total_cost, ...)?;

// distribute (excluding base_fee_burn which stays burned)
let distributable = total_cost - base_fee_burn;
let storage_share = T::PostStorageSharePermill::get().mul_floor(distributable);
let reaction_share = T::PostReactionSharePermill::get().mul_floor(distributable);
// burn share = distributable − storage − reaction (left burned implicitly)
T::Storage::do_deposit_to_reward_pool(storage_share);
T::Reaction::do_deposit_to_reaction_pool(reaction_share);

// Record gas usage for next block's base_fee adjustment
T::BaseFee::record_gas(total_size as u32);
```

DM (`pallets/messaging/src/lib.rs`) も同様。

### 2.5 Runtime 設定

```rust
parameter_types! {
    pub const GasTargetBytesPerBlock: u32 = 50_000;
    pub const BaseFeeMin: Balance = 100;          // 1e-10 MORAL/byte
    pub const BaseFeeMax: Balance = 100_000_000_000; // 0.1 MORAL/byte
    pub BaseFeeAdjMaxBump: Permill = Permill::from_parts(125_000);
    pub const PostByteTip: Balance = 800_000_000;   // 0.0008 MORAL/byte
    pub const PostBaseCostNew: Balance = 50_000_000_000_000; // 50 MORAL
    pub PostStorageShare: Permill = Permill::from_percent(50);
    pub PostReactionShare: Permill = Permill::from_percent(20);
    pub PostBurnShare: Permill = Permill::from_percent(30);  // implicit
}
```

### 2.6 テスト

```
apps/blockchain/runtime/src/eip1559_tests.rs:
  - base_fee_at_target_utilization_stays_constant
  - base_fee_doubles_after_~6_blocks_at_2x_utilization (compounding 1.125)
  - base_fee_halves_after_~6_blocks_at_0_utilization
  - base_fee_clamped_at_max
  - base_fee_clamped_at_min
```

### 2.7 acceptance

- E2E: `apps/frontend/e2e/specs/post-spam-defense.spec.ts` で 100 連投時の base_fee 上昇を観察
- Grafana に `base_fee` メトリクス追加

---

## P3. Storage Reward 式改訂 + 投稿/DM 分配再構成 (1.0d)

### 3.1 変更ファイル

```
apps/blockchain/pallets/storage/src/rewards.rs    (改修)
apps/blockchain/pallets/storage/src/lib.rs        (Config 追加)
apps/blockchain/runtime/src/lib.rs                 (Config 値設定)
```

### 3.2 rewards.rs 改訂

```rust
pub fn calculate_reward_v2(
    data_size: u32,
    base_reward_per_byte: u128,
    score: u64,
    threshold: u64,
    storage_pool_balance: u128,
    storage_pool_target: u128,
    node_bond: u128,
    total_active_bond: u128,
) -> u128 {
    if score < threshold { return 0; }

    let base = base_reward_per_byte.saturating_mul(data_size as u128);

    // pool ratio (枯渇時は線形減衰)
    let pool_ratio_ppm = if storage_pool_balance >= storage_pool_target {
        1_000_000
    } else {
        (storage_pool_balance.saturating_mul(1_000_000) / storage_pool_target.max(1)) as u128
    };

    // bond_share^0.5 — integer sqrt approximation
    let bond_share_ppm = if total_active_bond > 0 {
        node_bond.saturating_mul(1_000_000) / total_active_bond
    } else {
        1_000_000
    };
    let bond_factor_ppm = sqrt_ppm(bond_share_ppm); // sqrt(1e6 * x) / 1e3 ≈ sqrt(x) in ppm

    base
        .saturating_mul(pool_ratio_ppm) / 1_000_000
        .saturating_mul(bond_factor_ppm) / 1_000_000
}

fn sqrt_ppm(x_ppm: u128) -> u128 {
    // Newton iteration for integer sqrt
    let x = x_ppm;
    if x == 0 { return 0; }
    let mut z = (x + 1) / 2;
    let mut y = x;
    while z < y { y = z; z = (x / z + z) / 2; }
    y
}
```

### 3.3 Storage Pallet Config 追加

```rust
#[pallet::config]
pub trait Config: ... {
    // 既存に追加
    #[pallet::constant] type StoragePoolTarget: Get<u128>;
    type StakeProvider: pallet_storage_stake::BondInfo<Self::AccountId>;  // P4 で追加
}
```

### 3.4 Runtime

```rust
parameter_types! {
    pub const StoragePoolTarget: u128 = 500_000 * MORAL_UNITS;
    pub const BaseRewardPerByteV2: Balance = 5_000_000_000;  // 5 nano-MORAL/byte
}
```

### 3.5 Post / DM 分配比率

`pallets/post/src/lib.rs` および `pallets/messaging/src/lib.rs` で 80/10/10 → 50/20/30 へ。Config 経由で governance-mutable に:

```rust
#[pallet::config]
pub trait Config: ... {
    #[pallet::constant] type StorageSharePermill: Get<Permill>;
    #[pallet::constant] type ReactionSharePermill: Get<Permill>;
    // BurnShare = 1 - storage - reaction (implicit)
}
```

### 3.6 テスト

```
pallets/storage/src/tests.rs:
  - reward_with_full_pool_full_bond_share
  - reward_with_half_pool_linear_decay
  - reward_with_quarter_bond_returns_half_factor
  - reward_zero_when_pool_empty
```

---

## P4. pallet_storage_stake (新規) + slashing 改訂 (3.0d)

### 4.1 新規 pallet

```
apps/blockchain/pallets/storage_stake/Cargo.toml
apps/blockchain/pallets/storage_stake/src/lib.rs
apps/blockchain/pallets/storage_stake/src/types.rs
apps/blockchain/pallets/storage_stake/src/mock.rs
apps/blockchain/pallets/storage_stake/src/tests.rs
```

### 4.2 主要 storage

```rust
#[pallet::storage]
pub type Bonds<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Bond<T>>;

pub struct Bond<T: Config> {
    pub amount: BalanceOf<T>,
    pub declared_capacity_bytes: u64,
    pub bonded_at: BlockNumberFor<T>,
    pub release_requested_at: Option<BlockNumberFor<T>>,
    pub consecutive_failures: u32,
}

#[pallet::storage]
pub type TotalActiveBond<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;
```

### 4.3 主要 extrinsic

```rust
pub fn bond(origin, declared_capacity_bytes: u64) -> DispatchResult {
    let who = ensure_signed(origin)?;
    ensure!(declared_capacity_bytes >= T::MinDeclaredCapacity::get(), Error::CapacityTooLow);

    let gb = (declared_capacity_bytes / 1_073_741_824).max(1);
    let bond_amount = T::BondPerGB::get().saturating_mul(gb.into());

    T::Currency::reserve(&who, bond_amount)?;
    Bonds::<T>::insert(&who, Bond { amount: bond_amount, declared_capacity_bytes, ... });
    TotalActiveBond::<T>::mutate(|t| *t = t.saturating_add(bond_amount));
    Self::deposit_event(Event::Bonded { who, amount: bond_amount });
    Ok(())
}

pub fn request_release(origin) -> DispatchResult { ... }
pub fn finalize_release(origin) -> DispatchResult { ... }  // 7d 後
```

### 4.4 slashing 改訂

`pallet_storage::do_slash_node` を改訂し、bond から slash:

```rust
pub fn do_slash_node(node: T::AccountId, content_hash: ContentHash) -> DispatchResult {
    // ProofRecord を slashed=true にする (既存)
    ProofRecords::mutate(...);

    // Bond から slash
    let bond_info = T::StakeProvider::get_bond(&node).ok_or(Error::NoBond)?;
    let fails = bond_info.consecutive_failures.saturating_add(1);
    let slash_pct_ppm = (fails.min(T::MaxConsecutiveFailures::get()) * T::SlashRatePerFailPpm::get()) as u128;
    let slash_amount = bond_info.amount.saturating_mul(slash_pct_ppm.min(1_000_000)) / 1_000_000;

    let burn_share = slash_amount * T::SlashBurnSharePermill::get().deconstruct() / 1_000_000;
    let repair_share = slash_amount - burn_share;

    T::StakeProvider::slash_bond(&node, slash_amount, burn_share, repair_share);

    // RepairRewardPool に repair_share を deposit (既存ロジック)
    RepairRewardPools::<T>::mutate(content_hash, |p| *p = p.saturating_add(repair_share));

    Self::deposit_event(Event::NodeSlashed { node, content_hash, slash_amount, burn_share });
    Ok(())
}
```

### 4.5 Runtime

```rust
parameter_types! {
    pub const BondPerGB: Balance = 10 * MORAL;
    pub const MinDeclaredCapacity: u64 = 1_073_741_824;  // 1 GB
    pub const BondReleaseDelay: BlockNumber = 100_800;   // 7d
    pub const SlashRatePerFailPpm: u32 = 5_000;
    pub const MaxConsecutiveFailures: u32 = 10;
    pub SlashBurnShare: Permill = Permill::from_percent(30);
}
impl pallet_storage_stake::Config for Runtime { ... }
impl pallet_storage::Config for Runtime {
    ...
    type StakeProvider = StorageStake;
}
```

### 4.6 ノード登録フロー変更

`pallet_storage::register_node` 拡張:
```rust
pub fn register_node(origin, http_url: ..., peer_id: ..., pow_nonce: ...) -> DispatchResult {
    let who = ensure_signed(origin)?;
    // 必須: bond 済みであること
    ensure!(T::StakeProvider::has_bond(&who), Error::NotBonded);
    // 既存ロジック
    ...
}
```

### 4.7 テスト

```
pallets/storage_stake/src/tests.rs:
  - bond_locks_balance
  - bond_below_min_capacity_fails
  - request_release_then_7d_then_finalize
  - cannot_finalize_before_delay
  - slash_consumes_bond_proportionally
  - slash_burn_repair_split_correctly
  - 10_consecutive_failures_full_slash
```

---

## P5. Reaction γ + decay + lock (1.5d)

### 5.1 変更ファイル

```
apps/blockchain/pallets/reaction/src/lib.rs       (改修)
apps/blockchain/pallets/reaction/src/tests.rs     (改修)
apps/blockchain/runtime/src/lib.rs                 (Config)
```

### 5.2 動的 γ + decay 実装

```rust
#[pallet::storage]
pub type ReactorReactionCount<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

#[pallet::storage]
pub type ReactorLocks<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ReactorLock<T>>;

pub struct ReactorLock<T> {
    pub amount: BalanceOf<T>,
    pub locked_at: BlockNumberFor<T>,
}

// 新 extrinsic
pub fn lock_for_rewards(origin, amount: BalanceOf<T>) -> DispatchResult { ... }
pub fn unlock(origin) -> DispatchResult { /* 24h 経過後 */ }
```

`process_reaction` での払い出し:
```rust
let n = ReactorReactionCount::<T>::get(&reactor);
let decay_ppm = sqrt_ppm(1_000_000 * REACTOR_DECAY_K / (REACTOR_DECAY_K + n as u128));
let pool = ReactionRewardPool::<T>::get();
let total_supply = pallet_balances::Pallet::<T>::total_issuance().saturated_into::<u128>();
let gamma_ppm = (pool.saturating_mul(1_000_000) / total_supply.max(1)).min(T::GammaMaxPpm::get() as u128);

let work_factor_ppm = sqrt_ppm(...);  // PoW work amount

let raw_reward = gamma_ppm * decay_ppm * work_factor_ppm / 1_000_000_000_000;

// 5%/day cap
let daily_cap = pool * T::DailyPayoutCapPpm::get() as u128 / 1_000_000;
let granted = raw_reward.min(daily_cap.saturating_sub(DailyPayoutAccumulator::<T>::get()));

// reactor must have lock if reward > 0
if granted > 0 {
    ensure!(ReactorLocks::<T>::contains_key(&reactor), Error::NoLock);
}

ReactionRewardPool::<T>::mutate(|p| *p = p.saturating_sub(granted));
T::NativeToken::deposit_creating(&reactor, granted.saturated_into());
ReactorReactionCount::<T>::mutate(&reactor, |c| *c = c.saturating_add(1));
```

### 5.3 Runtime

```rust
parameter_types! {
    pub const GammaMaxPpm: u32 = 10_000;          // 1%
    pub const ReactorDecayK: u32 = 100;
    pub const DailyPayoutCapPpm: u32 = 50_000;    // 5%
    pub const ReactorLockMin: Balance = 100_000_000_000;  // 0.1 MORAL
    pub const ReactorLockDuration: BlockNumber = 2880;     // 24h
}
```

### 5.4 テスト

```
- reward_zero_when_no_lock
- gamma_drops_when_pool_drains
- reactor_decay_after_100_reactions
- daily_cap_blocks_pool_drain
- lock_unlock_cycle
- sybil_with_min_lock_burns_through_balance (シミュレーション風 invariant test)
```

---

## P6. Stealth Reward 配線 (0.5d)

### 6.1 pallet_stealth に reward pool 追加

```rust
// pallets/stealth/src/lib.rs
#[pallet::storage]
pub type StealthRewardPool<T: Config> = StorageValue<_, u128, ValueQuery>;

#[pallet::storage]
pub type RecipientReceiveCount<T: Config> = StorageMap<_, Blake2_128Concat, EphemeralPubkey, u32, ValueQuery>;

impl<T: Config> pallet_messaging::StealthRewardInterface for Pallet<T> {
    fn do_deposit_to_stealth_reward_pool(amount: u128) {
        StealthRewardPool::<T>::mutate(|p| *p = p.saturating_add(amount));
    }
}

pub fn claim_stealth_reward(origin, recipient_pubkey: EphemeralPubkey, signature: [u8; 64]) -> DispatchResult {
    // 署名検証
    // 受信回数 × γ_stealth で payout
    let count = RecipientReceiveCount::<T>::get(&recipient_pubkey);
    let pool = StealthRewardPool::<T>::get();
    let payout = ...;
    StealthRewardPool::<T>::mutate(|p| *p = p.saturating_sub(payout));
    T::Currency::deposit_creating(&signer, payout);
    Ok(())
}
```

### 6.2 messaging で受信回数を increment

```rust
// pallets/messaging/src/lib.rs (dispatch_dm)
T::StealthReward::do_deposit_to_stealth_reward_pool(stealth_share);
T::StealthReward::increment_recipient_count(&recipient_stealth);  // 新規 trait method
```

### 6.3 Runtime

```rust
impl pallet_messaging::Config for Runtime {
    type StealthReward = Stealth;  // ← () から変更
    ...
}
```

---

## P7. Faucet Cap (0.25d)

### 7.1 変更

```rust
// pallets/faucet/src/lib.rs
#[pallet::storage]
pub type TotalMinted<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

#[pallet::config]
pub trait Config: ... {
    #[pallet::constant] type TotalCap: Get<BalanceOf<Self>>;
}

pub fn submit_pow_claim(origin, ...) -> DispatchResult {
    let total_minted = TotalMinted::<T>::get();
    ensure!(total_minted < T::TotalCap::get(), Error::FaucetCapReached);
    // 既存 PoW 検証
    ...
    T::NativeToken::deposit_creating(&who, T::RewardAmount::get());
    TotalMinted::<T>::mutate(|t| *t = t.saturating_add(T::RewardAmount::get()));
    Ok(())
}
```

### 7.2 Runtime

```rust
parameter_types! {
    pub const FaucetTotalCap: Balance = 100_000 * MORAL;
}
impl pallet_faucet::Config for Runtime { type TotalCap = FaucetTotalCap; ... }
```

---

## P8. Governance (`pallet_parameters`) (1.5d)

### 8.1 戦略

Substrate の `pallet_parameters` (`paritytech/polkadot-sdk` 既存) を導入し、各経済パラメータを on-chain mutable に。

```rust
// runtime/src/lib.rs
use pallet_parameters::{define_aggregrated_parameters, define_parameters};

define_parameters!(
    pub EconomicParameters = {
        MinerSharePermill: Permill = Permill::from_percent(50),
        StorageSharePermill: Permill = Permill::from_percent(30),
        ReactionSharePermill: Permill = Permill::from_percent(20),
        PostStorageSharePermill: Permill = Permill::from_percent(50),
        PostReactionSharePermill: Permill = Permill::from_percent(20),
        DmStorageSharePermill: Permill = Permill::from_percent(50),
        DmStealthSharePermill: Permill = Permill::from_percent(20),
        BondPerGB: Balance = 10 * MORAL,
        SlashRatePerFailPpm: u32 = 5_000,
        GammaMaxPpm: u32 = 10_000,
        BaseFeeMin: Balance = 100,
        BaseFeeMax: Balance = 100_000_000_000,
    }
);
```

### 8.2 multisig 移行

`pallet_collective` (technical committee, 3-of-5) で `set_parameter(EconomicParameters::MinerSharePermill, new_value)` を発議できるようにする。

### 8.3 Phase 2 (将来)

- `pallet_referenda` で token-weighted vote (匿名性とのトレードオフは spec で別途)
- または PoW miner top-K vote (現 GRANDPA election と同じスキーム)

---

## P9. E2E + 検証 + Monitoring (1.0d)

### 9.1 シミュレーション再検証

`/tmp/anarchy_sim.py` を新パラメータで再実行 (P0 で確定した値で) し、`/tmp/anarchy_sim_output_final.txt` として保存。design doc の §5 数値表を更新。

### 9.2 E2E テスト

```
apps/frontend/e2e/specs/economic-base-fee.spec.ts (新規):
  - 100 連投時の base_fee 上昇を観察
  - 1 時間放置後に base_fee 戻る

apps/blockchain/tests/integration/economic_e2e.sh (新規):
  - chain_spec で 5 ノード起動
  - 1k post → σ_storage 増加確認
  - 100 reaction → reactor lock 必要確認
  - storage node bond → register → slash → bond 減少確認
```

### 9.3 Monitoring

```
infra/grafana-dashboards/anarchy-economy.json (新規):
  - chart: σ_storage, σ_reaction, σ_stealth (時系列)
  - chart: base_fee
  - chart: total_issuance / total_burned (累積)
  - chart: miner_revenue per block
  - alert: σ_reaction < 1k MORAL
  - alert: base_fee > 0.05 MORAL/byte (混雑検知)
```

### 9.4 ドキュメント更新

- `docs/economic_parameters.md` を新パラメータで全面書き換え
- `docs/blockchain_logic.md` の経済セクションを更新
- `docs/CHANGELOG.md` に経済モデル v2 移行を記録

### 9.5 chainspec 再生成

```bash
./scripts/build-mainnet-chainspec.sh --tsts-v1
```

旧 testnet データは破棄。`apps/blockchain/node/src/chain_spec.rs` の genesis を新値で:
```rust
INITIAL_REWARD_POOL: 100_000 * MORAL  // (was 1_000_000)
INITIAL_REACTION_REWARD_POOL: 100_000 * MORAL  // (was 10_000_000)
INITIAL_FAUCET_MINTED: 0
INITIAL_BASE_FEE: 10_000  // 1e-8 MORAL/byte
```

---

## 10. PR 提出順 (推奨)

```
PR #N+1: P1 Block reward 3-way + tail emission   → main にマージ
PR #N+2: P3 Storage reward 式改訂 (P1 の StoragePoolSink を活用)
PR #N+3: P4 pallet_storage_stake (P3 の StakeProvider を充足)
PR #N+4: P2 EIP-1559 base fee
PR #N+5: P5 Reaction γ + decay + lock
PR #N+6: P6 Stealth reward 配線
PR #N+7: P7 Faucet cap
PR #N+8: P8 Governance (parameters pallet)
PR #N+9: P9 E2E + monitoring + chainspec
```

各 PR は `superpowers:test-driven-development` に従い red→green→refactor。`superpowers:requesting-code-review` を経て main に。

---

## 11. リスク・ロールバック計画

### 11.1 バグ発見時

P1〜P9 はすべて **chainspec 再生成で破棄可能なテストネット段階で検証**してから mainnet 投入する。mainnet 投入後にバグ発見 → governance で当該パラメータを safe value に戻す → fix の sudo upgrade。

### 11.2 経済攻撃発見時

EIP-1559 base_fee は upper bound (BaseFeeMax) があるため**最悪ケースでも有限**。Sybil 大量発生 → governance で `GammaMaxPpm` を一時的に 0 に → 報酬流出停止 → patched runtime upgrade。

### 11.3 Quad √ 計算エラー

整数 sqrt の Newton 反復は `no_std` 環境で fallible。テストで `sqrt_ppm(0)`, `sqrt_ppm(1_000_000)`, `sqrt_ppm(u128::MAX)` の境界を必ずカバー。

---

## 12. Acceptance Criteria

mainnet 投入の OK 条件:

- [ ] P1〜P9 全 PR merge 済み
- [ ] testnet で 30 日間 連続稼働 (再起動なし)
- [ ] シミュレーション再現結果が design doc §5 と乖離 ±10% 以内
- [ ] E2E spam attack scenario で base_fee が 1h 以内に min まで戻る
- [ ] Storage node 50 個以上が bond → register → 1 週間 stable proof
- [ ] Sybil 10k reactor 試行で reactor_lock 経由で attacker MORAL 残高 < 0 を確認
- [ ] Grafana ダッシュボード稼働、警報発火テスト済み
- [ ] Multisig (3-of-5) で `set_parameter` 1 回以上成功
- [ ] `docs/economic_parameters.md` がコード実態と一致

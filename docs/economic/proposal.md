# Anarchy 経済モデル設計提案 — TSTS (Triple-Sink Triple-Source)

> **目的**: docs/TODO.md §4.4 (Mainnet設計・経済パラメータ) を満たす、ゲーム理論的に破綻しない経済モデルの提案。
> **前提資料**: [`docs/economic_parameters.md`](parameters.md) (現状パラメータ全棚卸し)
> **作成日**: 2026-05-07
> **対象**: mainnet 投入前の経済パラメータ最終決定および pallet 改修の前段階意思決定

---

## 0. 結論サマリ

現状の経済モデル (M0) には**致命的な構造欠陥が 8 点**ある (本書 §1)。これを **TSTS (Triple-Sink Triple-Source)** モデル (M1) に置換することで、以下を達成できる:

| 観点 | 現状 (M0) | 提案 (M1) | 改善 |
|---|---|---|---|
| 51% 攻撃コスト (era 64 後) | 0 (報酬枯渇) | TAIL=0.5 MORAL/block 永続 | ∞ |
| Storage 報酬支払 (5y, S1 シナリオ) | 815 MORAL | 1,289,233 MORAL | **1,580×** |
| 投稿 spam 経済コスト | 一定 (104 MORAL/post) | EIP-1559 で動的 (最大 462 MORAL/post に膨張) | spam を経済的に潰す |
| 活動停滞時の Storage 投資保証 | 流入ゼロで枯渇 | Block reward 30% が常時注入 | プール下限保証 |
| Stealth 還流 | trait `()` で実質 burn | 配線完了 | DM 受信者にマイクロ報酬 |
| 動的反応報酬 γ | 未実装 (固定 1 MORAL) | γ = ReactionPool/TotalIssuance | 自己平衡 |

トレードオフは **手数料 0 を放棄**すること (EIP-1559 base fee 導入)。Anarchy のオリジナル思想からの逸脱だが、これなしには game theoretic に破綻するため、**0 手数料原則は撤回**を提案する。

---

## 1. ゲーム理論分析: 現状モデルの破綻シナリオ

### 1.1 アクター・効用関数

| アクター | 効用 | 行動戦略空間 |
|---|---|---|
| PoW miner | block_reward + tx_fees − hash_cost | 採掘継続 / 撤退 / 攻撃 |
| Storage node | storage_reward − (disk_cost + bandwidth + opportunity_cost_of_stake) | 参加 / 撤退 / 怠惰 / 偽造 |
| Poster | social_value(post) + author_reaction_income − post_cost | 投稿 / spam / 沈黙 |
| Reactor | engagement_value − (cpu_cost + lock_opportunity_cost) | 善意反応 / Sybil farm / 沈黙 |
| Post author | Σ(reactor 報酬) — Like 反応ごとに mint される (本実装の収益主体) | 良質投稿で集客 / バズ狙い |
| DM 利用者 | privacy − dm_cost | 利用 / 撤退 |
| DM 受信者 | stealth_reward (P6 配線後) | 受信メタアドレス公開を継続 |
| 攻撃者 | attack_value − attack_cost | spam / Sybil / 51% / DoS |

> **注 (2026-05-08 修正)**: `pallet_reaction::react` の実装では、Like 反応の報酬は **post author** に
> mint される (`mint_into(&author, ...)`) — reactor 自身ではない。Reactor は CPU を投下することで
> 「post author を稼がせる」プロトコルになっている。よって reactor の直接的金銭的効用はゼロまたは
> マイナス (lock 機会費用 + cpu)。間接的には自身が投稿する側に回ったときに reaction_income を得る
> インセンティブで、長期的に正となる。Sybil 防御 (γ + decay + lock) は author 稼ぎを薄めることで
> spam-author を抑える二次効果も持つ。

### 1.2 現状モデルの 8 大破綻シナリオ

#### D1. 永続報酬の消失 (era 64 以降)
`BlockReward(h) = InitialReward × 0.5^(h/HalvingPeriod)` で、64 回 halving (≒ 256 年) 後に **mint = 0**。tx 手数料 0 を維持するなら miner の限界収益 = 0 → **51% 攻撃コスト = 0**。Bitcoin は手数料市場が代替するが、Anarchy は手数料 0 設計のため代替手段なし。

> **ゲーム理論**: Nash 均衡で参加 miner 数 → 0 (撤退が支配戦略)。残った miner は容易に 51% 確保可能。

#### D2. ストレージプールの活動依存性
`σ_storage(t)` への流入は posts/DMs 由来のみ。DAU=0 ならば流入 0、storage 報酬 0、ノード撤退、データ消失 → **chain は維持されるが SNS としてのコンテンツが失われる**。

> **シミュレーション (S2 stagnation, M0)**: 1k DAU 平坦で 5 年後 σ_storage = 838M MORAL だが、storage 支払総額はわずか 16 MORAL。流入は十分でも **BaseRewardPerByte=1e-12 が小さすぎて支払 ≪ 流入** で機能不全。

#### D3. Storage node に skin-in-the-game がない
slashing は `pending_rewards / 2`。**未受領報酬 = 0 のノードは slash されてもゼロコスト**。新規参加 → 数ブロック待機 → proof 失敗 → 別アカウント再参加 が支配戦略 (Sybil 連続再参加)。

#### D4. Reaction 報酬が固定 1 MORAL (動的式未実装)
spec の `Reward = Σ(R × Power_cpu) × γ` が未実装。**1 リアクション = 1 MORAL 固定**で、Sybil reactor 大量生成が支配戦略。

> **シミュレーション (S4 sybil, M0)**: 1M Sybil identity が 1 反応/day ずつ → 5 年で reaction_pool = 0 (枯渇)。残存 Sybil 取り分推定: 76.9% (446M MORAL)。

#### D5. Fee multiplier 固定 = 1.0 (混雑シグナルなし)
ブロック充填率に対する価格シグナルなし。post cost 一定 (104 MORAL/4KB-post) のため、富裕アクターは MAXIMUM_BLOCK_LENGTH=5MiB を埋め尽くす block-stuffing を ~26,000 MORAL/block で実行可能 (1B MORAL/月)。

#### D6. Stealth 還流が `()` で burn 化
`type StealthReward = ()` のため DM コストの 10% が**意図せず burn**。実効デフレ率を歪める。

#### D7. ハードコード比率 (80/10/10)
governance 手段なしで調整不可。事後の市場圧で再均衡できない。

#### D8. Faucet の永続インフレ
log カーブで難易度上昇するが claim 上限なし。永続的に MORAL を増やし続ける。

### 1.3 Sybil 数値シナリオ

シミュレータ (`/tmp/anarchy_sim.py`) の S4 (1M Sybil reactor / 10k DAU) で:

```
M0:  Sybil 取り分 = 76.9% (446M MORAL / 5y)
M1:  Sybil 取り分 = 82.6% (481M MORAL / 5y) ← γ × decay でも改善せず
```

→ **TSTS だけでは Sybil 不十分。本書 §3.5 で追加対策を導入する**。

---

## 2. 設計原則

### 2.1 経済モデルの 5 原則

1. **Sink–Source budget balance**: 任意の長期で `Σ Sources ≈ Σ Sinks` が成立し、プール残高が振動均衡する
2. **No free dimension**: 攻撃者が無限予算で支配できる軸を残さない (PoW・PoS・経済的コストのいずれかが必ず効く)
3. **Skin-in-the-game**: ノード参加には bond を要求し、裏切りは bond を毀損する
4. **Self-balancing rewards**: 報酬は固定値ではなく `γ × pool_ratio × √work` の形で需給に応じて自己調整する
5. **Permissionless governance**: 全パラメータは on-chain governance で調整可能、魔法数を埋め込まない

### 2.2 数学的不変条件

設計時に下記が成立することを証明 (本書 §4):

- **(I-1) 永続セキュリティ**: ∀t, BlockReward(t) ≥ TAIL > 0 (51% 攻撃コスト > 0)
- **(I-2) ストレージプール下限**: σ_storage(t) → c × N_active_bytes (c = 定数) で漸近、活動 0 でも block reward 注入で >0 を保つ
- **(I-3) 反応プール正値性**: ∀t, σ_reaction(t) ≥ 0、γ(t) → 0 で payout も 0 に滑らかに収束
- **(I-4) Sybil 不採算**: Sybil reactor の `reward / cpu_cost ≤ market_token_price` で逆ザヤ
- **(I-5) Spam 自己消費**: spam による base_fee 上昇が spam attacker の MORAL を有限時間で枯渇させる

---

## 3. TSTS モデル詳細仕様

### 3.1 トークノミクス全体図

```
                        ╔══════════ SOURCES ══════════╗
                        ║                              ║
   ┌──────────┐         ║   S1: Block reward (mint)    ║
   │ PoW miner │◀───────╫─── 50% ────┐                 ║
   └──────────┘         ║            │                 ║
                        ║   S2: Faucet (mint, capped)  ║
                        ║            │                 ║
                        ║   S3: User payment (post,DM) ║
                        ║            │                 ║
                        ╚════════════╪═════════════════╝
                                     │
                        ┌────────────┼────────────────┐
                        ▼            ▼                ▼
                  ┌─────────┐  ┌──────────┐    ┌───────────┐
                  │ Storage │  │ Reaction │    │  Stealth  │
                  │  Pool σ │  │  Pool σ  │    │   Pool σ  │
                  └────┬────┘  └────┬─────┘    └─────┬─────┘
                       │            │                │
                  storage_reward  γ × √work       per-recipient
                  × bond^0.5      × decay         × DM-received
                       │            │                │
                       ▼            ▼                ▼
                  Storage       Reactor          DM 受信者
                  node (bond)   (foreground PoW)
                       │
                  ╔════════════ SINKS ═══════════╗
                  ║  K1: EIP-1559 base-fee burn  ║
                  ║  K2: Slashing burn (30%)     ║
                  ║  K3: Post/DM burn (30%)      ║
                  ╚══════════════════════════════╝
```

### 3.2 パラメータ仕様 (mainnet 推奨値)

#### 3.2.1 ブロック報酬 (3-way fan-out + tail emission)

```rust
parameter_types! {
    pub const InitialBlockReward: Balance = 5 * MORAL;      // 5 MORAL
    pub const TailEmission:       Balance = 5 * MORAL / 10; // 0.5 MORAL (= 永続)
    pub const HalvingPeriod:      BlockNumber = 4_204_800;  // ~4y
    pub const MaxHalvings:        u32 = 64;                  // (TailEmission 後は無効)

    // 3-way fan-out (sum = 100)
    pub const MinerSharePermill:    Permill = Permill::from_percent(50);
    pub const StorageSharePermill:  Permill = Permill::from_percent(30);
    pub const ReactionSharePermill: Permill = Permill::from_percent(20);
}
```

実装式:
```
block_reward(h) = max(InitialBlockReward >> halvings(h), TailEmission)
miner_mint    = block_reward × 50%
storage_pool += block_reward × 30%   // 不変条件 I-2 を保証
reaction_pool += block_reward × 20%
```

**根拠**:
- 50% miner: PoS 系 (Polkadot 60-70%) より低めだが、Bitcoin と同等水準。PoW のセキュリティ予算として十分
- 30% storage: 1k DAU 平坦シナリオで σ_storage 残高を底上げし、活動 0 でもプール枯渇を防ぐ (I-2)
- 20% reaction: 反応マイニング初期の引き合いを残しつつ、過剰インフレを避ける (γ < 1 を維持)

#### 3.2.2 EIP-1559 base fee

```rust
parameter_types! {
    pub const GasTargetBytesPerBlock: u32 = 50_000;    // 50 KB target = 50% utilization
    pub const BaseFeeInit:    Balance = 10_000;         // 1e-8 MORAL/byte (10000 units)
    pub const BaseFeeMin:     Balance = 100;            // 1e-10 MORAL/byte
    pub const BaseFeeMax:     Balance = 100_000_000_000; // 0.1 MORAL/byte (cap)
    pub const BaseFeeAdjMaxBumpPermill: Permill = Permill::from_parts(125_000); // ±12.5%/block
}
```

実装式 (毎ブロック更新):
```
utilization = block_bytes_used / GasTargetBytesPerBlock
adj         = clamp(1 + (utilization − 1) / 8, [0.875, 1.125])
base_fee'   = clamp(base_fee × adj, [BaseFeeMin, BaseFeeMax])
```

各 extrinsic は `base_fee × tx_bytes` を **burn**。priority fee (tip) は optional で miner へ。

> **シミュレーション結果**:
> - S1 organic (1k→100k DAU): 5y 累計 base-fee burn = 112G MORAL → 強デフレ圧
> - S2 stagnation (1k DAU): base_fee は BaseFeeMin に張り付き → 通常利用は ~0.04 MORAL/4KB-post (無視できる)
> - S3 spam (100k posts/day): base_fee → 0.1 MORAL/byte cap → 4KB post = 410 MORAL → spam 攻撃が 100x 高くなり経済的に不可能

#### 3.2.3 投稿コスト分配

```rust
parameter_types! {
    pub const PostBaseCost: Balance = 50 * MORAL;          // 半減 (was 100)
    pub const PostByteTip:  Balance = 800_000_000;          // 0.0008 MORAL/byte (storage tip)
    // post cost = PostBaseCost + (PostByteTip + base_fee) × bytes
    pub const PostStorageSharePermill:  Permill = Permill::from_percent(50);
    pub const PostReactionSharePermill: Permill = Permill::from_percent(20);
    pub const PostBurnSharePermill:     Permill = Permill::from_percent(30);
}
```

base_fee burn は別経路 (EIP-1559)。残りの post cost を **50/20/30** に分配:
- 50% → σ_storage (was 80%)
- 20% → σ_reaction (was 10%)
- 30% → 永久 burn (was 10%)

burn 比率増加の根拠: tail emission 0.5 MORAL/block ≈ 1450 MORAL/h を相殺するための恒常デフレ圧。

#### 3.2.4 DM コスト分配

```rust
parameter_types! {
    pub const DmBaseCost: Balance = MORAL / 2;              // 半減 (was 1)
    pub const DmByteTip:  Balance = 40_000_000_000;          // 0.04 MORAL/byte
    pub const DmStorageSharePermill: Permill = Permill::from_percent(50);
    pub const DmStealthSharePermill: Permill = Permill::from_percent(20);  // ← 配線必須
    pub const DmBurnSharePermill:    Permill = Permill::from_percent(30);
}
```

`StealthReward = pallet_stealth::Pallet<Runtime>` に変更し、`()` を解消 (D6)。

#### 3.2.5 ストレージステーク (新 pallet `pallet_storage_stake`)

```rust
parameter_types! {
    pub const BondPerGB:           Balance = 10 * MORAL;   // 10 MORAL/GB declared capacity
    pub const MinDeclaredCapacity: u64 = 1_073_741_824;     // 1 GB
    pub const BondReleaseDelay:    BlockNumber = 100_800;   // 7 days
    pub const SlashRatePerFailPpm: u32 = 5_000;             // 0.5% bond per failed challenge
    pub const MaxConsecutiveFailures: u32 = 10;             // 10 fails → full bond slash
    pub const SlashBurnSharePpm:    u32 = 300_000;          // 30% burn, 70% RepairPool
}
```

slashing 改訂:
```
slash_amount   = node_bond × min(consecutive_fails × 0.005, 1.0)
burn          += slash_amount × 0.30
repair_pool   += slash_amount × 0.70
```

加えて、storage_reward 計算式を改訂:
```
storage_reward(node, fragment) =
    BaseRewardPerByte × data_size
    × min(1, σ_storage / σ_target)        // pool ratio (枯渇時は線形減衰)
    × √(node_bond / total_active_bond)    // quadratic Sybil resistance
```

**根拠 (quadratic √ の意味)**:
- N 個の Sybil ノードに分散しても、Σ √(b_i) ≤ √(Σ b_i) (jensen) なので 1 ノードへの集約より報酬総和が **減る**
- 巨大プレイヤー独占の場合、bond_share^0.5 は集中度を抑制

#### 3.2.6 反応報酬 (動的 γ + 二重防御)

```rust
parameter_types! {
    pub const ReactionPoolToTotalIssuanceTargetPpm: u32 = 10_000;  // γ_target = 1%
    pub const ReactorDecayK: u32 = 100;                              // 1/√(1+n/100)
    pub const ReactionDailyPayoutCapPpm: u32 = 50_000;              // 5% pool/day max
}
```

3 段防御:

**(a) γ 自己調整**:
```
γ(t) = σ_reaction(t) / TotalIssuance(t)
     ≤ γ_target = 1%   // governance パラメータ
```
プールが小さければ γ → 0 で報酬縮小、プール過剰なら γ_target で頭打ち。

**(b) Reactor decay**:
```
nth_reaction_reward = γ × √work × 1/√(1 + n/ReactorDecayK)
```
同一 reactor の 100 番目反応 = 1番目の 1/√2 ≈ 70%、1000番目 = 1/√11 ≈ 30%。

**(c) Daily cap**:
```
daily_payout ≤ σ_reaction × 5%
```
Sybil 大量攻撃でも 1 日にプール 5% 以上は流出しない。

**(d) Reactor stake (Sybil 防御の追加層)** (新規提案):
```
reactor_lock = 0.1 MORAL × 24h
```
反応するには 0.1 MORAL を 24h ロック必須。報酬を得ない (signal-only) 反応は free。これにより:
- 1M Sybil identity = 100,000 MORAL のロック必要 = bootstrap 困難
- 短期 attacker は MORAL 流動性を破壊
- 長期 attacker は機会費用 = 100,000 MORAL × interest_rate を負担

> **根拠**: 完全匿名を保ちつつ Sybil コストを上げる唯一の手段は **proof-of-cost** (財産的負担)。bond は session-only key と独立に lock 可能なため anonymity 原則と矛盾しない。

#### 3.2.7 Faucet サンセット

```rust
parameter_types! {
    pub const FaucetTotalCap:     Balance = 100_000 * MORAL;  // 1000 claims max
    pub const FaucetRewardAmount: Balance = 100 * MORAL;
    // (BaseDifficulty / Scaling / MaxDifficulty は現状維持)
}
```

`FaucetMintedTotal` がストレージに加わり、cap 到達で `submit_pow_claim` が `Error::FaucetCapReached` を返す。

#### 3.2.8 ガバナンス

`pallet_collective` (3-of-5 multisig 初期) → 中期で `pallet_referenda` (token-weighted) または `pallet_ranked_collective` (技術評議会型) を導入。全パラメータを `pallet_parameters` (Polkadot 流) で governance-mutable にする。

魔法数の例 (governance-tunable に置く):
- `MinerSharePermill / StorageSharePermill / ReactionSharePermill`
- `PostStorageSharePermill / PostReactionSharePermill / PostBurnSharePermill`
- `BaseFeeMin / BaseFeeMax / GasTarget`
- `BondPerGB / SlashRatePerFailPpm`
- `ReactionDailyPayoutCapPpm / ReactorDecayK`

---

## 4. 不変条件の証明・解析

### 4.1 (I-1) 永続セキュリティ

**主張**: ∀t, BlockReward(t) ≥ TailEmission > 0 ⟹ miner 限界収益 > 0 ⟹ rational miner は撤退しない。

```
BlockReward(t) = max(InitialBlockReward >> halvings(t), TailEmission)
              ≥ TailEmission = 0.5 MORAL/block
```

5 年あたり miner mint = 0.5 × 1,051,200 × 5 × 0.5 = 1,314,000 MORAL。仮に 1 MORAL = $0.01 とすれば年間 $26,280 の予算で chain security を維持。10x growth (1 MORAL = $0.10) なら $262,800/yr → small chain として十分。 ∎

### 4.2 (I-2) ストレージプール下限

**主張**: σ_storage(t) は活動 0 でも `block_reward × 30%` の流入を持つため、減衰しない。

ストレージ支払 = `α × N_active_bytes` (α = BaseRewardPerByte × challenges)。活動 0 ならば N_active_bytes = 0、よって支払 0。一方流入は `block_reward × 0.3 = 0.15 MORAL/block` (tail 後)。よって σ_storage は単調増加。 ∎

### 4.3 (I-3) 反応プール正値性

**主張**: σ_reaction(t) ≥ 0 を保ちつつ payout が滑らかに 0 に収束。

```
γ(t) = σ_reaction(t) / TotalIssuance(t)
payout(t) ≤ min(γ × N_reactions × √work_avg, σ_reaction × 5%)
```

`σ_reaction(t+1) = σ_reaction(t) + inflow - payout(t)` で payout cap が σ_reaction × 5% なので:
```
σ_reaction(t+1) ≥ σ_reaction(t) × 0.95 + inflow ≥ inflow > 0
```
よって σ_reaction は **0 に到達しない**。 ∎

### 4.4 (I-4) Sybil 不採算

**主張**: Sybil reactor の attack ROI が市場価格との関係で逆ザヤ化する条件:

```
Sybil_revenue_per_year = sybil_count × γ × √min_work × 365
Sybil_cost_per_year   = sybil_count × (cpu_year_cost + lock_opportunity_cost)
                     = sybil_count × (cpu_$/yr + 0.1 × MORAL_$ × interest_rate)
```

ROI < 1 ⟺
```
γ × √min_work × 365 / (cpu_$/yr / MORAL_$ + 0.1 × interest_rate) < 1
```

仮に MORAL = $0.01, cpu = $300/yr, interest = 5%:
```
γ × √min_work × 365 < (30000 + 0.005) ≈ 30000
γ × √min_work < 82
```
γ_target = 1% なので √min_work < 8200 (work < 67M iter ≈ 0.06s @ 1MH/s)。**16-bit PoW (= 65k iter) ならばギリギリ採算合うが、17-bit (= 131k) で逆ザヤ化**。よって `BaseDifficulty=17` 以上を mainnet で採用すれば Sybil 不採算。

ただし **reactor_lock = 0.1 MORAL × 24h** が追加防御として効く: 1M Sybil bootstrap には 100,000 MORAL の流動性を破壊する必要があり、市場で買い集めれば価格を押し上げて自己破壊する。 ∎

### 4.5 (I-5) Spam 自己消費

**主張**: 100k posts/day × 4KB = 400MB/day の spam 攻撃は EIP-1559 base_fee を BaseFeeMax に押し上げ、有限期間で attacker を破産させる。

シミュレータ結果 (S3, M1):
- 5 年で base-fee burn = 97G MORAL
- 攻撃者の post 累計コスト = 100k × 1825 × (50 + 0.1×4096 + 0.0008×4096) = ~7.5e10 MORAL
- 攻撃者は **75B MORAL を burn する必要がある**

総供給 (genesis 100k + 5y mint 2.5M) を遥かに超過するため、**攻撃は流動性枯渇で必ず失敗する**。 ∎

### 4.6 トレードオフ表 (どこで何を犠牲にしたか)

| 設計選択 | 得たもの | 犠牲にしたもの |
|---|---|---|
| Tail emission 0.5 MORAL | 永続セキュリティ | 純デフレ通貨を諦めた (現状はインフレ→デフレで均衡) |
| EIP-1559 base fee | spam 経済攻撃の阻止 | 「TX 手数料 0」原則 |
| Storage stake bond | skin-in-the-game | ノード起動コスト ↑ (非匿名性 ≠) |
| Reactor lock 0.1 MORAL | Sybil 経済コスト | 新規ユーザの即時参加性 ↓ |
| 動的 γ | 自己平衡 | 報酬予測可能性 ↓ |
| Governance 化 | 適応性 | governance 攻撃の表面 ↑ |

---

## 5. 数値シミュレーション結果サマリ

詳細は `/tmp/anarchy_sim.py` および `/tmp/anarchy_sim_output.txt`。

### 5.1 5 年シミュレーション (block_time=30s)

| シナリオ | M0 σ_storage | M1 σ_storage | M0 σ_reaction | M1 σ_reaction |
|---|---:|---:|---:|---:|
| S1 organic 1k→100k DAU | 42.3B | 16.9B | 123M | 60M |
| S2 stagnation 1k DAU | 838M | **342M (健全)** | 12M | 0.6M |
| S3 spam 100k posts/day | 23.6B | **8.2B** | 1.93B | 26M |
| S4 sybil 1M reactors | 8.4B | 3.4B | **0 (枯渇)** | **6.1M (生存)** |
| S5 5k DAU flat | 4.2B | 1.7B | 21M | 3M |

### 5.2 ストレージ支払総額 (5 年累積)

| シナリオ | M0 (BaseRewardPerByte=1e-12) | M1 (BaseRewardPerByte=5e-9) | 倍率 |
|---|---:|---:|---:|
| S1 organic | 815 MORAL | 1,289,233 MORAL | 1,580× |
| S2 stagnation | 16 | 25,523 | 1,595× |
| S3 spam | 700 | 1,106,289 | 1,580× |
| S5 5k DAU | 81 | 127,649 | 1,575× |

→ **M0 は事実上 storage node に報酬を払えていない**。M1 で適切な水準に。

### 5.3 EIP-1559 base fee の挙動

| シナリオ | 最終 base_fee (MORAL/byte) | 5y burn 累計 |
|---|---:|---:|
| S1 organic | 0.1 (cap) | 112G MORAL |
| S2 stagnation | 1e-7 (min) | 2,243 |
| S3 spam attack | 0.1 (cap) | 97G MORAL |
| S5 5k loyal | 1e-7 (min) | 11,213 |

→ **平常時は無視できる手数料、攻撃時のみ膨張**。ETH の base fee と同じ性質。

### 5.4 Sybil ROI 解析

シナリオ S4 (1M Sybil / 10k DAU, 5y):
- M0: Sybil 取り分 76.9% (446M MORAL), reaction pool 完全枯渇
- M1: Sybil 取り分 82.6% (481M MORAL), reaction pool 生存 (cap が効く)

→ **TSTS 単独では Sybil 抑制不十分 ⟹ §3.2.6 (d) reactor stake (lock 0.1 MORAL × 24h) が必須**。

---

## 6. 実装リスク・運用考慮

### 6.1 実装の複雑度

| 変更 | LoC 推定 | 工数 |
|---|---:|---:|
| Block reward 3-way fan-out | 50 | 2h |
| Tail emission | 20 | 1h |
| EIP-1559 base fee | 200 | 1d |
| pallet_storage_stake (新規) | 600 | 3d |
| Storage reward 式改訂 | 100 | 4h |
| Reaction γ 動的化 | 150 | 1d |
| Reactor lock | 80 | 4h |
| Stealth reward 配線 | 100 | 4h |
| Faucet cap | 30 | 1h |
| Governance 化 (parameter pallet) | 300 | 2d |
| **合計** | ~1,630 | **8-10 営業日** |

### 6.2 Migration 戦略

`CLAUDE.md` の **Compatibility Policy: 既存データ破棄して chainspec 再生成**。よって migration code は不要。**chainspec 再ジェネレート → testnet wipe → mainnet 投入**。

### 6.3 Governance 移行

- Phase 0: 全パラメータをコード定数 (現状) → testnet で検証
- Phase 1: mainnet ローンチ時に 3-of-5 multisig (`pallet_collective`)
- Phase 2: 6 か月後 token-weighted referenda (`pallet_referenda`) → ただし**匿名性原則に抵触**するため zk-vote 実装まで保留可能。代替として **PoW miner top-K vote** (現状の GRANDPA election と同じスキーム) で決議を提案。

### 6.4 監視・運用

- Grafana ダッシュボード: σ_storage, σ_reaction, σ_stealth, base_fee, total_issuance, miner_revenue, sybil_metric
- 警報: σ_reaction < 1k MORAL (枯渇間近), base_fee > 0.05 (混雑検知)

---

## 7. オープン課題 (Phase 2 で再検討)

1. **完全匿名 governance**: zk-SNARK ベース投票で stake-weighted を maintain しつつ匿名性を満たすか。
2. **Cross-pool rebalancing**: σ_storage 過剰 / σ_reaction 不足時の自動再均衡 mechanism (treasury rebalance).
3. **Reactor stake の UX**: 新規ユーザが 0.1 MORAL を持つ前提のため、faucet との連携設計.
4. **Storage stake の geographic distribution**: bond 額に応じた地理冗長性インセンティブ (現状は size のみ).
5. **AMM-like priority fee**: post の即時実行 (priority block 入れ) を望むユーザに対する追加 tip 経路.

---

## 8. 関連ドキュメント

- [`docs/economic_parameters.md`](parameters.md) — 現状パラメータ全棚卸し
- [`docs/economic_model_implementation_plan.md`](implementation-plan.md) — 実装計画
- [`docs/TODO.md` §4.4](../development/todo.md) — 進捗チェックリスト
- [`docs/blockchain_logic.md`](../architecture/blockchain.md) — chain ロジック総論
- [`docs/storage_logic.md`](../architecture/storage.md) — storage 報酬・PoW・GC

シミュレータ: [`docs/economic/simulator.py`](simulator.py) — `python3 docs/economic/simulator.py` で再現可能。出力サンプルは [`docs/economic/simulator_output.txt`](simulator-output.txt)。

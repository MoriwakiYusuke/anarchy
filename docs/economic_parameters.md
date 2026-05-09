# Anarchy 経済パラメータ全棚卸し (TSTS v1)

> **ステータス**: TSTS 経済モデル v1 全実装後 (PR #54 + #55) の現状値. mainnet 投入前の最終調整時はこの表を直接更新すること.
>
> **対象**: `runtime/src/lib.rs` ベースの実装値. governance 可変な項目は `pallet-economic-params` 経由で `set_*` extrinsic で書き換え可能 (mainnet 初期は `EnsureRoot OR Council 1/2`).
>
> **トークン精度**: 1 MORAL = 10^12 units (12 decimals)
> **ブロック時間**: 30 秒 → 1 日 ≈ 2,880 ブロック, 1 年 ≈ 1,051,200 ブロック
>
> **関連ドキュメント**:
> - 設計提案: [`economic_model_proposal.md`](economic_model_proposal.md)
> - 実装計画: [`economic_model_implementation_plan.md`](economic_model_implementation_plan.md)
> - シミュレータ: [`economic/simulator.py`](economic/simulator.py)

---

## 0. 凡例

各表の `governance` 列の意味:
- ✅ = `pallet-economic-params::set_*` で governance 可変
- ❌ = ConstU* 固定 (runtime upgrade が必要)
- ⚠ = 部分的 governance 可 (default 値のみ tunable, 既存の Config 接続は upgrade 必要)

---

## 1. ネイティブ通貨・チェーン全体

### 1.1 `pallet_balances` ([runtime/src/lib.rs L296-L304](../apps/blockchain/runtime/src/lib.rs#L296))

| 名前 | 値 | governance | 役割 |
|---|---|---|---|
| `ExistentialDeposit` | `1` (= 10^-12 MORAL) | ❌ | アカウント維持に必要な最小残高 |
| `MaxLocks` | `50` | ❌ | アカウントあたりロック数上限 |
| `MaxReserves` | `()` | ❌ | 予約スロット制限なし |
| `MaxFreezes` | `0` | ❌ | フリーズ未使用 |

### 1.2 `pallet_transaction_payment` ([runtime/src/lib.rs L308-L322](../apps/blockchain/runtime/src/lib.rs#L308))

| 名前 | 値 | 役割 |
|---|---|---|
| `WeightToFee` | `ConstantMultiplier<_, ConstU128<0>>` | TX 手数料 0 (post / DM の base fee で代替) |
| `LengthToFee` | `ConstantMultiplier<_, ConstU128<0>>` | 長さ手数料 0 |
| `FeeMultiplier` | `Multiplier::one()` | 動的乗数 1.0 固定 |
| `OperationalFeeMultiplier` | `5` | Operational class 倍率 (手数料 0 で死コード) |

### 1.3 `frame_system` ([runtime/src/lib.rs L84-L154](../apps/blockchain/runtime/src/lib.rs#L84))

| 名前 | 値 | 役割 |
|---|---|---|
| `MILLISECS_PER_BLOCK` | `30_000` | PoW ターゲット 30 秒 |
| `MAXIMUM_BLOCK_LENGTH` | `5 MiB` | ブロックサイズ上限 |
| `MAXIMUM_BLOCK_WEIGHT` | `(2 兆ps, 5 MiB)` | ref_time 2 秒, proof_size 5 MiB |
| `NORMAL_DISPATCH_RATIO` | `75 %` | Normal 系 dispatch がブロックの何割使えるか |
| `BlockHashCount` | `2400` | ブロックハッシュ保持数 (≒ 20 時間) |
| `MaxConsumers` | `64` | アカウントを参照できる pallet 数上限 |
| `SS58Prefix` | `42` | アドレス prefix |

---

## 2. PoW / コンセンサス

### 2.1 `pallet_difficulty` (LWMA-3) ([runtime/src/lib.rs L183-L192](../apps/blockchain/runtime/src/lib.rs#L183))

| 名前 | 値 | governance | 役割 |
|---|---|---|---|
| `TargetBlockTime` | `30_000ms` | ❌ | LWMA の目標 |
| `DifficultyAdjustWindow` | `60` | ❌ | LWMA window |
| `MinDifficulty` | `100` | ❌ | dev/WSL 用 floor (production は spec §1 で 10,000 推奨) |

### 2.2 `pallet_block_reward` ⭐ **TSTS P1: 3-way fan-out + tail emission** ([runtime/src/lib.rs L221-L271](../apps/blockchain/runtime/src/lib.rs#L221))

| 名前 | 旧 (M0) | **現 (TSTS v1)** | governance | 役割 |
|---|---|---|---|---|
| `InitialBlockReward` | `5 MORAL` | `5 MORAL` (`5e12` units) | ❌ | era 0 のブロック報酬 |
| `TailEmission` | (無し) | **`0.5 MORAL`** (`5e11`) | ❌ | 永続下限 — 51 % 攻撃コスト > 0 を保証 |
| `HalvingPeriod` | `4_204_800` | `4_204_800` blocks (≒ 4 年) | ❌ | halving 周期 |
| `MaxHalvings` | `64` | `64` | ❌ | 上限到達後 halved 部分は 0 (tail のみ) |
| `MinerSharePermill` | (100% mint) | **`50%`** | ⚠ via `set_block_reward_shares` | block reward の miner 取り分 |
| `StorageSharePermill` | — | **`30%`** | ⚠ 同上 | block reward の storage プール流入 |
| `ReactionSharePermill` | — | **`20%`** | ⚠ 同上 | block reward の reaction プール流入 |

**式**: `reward(h) = max(InitialReward >> halvings(h), TailEmission)`. 各 share を per-block で個別 mint.

**不変条件 I-1 (永続セキュリティ)**: ∀t, BlockReward(t) ≥ TailEmission > 0.

### 2.3 `pallet_grandpa_authority_election` ([runtime/src/lib.rs L275-L290](../apps/blockchain/runtime/src/lib.rs#L275))

| 名前 | 値 | 役割 |
|---|---|---|
| `ElectionWindowSize` | `100` | top-K 選出のための観測 window |
| `ElectionAuthorityCount` | `10` | GRANDPA finalizer 数 |
| `ElectionRotationPeriod` | `600` blocks (5 h) | ローテ間隔 |
| `ElectionRotationDelay` | `10` blocks (5 min) | 反映遅延 |

### 2.4 `pallet_grandpa` ([runtime/src/lib.rs L161-L169](../apps/blockchain/runtime/src/lib.rs#L161))

| 名前 | 値 | 役割 |
|---|---|---|
| `MaxAuthorities` | `32` | finalizer 最大 |
| `MaxNominators` | `0` | nominator 不在 |
| `EquivocationReportSystem` | `()` | スラッシング無効 |

---

## 3. Governance ⭐ **TSTS F8**

### 3.1 `pallet_collective` (Council) ([runtime/src/lib.rs L335-L373](../apps/blockchain/runtime/src/lib.rs#L335))

| 名前 | 値 | 役割 |
|---|---|---|
| `CouncilMaxMembers` | `7` | Council メンバー上限 |
| `CouncilMaxProposals` | `16` | in-flight 提案数上限 |
| `CouncilMotionDuration` | `1_440` blocks (~12 h) | 投票期間 |
| `MaxCollectivesProposalWeight` | `50 % × MaxBlockWeight` | 提案で実行できる call の重み上限 |
| `SetMembersOrigin` | `EnsureRoot` | mainnet 初期は sudo が member rotation |

**`EconomicGovernanceOrigin`** = `EitherOfDiverse<EnsureRoot, EnsureProportionAtLeast<Council, 1, 2>>`
- mainnet 初期: root のみ
- 中期: council 過半数も有効
- 長期: zk-vote referenda へ移行 (別 PR)

---

## 4. EIP-1559 Base Fee ⭐ **TSTS P2**

### 4.1 `pallet_base_fee` ([runtime/src/lib.rs L391-L403](../apps/blockchain/runtime/src/lib.rs#L391))

| 名前 | 値 (mainnet 推奨) | governance | 役割 |
|---|---|---|---|
| `GasTargetBytesPerBlock` | `50_000` (50 KB) | ✅ via `EconomicParams::DefaultGasTarget` (default のみ) | 1 block の target 使用 bytes |
| `BaseFeeMin` | `100` units (= `1e-10` MORAL/byte) | ✅ via `set_base_fee_range` | base_fee 下限 |
| `BaseFeeMax` | `100_000_000_000` units (= `0.1` MORAL/byte) | ✅ 同上 | spam 攻撃時の cap |
| `BaseFeeInit` | `10_000` units (= `1e-8` MORAL/byte) | ❌ | genesis 初期値 |

**式**: 毎 block `on_finalize` で `base_fee × (1 + (used/target − 1) / 8)`, ±12.5% で clamp, `BaseFeeMin..BaseFeeMax` で saturate.

**不変条件 I-5 (Spam 自己消費)**: 持続的 spam で base_fee → BaseFeeMax. 攻撃者の MORAL を有限時間で枯渇.

---

## 5. 投稿コスト・Storage / Reaction 還流 ⭐ **TSTS P3+P7**

### 5.1 `pallet_post` ([runtime/src/lib.rs L495-L510](../apps/blockchain/runtime/src/lib.rs#L495))

| 名前 | 旧 (M0) | TSTS v1 中間 | **現 (TSTS v1 final)** | governance | 役割 |
|---|---|---|---|---|---|
| `PostBaseCost` | `100 MORAL` | `50 MORAL` | **`25 MORAL`** | ❌ | 投稿基本コスト (Faucet 100 で 3〜4 投稿 UX) |
| `PostByteCost` | `0.001 MORAL/byte` | `0.0008 MORAL/byte` | **`0.0008 MORAL/byte`** | ❌ | バイト単価 |
| `MaxContentLength` | `1 GB` | `1 GB` | `1 GB` | ❌ | 上限 |

**bootstrap UX 改訂の根拠**: Faucet `RewardAmount = 100 MORAL` 据え置きで、新規ユーザー UX が
「1 投稿で MORAL 枯渇」では離脱率が高いため `PostBaseCost` を 50 → 25 に減額。Sybil 攻撃経路
(Faucet pool size + 難易度) は不変なので攻撃経済性に影響なし。Spam 攻撃は EIP-1559 base_fee が
立ち上がって total コスト保たれる。

### 5.2 投稿コストの分配 ([pallet-post/src/lib.rs](../apps/blockchain/pallets/post/src/lib.rs))

| 旧 (M0) | **現 (TSTS v1)** | governance |
|---|---|---|
| 80 % storage / 10 % reaction / 10 % burn | **50 % / 20 % / 30 %** | ✅ via `set_post_storage_share` / `set_post_reaction_share` |

**配分対象**: `base_cost + size_cost`. base_fee burn 部分は完全 burn (混雑自己消費).

**式**: `post_total = base_cost + (byte_cost + base_fee) × bytes`
- `storage_share = post_distributable × 50%` → σ_storage 流入
- `reaction_share = post_distributable × 20%` → σ_reaction 流入
- `burn_share = 30%` (残差として burn) + `base_fee × bytes` (混雑時に膨張)

---

## 6. Storage Pallet ⭐ **TSTS P3+F1**

### 6.1 報酬計算 ([pallet-storage/src/rewards.rs](../apps/blockchain/pallets/storage/src/rewards.rs))

| 名前 | 旧 (M0) | **現 (TSTS v1)** | governance | 役割 |
|---|---|---|---|---|
| `BaseRewardPerByte` | `1` unit (`1e-12` MORAL/byte) | **`5_000` units (= `5e-9` MORAL/byte = 5 nano-MORAL/byte)** | ❌ | proof 成功 1 回あたり = `BaseRewardPerByte × data_size` |
| `ScoreThreshold` | `100` | `100` | ❌ | 報酬対象スコア |
| `ScoreHysteresisMargin` | `20` | `20` | ❌ | 復帰閾値 = `ScoreThreshold + 20` |
| `MinWithdrawalAmount` | `500 MORAL` | `500 MORAL` | ❌ | 引き出し最小額 |
| `StoragePoolTarget` | — | **`500_000 MORAL`** | ❌ | プール残高がこれ以下なら線形 decay |
| `SlashRatePerFailPpm` | (50% pending) | **`50_000` ppm (= 5 %/fail)** | ✅ via `set_slash_rate_per_fail_ppm` | 1 failed challenge あたり bond 削減割合 |

**式 (`calculate_reward_v3`)**:
```
reward = BaseRewardPerByte × data_size
       × min(1, σ_storage / σ_target)            ← pool ratio decay (P3)
       × √(node_bond / total_active_bond)        ← quadratic Sybil resistance (F1)
```

**不変条件 I-2 (ストレージプール下限)**: block reward 30 % 流入で ∀t, σ_storage ≥ tail mint × 30 %.

### 6.2 Storage Stake (`pallet_storage_stake`) ⭐ **TSTS P4** ([runtime/src/lib.rs L376-L388](../apps/blockchain/runtime/src/lib.rs#L376))

| 名前 | 値 (mainnet 推奨) | governance | 役割 |
|---|---|---|---|
| `BondPerGB` | **`10 MORAL`/GB** | ✅ via `set_bond_per_gb` | 1 GB 宣言容量あたりの bond |
| `MinDeclaredCapacity` | `1 GB` (`1_073_741_824` bytes) | ❌ | 最小宣言容量 |
| `BondReleaseDelay` | `100_800` blocks (7 d) | ❌ | release 申請から finalize まで |
| `SlashBurnSharePermill` | `30 %` | ❌ | slash 額のうち burn する割合 (残り 70 % は free balance に戻る) |

**Slash 動作**: `bond × SlashRatePerFailPpm` を `do_slash_node` で削減. 30 % は slash_reserved で burn, 70 % は unreserve で operator に返却.

### 6.3 PoW (ノード登録) ([pallet-storage/src/pow.rs](../apps/blockchain/pallets/storage/src/pow.rs))

| 名前 | 値 | 役割 |
|---|---|---|
| `BasePowDifficulty` | `12` bits | ノード登録 PoW 基本難易度 |
| `PowObservationPeriod` | `10` blocks | 観測 window |
| `MAX_DIFFICULTY` | `24` | ハードコード上限 |

### 6.4 容量・形状制約

| 名前 | 値 |
|---|---|
| `MaxFragmentSize` | `1 GB` |
| `MinPeerIdLen` / `MaxPeerIdLen` | `38` / `64` byte |
| `MaxHoldersPerFragment` | `100` |
| `MaxFragmentsPerNode` | `10,000` |
| `MinNodeCapacity` | `1 GB` |
| `MaxHttpUrlLen` | `256` byte |

### 6.5 Rate-limit

| 名前 | 値 |
|---|---|
| `MaxRegistrationsPerBlock` | `5` |
| `MaxDeclarationsPerBlockPerNode` | `10` |
| `MaxChallengesPerBlock` | `10` |

---

## 7. Reaction Pallet ⭐ **TSTS P5**

### 7.1 動的 γ + decay + reactor lock ([runtime/src/lib.rs L645-L678](../apps/blockchain/runtime/src/lib.rs#L645))

| 名前 | 旧 (M0) | **現 (TSTS v1)** | governance | 役割 |
|---|---|---|---|---|
| `ReactionReward` | `1 MORAL` 固定 | (γ_max=0 時 fallback のみ) | ❌ | 動的 γ が無効なら fallback |
| `BaseDifficulty` | `16` bits | `16` bits | ❌ | 反応 PoW 基本 |
| `MinDifficulty` / `MaxDifficulty` | `8` / `32` | `8` / `32` | ❌ | 範囲 |
| `ChallengeValidity` | `100` blocks | `100` blocks | ❌ | チャレンジ有効期間 |
| `TargetReactionRate` | `10`/block | `10`/block | ❌ | 目標スループット |
| `AdjustmentWindow` | `10` blocks | `10` blocks | ❌ | 難易度調整 window |
| `AdjustmentDivisor` | `4` | `4` | ❌ | 平滑係数 |
| `GammaMaxPpm` | — | **`10_000` ppm (= 1 %)** | ❌ | γ = pool/total_issuance の上限 |
| `ReactorDecayK` | — | **`100`** | ❌ | n 番目反応の decay = `1/√(1 + n/K)` |
| `PerBlockPayoutCapPpm` | — | **`17` ppm (≒ 5 %/day)** | ❌ | per-block の pool 流出上限 |
| `ReactorLockMin` | — | **`0.1 MORAL`** (`1e11` units) | ✅ via `set_reactor_lock_min` | 報酬を得るための最小 lock |
| `ReactorLockDuration` | — | **`2_880` blocks (24 h)** | ❌ | lock の解除待ち時間 |

**式 (compute_reward)**:
```
γ = min(GammaMaxPpm/1e6, σ_reaction / TotalIssuance)
decay = √(K / (K + reactor_count))
reward = ReactionReward × γ × decay
       (ただし pool × PerBlockPayoutCapPpm/1e6 で per-block 上限)
```

**Reactor lock**: `lock_for_rewards(amount)` で `ReservableCurrency::reserve` 必須. 報酬を mint する前提条件 (`ReactorLockMin > 0` 時).

**不変条件 I-3 (反応プール正値性)**: per-block cap で σ_reaction → 0 不可.
**不変条件 I-4 (Sybil 不採算)**: `reactor_lock × Sybil_count` の MORAL 流動性破壊が必要.

---

## 8. DM (Messaging) コスト・分配 ⭐ **TSTS P6**

### 8.1 `pallet_messaging` ([runtime/src/lib.rs L711-L750](../apps/blockchain/runtime/src/lib.rs#L711))

| 名前 | 旧 (M0) | TSTS v1 中間 | **現 (final)** | governance | 役割 |
|---|---|---|---|---|---|
| `DmBaseCost` | `1 MORAL` | `0.5 MORAL` | **`0.25 MORAL`** | ❌ | DM 基本 (bootstrap UX 改訂で 50→25) |
| `DmByteCost` | `0.05 MORAL/byte` | `0.04 MORAL/byte` | **`0.04 MORAL/byte`** | ❌ | バイト単価 |
| `MaxDmCiphertextLen` | `262_144` (256 KiB) | `262_144` | `262_144` | ❌ | 1 DM サイズ上限 |
| `MaxDispatchesPerBlock` | `256` | `256` | `256` | ❌ | block あたり |

### 8.2 DM コスト分配

| 旧 (M0) | **現 (TSTS v1)** | governance |
|---|---|---|
| 80 % storage / 10 % stealth (`()` で実質 burn) / 10 % burn | **50 % / 20 % / 30 %** | ✅ via `set_dm_storage_share` / `set_dm_stealth_share` |

**Stealth 還流**: `pallet_stealth::StealthRewardPool` に配線済 (P6). `claim_stealth_reward` extrinsic で受信実績比例で payout 可能.

### 8.3 DM 受信報酬 (Stealth Pool) ⭐ **TSTS F2 / F2.5**

| 名前 | 値 | governance | 役割 |
|---|---|---|---|
| `ClaimCapPpm` | `100_000` ppm (= 10 %) | ❌ | 1 回 claim あたり pool 流出上限 |
| Signature 検証 | sp_io::ed25519_verify | ❌ | stealth_pubkey で `(signer, ephemeral_pubkey)` を署名検証 (F2.5) |
| Correspondence verifier | `()` no-op | ❌ | F10 zk-proof scaffold (将来 Groth16 / Halo2 で差替) |

**Cap で truncate された場合の partial claim**: `advanced_count = max(1, unclaimed × payout / proportional_full)` で **比例分のみ** claimed_count を進める. 残りは次回回収可能.

---

## 9. Faucet Pallet ⭐ **TSTS P7**

| 名前 | 旧 (M0) | **現 (TSTS v1)** | governance | 役割 |
|---|---|---|---|---|
| `BaseDifficulty` | `18` bits (~3 sec) | `18` bits | ❌ | 初期難易度 |
| `DifficultyScalingFactor` | `1000` | `1000` | ❌ | log2 スケール |
| `MaxDifficulty` | `28` bits (~3 min) | `28` bits | ❌ | 上限 |
| `RewardAmount` | `100 MORAL` | `100 MORAL` | ❌ | 1 claim あたり |
| `ChallengeValidity` | `100` blocks | `100` blocks | ❌ | チャレンジ有効期間 |
| `TotalCap` | (無し, 永続) | **`100,000 MORAL`** | ❌ | 累積発行上限 — bootstrap 専用 sunset |

**式 (難易度)**: `min(base + ⌊log2(1 + total_claims/scaling)⌋, max)`

`TotalMinted >= TotalCap` で `Error::FaucetCapReached` 発火 → claim 不可.

---

## 10. Popularity Pallet (人気度・GC) ([runtime/src/lib.rs L682-L702](../apps/blockchain/runtime/src/lib.rs#L682))

| 名前 | 値 | 役割 |
|---|---|---|
| `InitialScore` | `100_000` | 投稿開始時スコア |
| `LikeWeight` | `100` | Like 加点 |
| `DislikeWeight` | `50` | Dislike 減点 |
| `DecayRatePermill` | `999_950 / 1_000_000` | per-block 減衰 (半減期 ~23h) |
| `LowPopularityThreshold` | `1_000` | GC マーク基準 |
| `HysteresisMargin` | `500` | 復帰用マージン |
| `GracePeriod` | `100_800` blocks (7 d) | GC 猶予 |
| `MaxPostsScannedPerBlock` | `8` | 衰退スキャン上限 |
| `MaxDeletionsPerBlock` | `4` | 削除上限/block |
| `MaxDeletionScanReads` | `16` | 削除走査読込上限 |
| `MaxDecaySteps` | `1_000_000` | 衰退補間 step 上限 |

---

## 11. その他の周辺定数

### 11.1 Stealth ([runtime/src/lib.rs L598-L606](../apps/blockchain/runtime/src/lib.rs#L598))

| 名前 | 値 | 役割 |
|---|---|---|
| `MaxEntriesPerBlock` | `100` | 1 block の ephemeral key 登録上限 |
| `ClaimCapPpm` | `100_000` (10 %) | claim_stealth_reward の per-claim cap |

### 11.2 Nickname

| 名前 | 値 |
|---|---|
| `MaxNicknameLength` | `128` byte |

---

## 12. Genesis 初期分配 ([node/src/chain_spec.rs](../apps/blockchain/node/src/chain_spec.rs))

| 名前 | 値 (dev/testnet) | 役割 |
|---|---|---|
| `INITIAL_MORAL` | `10,000 MORAL` × endowed accounts | 初期残高 |
| `INITIAL_REWARD_POOL` (storage) | `100,000 MORAL` (旧 1M から縮小) | Storage 報酬プール seed |
| `INITIAL_REACTION_REWARD_POOL` | `100,000 MORAL` (旧 10M から縮小) | Reaction 報酬プール seed |
| `INITIAL_REACTION_DIFFICULTY` | `16` bits | Reaction 初期難易度 |
| sudo key | Alice | (mainnet では削除する想定) |

**genesis 縮小の理由**: Block reward 30 % 流入 (TSTS P1) で運用補充されるため大きい seed は不要.

---

## 13. Governance 経由可変パラメータ一覧 ⭐ **TSTS F5**

`pallet-economic-params` の `set_*` extrinsic で `EconomicGovernanceOrigin` (= EnsureRoot OR Council majority) から発議可能:

| Setter | 対象 | バリデーション |
|---|---|---|
| `set_post_storage_share(Permill)` | post 配分 storage 割合 | `p ≤ 100%` & `p + post_reaction ≤ 100%` |
| `set_post_reaction_share(Permill)` | post 配分 reaction 割合 | `p ≤ 100%` & `p + post_storage ≤ 100%` |
| `set_dm_storage_share(Permill)` | DM 配分 storage 割合 | 同上 |
| `set_dm_stealth_share(Permill)` | DM 配分 stealth 割合 | 同上 |
| `set_block_reward_shares(miner, storage, reaction)` | block reward 3-way 比率 | `sum ≤ 100%` |
| `set_reactor_lock_min(u128)` | reactor lock 最小額 | なし |
| `set_bond_per_gb(u128)` | storage stake 単価 | なし |
| `set_slash_rate_per_fail_ppm(u32)` | slashing 比率 | `≤ 1_000_000` (100 %) |
| `set_base_fee_range(min, max)` | EIP-1559 base fee | `min ≤ max` |

---

## 14. mainnet 投入前のチェックリスト

| 確認項目 | 詳細 |
|---|---|
| `MinDifficulty` (PoW) | `100` (dev) → `10_000` 推奨に上げる |
| `INITIAL_REWARD_POOL` / `INITIAL_REACTION_REWARD_POOL` | testnet と同等で OK (block reward が補充) |
| `FaucetTotalCap` | `100_000 MORAL` で良いか再検討 (claim 1000 件想定) |
| `BondPerGB` | mainnet 価格で sybil コストが効くか確認 |
| Council 初期 members | sudo が `Council::set_members` で 5-7 名設定 |
| sudo 削除 | mainnet 開始から N ブロック後に sudo を空 key に置換 |
| RandomX seed rotation | epoch ごとの seed 切替 (TODO §4.7 Phase C) |

---

## 15. シミュレーション参照

5 年シミュレーション結果は [`economic/simulator_output.txt`](economic/simulator_output.txt) を参照. 主要 KPI:

| 観点 | M0 (旧) | M1 (TSTS v1) |
|---|---|---|
| Storage 5y 累計支払 (S1 organic) | 815 MORAL | **1,289,233 MORAL** (×1,580) |
| Reaction pool 残高 (S4 Sybil 1M, 5y) | 0 (枯渇) | **6.1M MORAL** (生存) |
| Spam 攻撃時の post コスト | 一定 (104) | **0.1 MORAL/byte cap で膨張** |
| 51% 攻撃コスト (era 64+) | 0 | **TailEmission × hashrate** 永続 |

---

## 16. 改訂履歴

| 日付 | 変更 | 出典 PR |
|---|---|---|
| 2026-05-07 | TSTS v1 経済モデル全面適用 (P1〜P7) | #54 |
| 2026-05-07 | F1 Storage↔Stake wire-up + F2 stealth claim + F4 Grafana | #55 |
| 2026-05-07 | F2.5 ed25519 + F3 exporter + F5 economic-params + F6 frontend | #55 |
| 2026-05-07 | F7 effective_*() refactor + F8 Council + F9 node Prometheus + F10 zk scaffold | #55 |
| 2026-05-07 | Copilot review 11 件全対応 (cap-claimed bug fix, Permill validation, etc.) | #55 |
| 2026-05-07 | E2E 14/14 pass (実機 WSL2 で確認) | #55 |

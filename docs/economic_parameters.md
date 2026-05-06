# Anarchy 経済パラメータ全洗い出し

> **目的**: `docs/TODO.md §4.4 Mainnet設計・経済パラメータ` を埋めるための一次資料。
> 現状コードに存在する経済関連の変数を、Substrate 標準のものを含めて網羅的に列挙する。
> mainnet 投入時にどの値をどう調整するかを議論するときの基礎台帳として使うこと。

- 対象コミット: `feature/pow-migration-cutover` (Phase B PoW 移行後)
- トークン精度: **1 MORAL = 10^12 units (12 decimals)**
- ブロック時間: **30 秒** → 1 日 ≈ 2,880 ブロック, 1 年 ≈ 1,051,200 ブロック

各パラメータの初期値は実装上のコメントで「dev/testnet 用」と明記されているものが多く、
mainnet ではほぼ全項目を再検討する前提で読むこと。

---

## 1. ネイティブ通貨・チェーン全体（Substrate 標準）

### 1.1 `pallet_balances` ([runtime/src/lib.rs L248-L263](../apps/blockchain/runtime/src/lib.rs#L248-L263))

| 名前 | 値 | 役割 |
|---|---|---|
| `ExistentialDeposit` | `1` (= 10^-12 MORAL) | アカウント維持に必要な最小残高（事実上 0） |
| `MaxLocks` | `50` | アカウントあたりロック数上限 |
| `MaxReserves` | `()` | 予約スロット制限なし |
| `MaxFreezes` | `0` | フリーズ未使用 |
| `DustRemoval` | `()` | ED 未満は単に消滅 |

### 1.2 `pallet_transaction_payment` ([runtime/src/lib.rs L267-L281](../apps/blockchain/runtime/src/lib.rs#L267-L281))

| 名前 | 値 | 役割 |
|---|---|---|
| `WeightToFee` | `ConstantMultiplier<_, ConstU128<0>>` | **重み手数料 = 0** |
| `LengthToFee` | `ConstantMultiplier<_, ConstU128<0>>` | **長さ手数料 = 0** |
| `FeeMultiplier` | `Multiplier::one()` | 動的乗数 1.0 固定（=Base Fee なし） |
| `OperationalFeeMultiplier` | `5` | Operational class 倍率（手数料 0 のため事実上死コード） |
| `SignedExtra` | `ChargeTransactionPayment` 削除済 | tip/手数料の徴収経路自体が無い |

### 1.3 `frame_system` ([runtime/src/lib.rs L83-L154](../apps/blockchain/runtime/src/lib.rs#L83-L154))

| 名前 | 値 | 役割 |
|---|---|---|
| `MILLISECS_PER_BLOCK` | `30_000` | PoW ターゲット 30 秒 |
| `MAXIMUM_BLOCK_LENGTH` | `5 MiB` | ブロックサイズ上限 |
| `MAXIMUM_BLOCK_WEIGHT` | `(2 兆ps, 5 MiB)` | ref_time 2 秒, proof_size 5 MiB |
| `NORMAL_DISPATCH_RATIO` | `75 %` | Normal 系 dispatch がブロックの何割使えるか |
| `BlockHashCount` | `2400` | ブロックハッシュ保持数（≒ 20 時間） |
| `MaxConsumers` | `64` | アカウントを参照できる pallet 数上限 |
| `SS58Prefix` | `42` | アドレス prefix |

---

## 2. PoW / コンセンサス

### 2.1 `pallet_difficulty` (LWMA-3) ([runtime/src/lib.rs L180-L192](../apps/blockchain/runtime/src/lib.rs#L180-L192))

| 名前 | 値 | 役割 |
|---|---|---|
| `TargetBlockTime` | `30_000ms` | LWMA の目標 |
| `DifficultyAdjustWindow` | `60` | LWMA window（直近 60 ブロック） |
| `MinDifficulty` | `100` | 床（dev/WSL 向け。production は spec §1 で 10,000 推奨） |
| `GenesisConfig.initial_difficulty` | （genesis で指定） | 初期難易度 |

### 2.2 `pallet_block_reward` ⭐ **インフレ供給の本体** ([runtime/src/lib.rs L219-L230](../apps/blockchain/runtime/src/lib.rs#L219-L230))

| 名前 | 値 | 役割 |
|---|---|---|
| `InitialBlockReward` | `5 MORAL` (`5e12`) | era 0 のブロック報酬 |
| `HalvingPeriod` | `4_204_800` ブロック (≒ 4 年) | halving 周期 |
| `MaxHalvings` | `64` | 上限到達後 mint 停止 |
| `AuthorOrigin` | `PowAuthorAdapter` (engine ID `pow_`) | author 抽出 |

→ 総発行上限（Bitcoin 同型）: `2 × InitialReward × HalvingPeriod` = 約 **42,048,000 MORAL**

### 2.3 `pallet_grandpa_authority_election` ([runtime/src/lib.rs L233-L245](../apps/blockchain/runtime/src/lib.rs#L233-L245))

| 名前 | 値 | 役割 |
|---|---|---|
| `ElectionWindowSize` | `100` | top-K 選出のための観測 window |
| `ElectionAuthorityCount` | `10` | GRANDPA finalizer 数 |
| `ElectionRotationPeriod` | `600` ブロック (5 h) | ローテ間隔 |
| `ElectionRotationDelay` | `10` ブロック (5 min) | 反映遅延 |

### 2.4 `pallet_grandpa` ([runtime/src/lib.rs L161-L169](../apps/blockchain/runtime/src/lib.rs#L161-L169))

| 名前 | 値 | 役割 |
|---|---|---|
| `MaxAuthorities` | `32` | finalizer 最大 |
| `MaxNominators` | `0` | nominator 不在 |
| `EquivocationReportSystem` | `()` | スラッシング無効（PoW で抑止） |

---

## 3. 投稿コスト・burn / Storage 報酬プール

### 3.1 `pallet_post` — 投稿コスト ([runtime/src/lib.rs L291-L301](../apps/blockchain/runtime/src/lib.rs#L291-L301))

| 名前 | 値 | 役割 |
|---|---|---|
| `PostBaseCost` | **`100 MORAL`** (`1e14`) | 投稿基本コスト |
| `PostByteCost` | **`0.001 MORAL/byte`** (`1e9`) | バイト単価 |
| `MaxContentLength` | `1 GB` | 上限 |

→ 1 KB 投稿: `100 + 1024 × 0.001 ≈ 101.024 MORAL`, 1 MB: `~1148 MORAL`

### 3.2 投稿コストの分配 ([pallets/post/src/lib.rs L264-L276](../apps/blockchain/pallets/post/src/lib.rs#L264-L276))

| 配分 | 比率 | 行先 |
|---|---|---|
| Storage 報酬プール | **80 %** | `RewardPoolBalance` |
| Reaction 報酬プール | **10 %** | `ReactionRewardPool` |
| 永久 burn | **10 %** | (`burn_from` の残差として消滅) |

> ⚠ ハードコード比率 (80/10/10)。pallet 定数化されておらず Config から触れない。

---

## 4. Storage Pallet ([runtime](../apps/blockchain/runtime/src/lib.rs#L319-L359) + [pallet](../apps/blockchain/pallets/storage/src/lib.rs))

### 4.1 報酬計算

| 名前 | 値 | 役割 |
|---|---|---|
| `BaseRewardPerByte` | **`1 unit`** (= `1e-12 MORAL/byte`) | proof 成功 1 回あたり = `BaseRewardPerByte × data_size` |
| `ScoreThreshold` | `100` | 報酬対象スコア（未満は 0 報酬 + ForgettingCandidate） |
| `ScoreHysteresisMargin` | `20` | 復帰閾値 = `ScoreThreshold + 20` |
| `MinWithdrawalAmount` | **`500 MORAL`** | 引き出し最小額 |
| `GenesisConfig.initial_reward_pool` | `1,000,000 MORAL` (testnet) | 初期プール残高 |

### 4.2 PoW (ノード登録) — [pallets/storage/src/pow.rs](../apps/blockchain/pallets/storage/src/pow.rs)

| 名前 | 値 | 役割 |
|---|---|---|
| `BasePowDifficulty` | `12` bits | ノード登録 PoW 基本難易度 |
| `PowObservationPeriod` | `10` ブロック | 観測 window |
| `MAX_DIFFICULTY` (const) | `24` | ハードコード上限 |
| 加算式 | `additional = registrations / 5` | 動的加算 |

### 4.3 容量・形状制約

| 名前 | 値 |
|---|---|
| `MaxFragmentSize` | `1 GB` |
| `MinPeerIdLen` / `MaxPeerIdLen` | `38` / `64` byte |
| `MaxHoldersPerFragment` | `100` |
| `MaxFragmentsPerNode` | `10,000` |
| `MinNodeCapacity` | `1 GB` |
| `MaxHttpUrlLen` | `256` byte |

### 4.4 Rate-limit

| 名前 | 値 |
|---|---|
| `MaxRegistrationsPerBlock` | `5` |
| `MaxDeclarationsPerBlockPerNode` | `10` |
| `MaxChallengesPerBlock` | `10` |

### 4.5 Slashing/Repair ([pallets/storage/src/lib.rs L1938-L1975](../apps/blockchain/pallets/storage/src/lib.rs#L1938-L1975))

| 名前 | 値 | 役割 |
|---|---|---|
| Slash penalty | **`PendingRewards / 2`** (50 %) | ハードコード |
| Penalty 行先 | `RepairRewardPools[content_hash]` | repair 完了者へ |
| `priority_score` 加重 | slashed +1000, score-low +100, last_proved/100 ≤500 | eviction 順位 |

---

## 5. Reaction Pallet (反応マイニング) ⭐ **二段目のインフレ源**

### 5.1 [runtime](../apps/blockchain/runtime/src/lib.rs#L386-L411) + [pallet](../apps/blockchain/pallets/reaction/src/lib.rs)

| 名前 | 値 | 役割 |
|---|---|---|
| `ReactionReward` | **`1 MORAL` 固定** | 1 反応あたり報酬（spec §動的式は未実装） |
| `BaseDifficulty` | `16` bits | 反応 PoW 基本 |
| `MinDifficulty` / `MaxDifficulty` | `8` / `32` | 範囲 |
| `ChallengeValidity` | `100` ブロック | チャレンジ有効期間 |
| `TargetReactionRate` | `10`/block | 目標スループット |
| `AdjustmentWindow` | `10` ブロック | 難易度調整 window |
| `AdjustmentDivisor` | `4` | 平滑係数 |
| `GenesisConfig.initial_reward_pool` | `10,000,000 MORAL` (testnet) | 初期プール |
| `GenesisConfig.initial_difficulty` | `16` bits (testnet) | 初期難易度 |

> ⚠ `Reward = Σ(Reaction × Power_cpu) × γ` という TODO の動的式は **未実装**。
> 現在は固定報酬。γ（インフレ調整係数）も storage/state には存在しない。

---

## 6. Faucet Pallet ([runtime/src/lib.rs L304-L316](../apps/blockchain/runtime/src/lib.rs#L304-L316))

| 名前 | 値 | 役割 |
|---|---|---|
| `BaseDifficulty` | `18` bits (~3 sec) | 初期難易度 |
| `DifficultyScalingFactor` | `1000` (claims) | log2 スケール |
| `MaxDifficulty` | `28` bits (~3 min) | 上限 |
| `RewardAmount` | **`100 MORAL`** | 1 claim あたり |
| `ChallengeValidity` | `100` ブロック | チャレンジ有効期間 |
| 計算式 | `min(base + ⌊log2(1 + total_claims/scaling)⌋, max)` | 動的難易度 |

---

## 7. DM (Messaging) コスト・分配 ⭐

### 7.1 [runtime](../apps/blockchain/runtime/src/lib.rs#L443-L458) + [messaging/lib.rs](../apps/blockchain/pallets/messaging/src/lib.rs#L285-L316)

| 名前 | 値 | 役割 |
|---|---|---|
| `DmBaseCost` | **`1 MORAL`** (`1e12`) | DM 基本 |
| `DmByteCost` | **`0.05 MORAL/byte`** (`5e10`) | バイト単価 |
| `MaxDmCiphertextLen` | `262_144` (256 KiB) | 1 DM サイズ上限 |
| `MaxDispatchesPerBlock` | `256` | block あたり |

### 7.2 DM コスト分配（ハードコード）

| 配分 | 比率 | 行先 |
|---|---|---|
| Storage プール | **80 %** | `RewardPoolBalance` |
| Stealth 報酬プール | **10 %** | `StealthReward = ()` のため **現状は永久 burn** |
| 永久 burn | **10 %** | 残差として消滅 |

> ⚠ Stealth reward 還流は trait 定義のみで未配線。実質 **20 % が burn**。

---

## 8. Popularity Pallet（人気度・GC） ([runtime/src/lib.rs L415-L438](../apps/blockchain/runtime/src/lib.rs#L415-L438))

| 名前 | 値 | 役割 |
|---|---|---|
| `InitialScore` | `100_000` | 投稿開始時スコア |
| `LikeWeight` | `100` | Like 加点 |
| `DislikeWeight` | `50` | Dislike 減点 |
| `DecayRatePermill` | `999_950 / 1_000_000` | per-block 減衰（半減期 ~23h） |
| `LowPopularityThreshold` | `1_000` | GC マーク基準 |
| `HysteresisMargin` | `500` | 復帰用マージン |
| `GracePeriod` | `100_800` ブロック (7 日) | GC 猶予 |
| `MaxPostsScannedPerBlock` | `8` | 衰退スキャン上限 |
| `MaxDeletionsPerBlock` | `4` | 削除上限/block |
| `MaxDeletionScanReads` | `16` | 削除走査読込上限 |
| `MaxDecaySteps` | `1_000_000` | 衰退補間 step 上限 |

---

## 9. その他の周辺定数

### 9.1 Stealth ([runtime/src/lib.rs L368-L373](../apps/blockchain/runtime/src/lib.rs#L368-L373))

| 名前 | 値 |
|---|---|
| `MaxEntriesPerBlock` | `100` |

### 9.2 Nickname

| 名前 | 値 |
|---|---|
| `MaxNicknameLength` | `128` byte |

---

## 10. Genesis 初期分配 ([node/src/chain_spec.rs L160-L187](../apps/blockchain/node/src/chain_spec.rs#L160-L187))

| 名前 | 値 (dev/testnet) | 役割 |
|---|---|---|
| `INITIAL_MORAL` | `10,000 MORAL` × endowed accounts | 初期残高 |
| `INITIAL_REWARD_POOL` (storage) | `1,000,000 MORAL` | Storage 報酬プール |
| `INITIAL_REACTION_REWARD_POOL` | `10,000,000 MORAL` | Reaction 報酬プール |
| `INITIAL_REACTION_DIFFICULTY` | `16` bits | Reaction 初期難易度 |
| sudo key | Alice | （mainnet では削除する想定） |

---

## サマリ：mainnet 設計で詰めるべき軸

| 軸 | 現状の主役パラメータ |
|---|---|
| **総供給 / インフレ上限** | `InitialBlockReward × HalvingPeriod × 2` (≈ 4,200 万 MORAL) |
| **時間あたりインフレ率** | `InitialBlockReward / 30s` × era 補正、+ Reaction プール枯渇速度 |
| **デフレ圧** | `PostBase/ByteCost` の 10 % burn + DM 20 % burn |
| **手数料モデル** | TX 手数料 0（変えるなら `WeightToFee` / `LengthToFee` / `FeeMultiplier`） |
| **ストレージインセンティブ** | `BaseRewardPerByte × data_size` × proof 頻度、プール 80 % 流入 |
| **反応マイニング曲線** | TODO 通り `Σ(R × Power_cpu) × γ` を実装するなら新 Storage と新定数が必要 |
| **Faucet 露出量** | `RewardAmount × 期待 claim 数`、難易度 log カーブで漸増 |
| **Genesis 分配** | endowed accounts, 報酬プール初期残高、難易度初期値 |
| **ハードコード比率（要見直し候補）** | post 80/10/10, DM 80/10/10, slashing 50 %, priority_score 加重 |
| **未配線のリーク** | Stealth reward (10 %) → `()` で burn になっている |

### 要注意の "TODO 連動" 項目

- Reaction の動的式 `Reward = Σ(R × Power_cpu) × γ` および γ = `ReactionRewardPool / TotalSupply` は **未実装**。
  実装するには `pallet_reaction` に `ReactionRewardPoolStorage` の参照と `pallet_balances::TotalIssuance`
  経由の動的算出が要る。
- Stealth reward 還流（10 %）の trait 実装が `()` のままで、実質 burn。
- 投稿/DM の 80/10/10 はマジックナンバー。Config 化するか `parameter_types!` 化が望ましい。

---

## 関連ドキュメント

- [`docs/TODO.md` §4.4](TODO.md) — Mainnet 設計・経済パラメータの未着手チェックリスト
- [`docs/blockchain_logic.md`](blockchain_logic.md) — チェーン全体のロジック
- [`docs/storage_logic.md`](storage_logic.md) — Storage 報酬・PoW・GC の流れ

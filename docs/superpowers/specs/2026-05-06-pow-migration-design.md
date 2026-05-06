# PoW 移行設計 (Production Grade)

> **Status**: 設計確定 (実装前)
> **Date**: 2026-05-06
> **Branch (予定)**: `feature/pow-migration` (`main` から分岐)
> **関連**: [TODO.md §4.7](../../TODO.md), [CONCEPTS.md "コンセンサス方式の検討"](../../CONCEPTS.md), [Principle #4 (foreground PoW)](../../../CLAUDE.md#security-principles-non-negotiable)

## 0. ゴール

Anarchy のコンセンサスを **Aura/GRANDPA (PoA)** から **RandomX PoW + Permissionless GRANDPA** に置き換え、mainnet にそのまま投入できる粒度で完成させる。"誰でも参加できるマイニング" という Anarchy の中核原則を、ブロック生成 (PoW) と finality (top-K miner rotation) の両方で達成する。

### 0.1 完全 PoW の意味
- block 生成: RandomX PoW (permissionless)
- finality: GRANDPA 維持。ただし authority set は **直近 N=100 ブロックを採掘した miner の上位 K=10 を毎セッション自動選出**。sudo 介在なし。
- PoA 要素 (`pallet_aura`、固定 authority list) は完全撤廃

### 0.2 非ゴール
- NPoS staking (本設計と consensus 哲学が異なる。MORAL ステーク不要)
- 後方互換性 (CLAUDE.md ポリシーにより chain reset で新 genesis 投入。migration code は書かない)
- reaction-mining (`pallet_reaction`) の algo 変更 (browser foreground sha3 PoW、別ドメイン)
- フロントエンド側 PAPI コードの consensus 関連変更 (smoldot は light client で consensus 不問)

## 1. 確定パラメータ

| 項目 | 値 | 根拠 |
|---|---|---|
| PoW algo | **RandomX** (full dataset, 2GB scratchpad) | Anarchy 原則 "誰でも参加できる" → ASIC 耐性必須。Kulupu 実績あり |
| ブロック時間 (target) | **30s** | 30s は orphan 率と UX のバランス点。Kulupu と同等 |
| DAA | **LWMA-3**, window=60 blocks | 小チェーンで hashrate jump に強い (Monero 標準) |
| 最小 difficulty floor | `U256::from(10_000)` | 起動時 stall 防止 |
| Finality | **PoW + Permissionless GRANDPA**, authority = 直近 100 ブロック採掘 miner top-10 | sudo 介在ゼロ。authority set は session ごとに自動ローテ |
| ブロック報酬 | 初期 **5 MORAL/block**, **4 年毎 halving** | halving 周期 = 4 年 × 365.25 日 × 86400s / 30s = **4_204_800 blocks/era** |
| 漸近上限 | **~42M MORAL** | 収束計算 `5 × 4_204_800 × Σ(1/2^k) = 42,048,000`。後述 §6.2 |
| reaction-mining algo | sha3 のまま (変更なし) | 別ドメイン。browser に RandomX は不可能 |
| Author 識別 | PoW pre-runtime digest に `AccountId` 埋込 (Kulupu 方式) | runtime 側で `FindAuthor` 実装 |

## 2. 全体アーキテクチャ

```
                            +--------------------------------+
                            |    runtime (FRAME composite)   |
                            |                                |
          +---DifficultyApi---> pallet_difficulty (LWMA-3)   |
          |                 |   pallet_block_reward (halving)|
          |                 |   pallet_grandpa_authority_election
          |                 |   pallet_grandpa  (existing, 改修)
          |                 |   pallet_balances (existing)   |
          |                 |   ...                          |
          |                 +--------------------------------+
          |                          ^
          |                          | (FindAuthor: digest 解析)
          v                          |
+--------------------+   PreDigest   |   +-------------------+
|  node/src/pow/     |---(coinbase)->|   | node/src/service. |
|  randomx_algo.rs   |               |   |   rs              |
|  difficulty.rs     |               |   |  - PoW import_que |
|  author.rs         |<--mining------+   |  - mining_worker  |
+--------------------+                   |  - GRANDPA voter  |
                                         |     (rotation 対応)
                                         +-------------------+
```

主要差分:
- node: `sc_consensus_aura::*` を全削除し `sc_consensus_pow::*` に置換
- runtime: `pallet_aura`/`AuraApi` 撤廃、3 つの新規 pallet 追加
- `chain_spec`: Aura keys 削除、production genesis、初期 difficulty 設定

## 3. 依存クレート (要 stable2503 互換性検証)

| クレート | 用途 | 検証ステップ |
|---|---|---|
| `sc-consensus-pow`, `sp-consensus-pow` | PoW import / mining worker / runtime trait | M1 で `cargo check` |
| `randomx-rs` (Kulupu fork or upstream) | RandomX hash | M1 で wasm32v1-none と native の双方コンパイル確認 |
| `sp-consensus-grandpa`, `pallet-grandpa`, `sc-consensus-grandpa` | finality (改修) | 既存依存を維持 |

**互換性 fallback**: `randomx-rs` が stable2503 と非互換な場合、`monero-randomx` または Kulupu fork (`https://github.com/kulupu/kulupu`) の randomx 部分を vendor。

## 4. Runtime 変更

### 4.1 削除
- `use sp_consensus_aura::*`
- `impl pallet_aura::Config for Runtime`
- `construct_runtime!` から `Aura: pallet_aura`
- `SessionKeys.aura`
- `impl sp_consensus_aura::AuraApi<...>`

### 4.2 新規 pallet: `pallet_difficulty`

[`pallets/difficulty/src/lib.rs`](../../../apps/blockchain/pallets/difficulty/src/lib.rs)

```rust
#[pallet::config]
pub trait Config: frame_system::Config + pallet_timestamp::Config {
    type TargetBlockTime: Get<Self::Moment>;       // 30_000 ms
    type DifficultyAdjustWindow: Get<u32>;          // 60
    type MinDifficulty: Get<U256>;                  // 10_000
}

#[pallet::storage]
pub type CurrentDifficulty<T> = StorageValue<_, U256, ValueQuery>;

#[pallet::storage]
pub type PastDifficultiesAndTimestamps<T> = StorageValue<
    _, BoundedVec<(U256, T::Moment), ConstU32<60>>, ValueQuery
>;

// LWMA-3:
//   weight_i = i (1..=N)
//   target_block_time = T
//   solve_time_i = max(1ms, ts_i - ts_{i-1})
//   weighted_sum_target = sum(weight_i * target)
//   weighted_sum_solve  = sum(weight_i * solve_time_i)
//   harmonic_mean_diff  = N / sum(weight_i / diff_i)
//   next_diff = harmonic_mean_diff * weighted_sum_target / weighted_sum_solve
//   next_diff = max(next_diff, MinDifficulty)

impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn on_finalize(_n: BlockNumberFor<T>) {
        // 1. 現ブロックの (CurrentDifficulty, now()) を window に push
        // 2. window が満杯なら LWMA-3 で next を計算
        // 3. CurrentDifficulty を更新
    }
}

sp_api::decl_runtime_apis! {
    pub trait DifficultyApi {
        fn difficulty() -> U256;
    }
}
```

### 4.3 新規 pallet: `pallet_block_reward`

[`pallets/block_reward/src/lib.rs`](../../../apps/blockchain/pallets/block_reward/src/lib.rs)

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type Currency: Currency<Self::AccountId>;
    type InitialReward: Get<BalanceOf<Self>>;       // 5 MORAL = 5e12
    type HalvingPeriod: Get<BlockNumberFor<Self>>;  // 4_204_800
    type MaxHalvings: Get<u32>;                      // 64 (実用上 reward → 0 へ収束)
    type AuthorOrigin: FindAuthor<Self::AccountId>;
}

impl<T: Config> Pallet<T> {
    pub fn current_reward(n: BlockNumberFor<T>) -> BalanceOf<T> {
        let halvings = (n / T::HalvingPeriod::get()).saturated_into::<u32>();
        if halvings >= T::MaxHalvings::get() { return Zero::zero(); }
        T::InitialReward::get() >> halvings   // u128 right-shift で halving
    }
}

impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn on_finalize(n: BlockNumberFor<T>) {
        if let Some(author) = T::AuthorOrigin::find_author(
            <frame_system::Pallet<T>>::digest().logs.iter().filter_map(|l| l.as_pre_runtime())
        ) {
            let reward = Self::current_reward(n);
            if !reward.is_zero() {
                let _ = T::Currency::deposit_creating(&author, reward);
            }
        }
    }
}
```

`FindAuthor` 実装は `pow::author::PowAuthor`: PreRuntime digest の `[ANRC]` engine ID 配下に SCALE-encoded `AccountId` が入っている前提でデコード。

### 4.4 新規 pallet: `pallet_grandpa_authority_election`

[`pallets/grandpa_authority_election/src/lib.rs`](../../../apps/blockchain/pallets/grandpa_authority_election/src/lib.rs)

permissionless GRANDPA authority set を提供する。

```rust
#[pallet::config]
pub trait Config: frame_system::Config + pallet_grandpa::Config {
    type WindowSize: Get<u32>;          // 100 (直近 100 ブロック)
    type AuthorityCount: Get<u32>;      // 10 (top-K)
    type RotationPeriod: Get<BlockNumberFor<Self>>; // 600 ブロック (5 時間 @30s)
    type AuthorOrigin: FindAuthor<Self::AccountId>;
}

#[pallet::storage]
pub type RecentAuthors<T> = StorageValue<
    _, BoundedVec<T::AccountId, ConstU32<100>>, ValueQuery
>;
// ring buffer: 直近 100 ブロックの author を保持

#[pallet::storage]
pub type AuthorityKeys<T> = StorageMap<_, Blake2_128Concat, T::AccountId, GrandpaId>;
// miner が事前に register_grandpa_key extrinsic で登録した GRANDPA key

impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn on_finalize(n: BlockNumberFor<T>) {
        // 1. RecentAuthors に現ブロックの author を push (ring buffer)
        // 2. n % RotationPeriod == 0 なら:
        //    a. RecentAuthors を集計 → 出現回数 top-K
        //    b. それぞれの AuthorityKeys を引いて GRANDPA new authority set を構成
        //    c. pallet_grandpa::schedule_change(new_set, delay=10 blocks, ...)
    }
}

#[pallet::call]
impl<T: Config> Pallet<T> {
    /// マイナーが事前に GRANDPA key を登録 (top-K に入った時点で自動 active)
    pub fn register_grandpa_key(origin, key: GrandpaId) -> DispatchResult { ... }
    pub fn unregister_grandpa_key(origin) -> DispatchResult { ... }
}
```

**設計上のポイント**:
- top-K に入っても `AuthorityKeys` 未登録の miner はスキップ (リスト次順で補充)
- top-K 全員が未登録なら現 authority set を維持 (locking 防止)
- これにより mining → finality voting への参加が permissionless かつ opt-in

### 4.5 SessionKeys / Runtime APIs

```rust
impl_opaque_keys! {
    pub struct SessionKeys {
        pub grandpa: Grandpa,        // aura: Aura は削除
    }
}

impl_runtime_apis! {
    // AuraApi 全削除
    impl pallet_difficulty::DifficultyApi<Block> for Runtime { ... }
    impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime { /* 既存維持 */ }
}
```

## 5. Node 変更

### 5.1 [`node/src/pow/randomx_algo.rs`](../../../apps/blockchain/node/src/pow/randomx_algo.rs)

```rust
pub struct RandomXAlgorithm<C> {
    client: Arc<C>,
    vm_cache: Arc<Mutex<RandomXCache>>,  // dataset (2GB) + key 切替
}

impl<B, C> PowAlgorithm<B> for RandomXAlgorithm<C>
where
    B: BlockT<Hash = H256>,
    C: ProvideRuntimeApi<B>,
    C::Api: DifficultyApi<B>,
{
    type Difficulty = U256;

    fn difficulty(&self, parent: B::Hash) -> Result<U256, Error<B>> {
        self.client.runtime_api().difficulty(parent).map_err(|e| ...)
    }

    fn verify(&self, parent: &BlockId<B>, pre_hash: &H256, _pre_digest: Option<&[u8]>,
              seal: &Seal, difficulty: U256) -> Result<bool, Error<B>> {
        let seal: PowSeal = decode(seal)?;
        let key = self.randomx_key_for(parent)?;     // §5.4
        let work_hash = self.vm_cache.lock().hash(&key, &compose(pre_hash, seal.nonce));
        Ok(U256::from_big_endian(&work_hash) <= U256::MAX / difficulty)
    }
}
```

### 5.2 [`node/src/pow/author.rs`](../../../apps/blockchain/node/src/pow/author.rs)
`FindAuthor` impl: PreRuntime digest engine ID `b"ANRC"` の payload を `AccountId32` として decode。

### 5.3 [`node/src/service.rs`](../../../apps/blockchain/node/src/service.rs) 改修

```rust
// 削除: sc_consensus_aura import / start_aura / aura import_queue
// 追加:
let pow_algo = RandomXAlgorithm::new(client.clone());
let pow_block_import = sc_consensus_pow::PowBlockImport::new(
    grandpa_block_import.clone(),
    client.clone(),
    pow_algo.clone(),
    0,
    select_chain.clone(),
    inherent_data_providers.clone(),
);
let import_queue = sc_consensus_pow::import_queue(
    Box::new(pow_block_import.clone()),
    Some(Box::new(grandpa_block_import.clone())),
    pow_algo.clone(),
    &task_manager.spawn_essential_handle(),
    config.prometheus_registry(),
)?;

// マイニング (--mine 指定時のみ)
if cli.run.mine {
    let coinbase = parse_ss58(&cli.run.coinbase)?;
    let pre_runtime = vec![(POW_ENGINE_ID, coinbase.encode())];
    let (worker, worker_task) = sc_consensus_pow::start_mining_worker(
        Box::new(pow_block_import),
        client.clone(),
        select_chain,
        pow_algo,
        proposer,
        sync_service.clone(),
        sync_service.clone(),
        Some(pre_runtime),
        inherent_data_providers,
        Duration::from_secs(10),
        Duration::from_secs(5),
    );
    task_manager.spawn_essential_handle().spawn_blocking("pow", Some("mining"), worker_task);
    // バックグラウンドで RandomX VM が nonce を回す
}

// GRANDPA: voter は既存通り走らせる
//   authority set rotation は pallet_grandpa::schedule_change 経由で自動反映
```

### 5.4 RandomX key (seed) ローテーション
- RandomX は key 切替時に dataset 再構築 (2GB) が走るので頻繁切替は不可
- key = `block_hash(epoch_start)` where `epoch = block_number / 2048`
- → 約 17 時間ごとに seed 切替 (Monero と同等の周期)

### 5.5 CLI
[`node/src/cli.rs`](../../../apps/blockchain/node/src/cli.rs):
```rust
pub struct RunCmd {
    #[arg(long)] pub mine: bool,
    #[arg(long, value_parser = parse_ss58)] pub coinbase: Option<AccountId>,
    #[arg(long, default_value = "fast")] pub randomx_mode: RandomxMode, // fast=2GB / light=256MB
}
```

## 6. ブロック報酬の経済設計

### 6.1 halving 詳細
| Era | Block 範囲 | Reward (MORAL) | Era 期間 |
|---|---|---|---|
| 0 | 0 .. 4_204_800 | 5 | ~4 年 |
| 1 | .. 8_409_600 | 2.5 | ~4 年 |
| 2 | .. 12_614_400 | 1.25 | ~4 年 |
| ... | ... | ... | ... |
| 63 | .. | 5 / 2⁶³ ≈ 0 (実質終了) | |
| 64+ | 任意 | 0 | 永続 |

### 6.2 漸近供給上限
$$ \sum_{k=0}^{\infty} 5 \times 4{,}204{,}800 \times 2^{-k} = 5 \times 4{,}204{,}800 \times 2 = 42{,}048{,}000 \text{ MORAL} $$

≒ **42M MORAL** が漸近上限。Faucet/post burn は別系統 (mint↔burn のネット供給は別計算)。

### 6.3 既存 mint/burn との整合
- `pallet_faucet`: PoW migration 後も unsigned tx で 100 MORAL mint (短期は維持、mainnet ローンチ前に halving 連動で減額検討 — 別タスク)
- `pallet_post`: post burn は維持
- 新規 mint = ブロック報酬のみ
- ネット供給 = 累積ブロック報酬 + Faucet mint − post burn

## 7. chain_spec / genesis

[`node/src/chain_spec.rs`](../../../apps/blockchain/node/src/chain_spec.rs):

```rust
fn production_genesis() -> RuntimeGenesisConfig {
    RuntimeGenesisConfig {
        // aura: 削除
        grandpa: GrandpaConfig {
            authorities: vec![/* genesis bootstrap miner 1 名のみ */],
            ..Default::default()
        },
        difficulty: DifficultyConfig {
            initial_difficulty: U256::from(/* §10.1 ベンチで決定 */),
        },
        block_reward: BlockRewardConfig {},  // パラメータは Config trait 側
        grandpa_authority_election: GrandpaAuthorityElectionConfig {},
        balances: BalancesConfig { balances: vec![] },  // pre-mint なし
        sudo: SudoConfig { key: None },                  // mainnet では sudo 完全撤廃
        ..Default::default()
    }
}
```

**genesis bootstrap**: 最初の 100 ブロック (RecentAuthors window が満たされるまで) は genesis grandpa authority 1 名で finality を回し、window 充填後に自動で permissionless rotation に切替。

## 8. Reaction-mining (Principle #4) との分離

| 観点 | Consensus PoW | Reaction PoW |
|---|---|---|
| 場所 | mining node (RandomX VM) | browser foreground (Web Worker) |
| Algo | RandomX (2GB scratchpad) | sha3 (`primitives_pow`) |
| Difficulty 管理 | `pallet_difficulty` | `pallet_reaction::CurrentDifficulty` |
| Storage 名前空間 | `Difficulty::*` | `Reaction::*` |
| 干渉 | **なし** (storage / config / api 全分離) |

spec / PR 説明文 / `docs/security/pow-threat-model.md` に明記。

## 9. テスト戦略

### 9.1 Unit (Rust)
| pallet | ケース |
|---|---|
| `pallet_difficulty` | LWMA-3 数値 (window 充填前 / 充填後 / hashrate 1000 倍急増 / 0.001 倍急減 / MinDifficulty floor) |
| `pallet_block_reward` | era 0/1/63/64 の reward / author 不明時 no-op / overflow なし |
| `pallet_grandpa_authority_election` | top-K 集計 / 同票 tie-break (AccountId 辞書順) / 未登録 key スキップ / rotation トリガ |

### 9.2 Service smoke
`cargo run -- --dev --mine --coinbase //Alice` 単体で:
- 5 分間ブロック生成継続 (target 30s ± 60% 以内)
- GRANDPA finality が進行
- RandomX dataset init (2GB) が完了する

### 9.3 Integration (shell, [`apps/blockchain/tests/integration/pow/`](../../../apps/blockchain/tests/integration/pow/))
| シナリオ | 検証 |
|---|---|
| `multi_miner.sh` | 3 ノード (各 `--mine` 別 coinbase) で 30 分稼働、reorg 観察、GRANDPA finality 各ノード一致 |
| `hashrate_jump.sh` | hashrate 1000 倍急増シミュレーション (追加 100 ノード起動) → DAA が 60 ブロック以内に target 30s に再収束 |
| `authority_rotation.sh` | top-10 メンバーが入れ替わる際の GRANDPA `schedule_change` 反映と finality 連続性 |
| `selfish_mining.sh` | 攻撃ノードが private chain を 6 ブロック秘匿後 publish → reorg されるが finality は守られる |
| `coinbase_inject.sh` | 不正な PreRuntime digest (壊れた author) → block reject される |

### 9.4 Frontend smoke (手動)
- smoldot で chain head 進行確認
- PAPI で `system_chain` 応答
- 既存の post / reaction / DM フローが従来通り動作 (consensus 変更は frontend 不可視のはず)

## 10. 本番チューニング

### 10.1 初期 difficulty 決定
[`scripts/bench-randomx.sh`](../../../scripts/bench-randomx.sh) (新規) で参照ハードウェア (e.g. 8-core CPU) の hashrate を計測し、target 30s で 1 ブロックが解ける difficulty を逆算。

### 10.2 RandomX 本番設定
| 項目 | 値 |
|---|---|
| Mode | Full (2GB dataset) |
| Large pages | 推奨 (Linux: `vm.nr_hugepages`、Windows: SeLockMemoryPrivilege) |
| Hard-AES | 自動検出 (AES-NI) |
| JIT | 有効 |

ドキュメント: [`docs/operations/pow-mining-setup.md`](../../operations/pow-mining-setup.md) (新規)

### 10.3 Metrics (Prometheus)
node が公開:
- `anarchy_pow_hashrate_estimate` (gauge, hashes/s)
- `anarchy_pow_block_time_seconds` (histogram)
- `anarchy_pow_orphan_blocks_total` (counter)
- `anarchy_pow_difficulty` (gauge)
- `anarchy_grandpa_authority_rotations_total` (counter)
- `anarchy_grandpa_authority_set_size` (gauge)

## 11. 脅威モデル ([`docs/security/pow-threat-model.md`](../../security/pow-threat-model.md) 新規)

| 脅威 | 想定攻撃 | 緩和策 |
|---|---|---|
| 51% 攻撃 | 過半数 hashrate 確保で reorg | LWMA-3 で difficulty 急騰、コミュニティ hashrate 拡大、UX 側で confirmation 深度 ≥ 12 推奨 |
| Selfish mining | private chain で公開タイミング操作 | 影響あるが Bitcoin と同等。GRANDPA finality で公開チェーンが finalized されれば selfish chain は無効化 |
| Time warp | timestamp 操作で difficulty 下げ | `pallet_timestamp` の `MinimumPeriod` + 各ブロックの timestamp 単調性チェック |
| GRANDPA authority sybil | 1 attacker が複数 mining node で top-K 占拠 | top-K 占拠は技術的に可能だが、それには元々 hashrate が必要 = 51% 相当のコスト。NPoS でない以上、追加コスト障壁はないが consensus 安全性は PoW で守られる |
| RandomX seed 切替時の DoS | seed 切替時に dataset 再構築で停止 | epoch 境界の数ブロック前から並行 prebuild |
| Long-range attack | 創世から別 chain を構築 | GRANDPA finality が打ったブロックは fork choice で覆らない (Substrate 標準) |
| Equivocation (GRANDPA) | 同じ height で異なる vote | `pallet_grandpa::report_equivocation` 既存。authority key を `pallet_grandpa_authority_election` から自動 unregister |

## 12. Mainnet 投入ランブック ([`docs/operations/pow-mainnet-runbook.md`](../../operations/pow-mainnet-runbook.md) 新規)

CLAUDE.md "Compatibility Policy" に従い **chain reset 方式**:

1. `feature/pow-migration` を `main` にマージ
2. `apps/blockchain` を release build (production profile)
3. `production-spec.json` を `cargo run -- build-spec --chain production --raw` で生成
4. genesis bootstrap miner 1 名を選定し、その GRANDPA key を chain_spec に焼き込み
5. 旧チェーンの停止アナウンス → 新 genesis でローンチ
6. 最初の 600 ブロック (5 時間) で authority rotation が機能していることを確認
7. monitoring が green なら community mining 解放

migration code / state migration は **書かない** (CLAUDE.md ポリシー)。

## 13. マイルストーン

| # | 内容 | 期間目安 |
|---|---|---|
| M1 | stable2503 + `sc-consensus-pow` + `randomx-rs` の `cargo check` 通過 PoC | 1〜2 日 |
| M2 | `pallet_difficulty` 実装 + unit tests | 2 日 |
| M3 | `pallet_block_reward` 実装 + unit tests + halving 検証 | 2 日 |
| M4 | `pallet_grandpa_authority_election` 実装 + unit tests | 3 日 |
| M5 | `node/src/pow/` モジュール (RandomX algo / author / difficulty) | 3 日 |
| M6 | `service.rs` 改修, CLI, chain_spec 更新 | 2 日 |
| M7 | 1 ノード dev mining smoke 通過 | 1 日 |
| M8 | 3 ノード integration test (multi_miner / hashrate_jump / authority_rotation / selfish_mining / coinbase_inject) | 3 日 |
| M9 | RandomX 本番チューニング + Prometheus metrics + ベンチで初期 difficulty 確定 | 2 日 |
| M10 | 脅威モデル / mining setup / mainnet runbook の docs | 2 日 |
| M11 | PR / レビュー / TODO.md 4 sub-bullet を `[X]` / CONCEPTS.md "コンセンサス方式の検討" を完了マーク | 1 日 |

**合計**: 22〜24 営業日 (約 1 ヶ月)

## 14. 未解決 / フォローアップ

| # | 項目 | 対応方針 |
|---|---|---|
| F1 | Faucet (100 MORAL/req) と halving の整合 | mainnet ローンチ後に Faucet 報酬を halving 連動で減額 (別タスク) |
| F2 | Treasury / governance 連動 | TODO.md §4.4-4.6 と独立。PoW 完成後にガバナンス設計を別 spec で |
| F3 | Frontend での confirmation 深度 UX | post 確定の "確認中" 表示。本 spec の範囲外 (frontend tasks にリストアップ) |
| F4 | Light client (smoldot) での PoW header verify | smoldot は consensus algo agnostic だが、RandomX header verify は重い。light client は finality 信頼で済ませる |
| F5 | RandomX の wasm 互換性 (validation 用) | runtime は PoW verify を行わないので無関係 (verify は node 側のみ) |

# PoW Migration — Phase B 実装プラン (Runtime Cutover + Service + Tests + Docs)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Phase A で追加した pallet 3 つと node/pow モジュールを実際に runtime と service に配線し、Aura/GRANDPA PoA から RandomX PoW + Permissionless GRANDPA への consensus 切替を完了する。マージ時点で dev chain は PoW で動作するようになる (chain reset 必須)。

**Prerequisite:** Phase A PR (#52) が main にマージ済みであること。

**Branch:** `feature/pow-migration-cutover` (`main` から分岐)

**Spec:** [`docs/superpowers/specs/2026-05-06-pow-migration-design.md`](../specs/2026-05-06-pow-migration-design.md)

**Phase A 引継 (要対応)**:
- `RandomXAlgorithm::verify` は Phase A では `Ok(false)` stub。Task 2 で実 RandomX hash 計算に置換。
- `pallet_grandpa_authority_election` の rotation テストが session pallet 不在で `ScheduleChangeFailed` も valid として通っている。Task 5 / Task 7 の実 chain test で動作確認。
- `cargo fmt` 未実行 (rustfmt 未インストール)。Phase B 着手前に `rustup component add rustfmt`。

**Tech Stack (Phase A から継続):** Polkadot SDK stable2503 (FRAME), `sc-consensus-pow = "0.54.0"`, `sp-consensus-pow = "0.46.0"`, `randomx-rs = "1.4.1"`

---

## File Structure

新規作成:

| パス | 役割 |
|---|---|
| `apps/blockchain/tests/integration/pow/multi_miner.sh` | 3 ノード reorg + finality シナリオ |
| `apps/blockchain/tests/integration/pow/hashrate_jump.sh` | hashrate 急増 → DAA 収束 |
| `apps/blockchain/tests/integration/pow/authority_rotation.sh` | top-K rotation 連続性 |
| `apps/blockchain/tests/integration/pow/selfish_mining.sh` | 攻撃 reorg + finality 防御 |
| `apps/blockchain/tests/integration/pow/coinbase_inject.sh` | 不正 PreRuntime digest reject |
| `apps/blockchain/tests/integration/pow/README.md` | 5 シナリオの実行手順とハードウェア要件 |
| `scripts/bench-randomx.sh` | reference HW で hashrate 実測 → 初期 difficulty 算出 |
| `docs/security/pow-threat-model.md` | 51% / selfish / time warp / sybil / equivocation |
| `docs/operations/pow-mining-setup.md` | RandomX large pages / full mode / JIT 設定 |
| `docs/operations/pow-mainnet-runbook.md` | mainnet 投入手順 (chain reset 方式) |
| `.github/workflows/pow-smoke.yml` | CI: pallet unit + 1 ノード light smoke (新規 or 既存統合) |

修正:

| パス | 内容 |
|---|---|
| `apps/blockchain/runtime/src/lib.rs` | pallet_aura 削除 / 新 pallet 3 つ統合 / DifficultyApi 実装 / FindAuthor 配線 |
| `apps/blockchain/runtime/Cargo.toml` | pallet-aura → 削除、pallet-difficulty / pallet-block-reward / pallet-grandpa-authority-election 追加 |
| `apps/blockchain/node/src/service.rs` | sc_consensus_aura → sc_consensus_pow、start_aura → start_mining_worker |
| `apps/blockchain/node/src/cli.rs` | --mine / --coinbase / --randomx-mode フラグ追加 |
| `apps/blockchain/node/src/chain_spec.rs` | aura authorities 削除、production_config 追加、初期 difficulty |
| `apps/blockchain/node/src/pow/randomx_algo.rs` | verify を実 RandomX hash 計算に置換 |
| `docs/TODO.md` | §4.7 PoW 移行検討 4 sub-bullet を `[X]` |
| `docs/CONCEPTS.md` | "コンセンサス方式の検討" を完了マーク |

---

## Task 1: Runtime Integration — pallet_aura 撤廃 + 新 pallet 統合

**Files:**
- Modify: `apps/blockchain/runtime/Cargo.toml`
- Modify: `apps/blockchain/runtime/src/lib.rs`

### Step 1.1: Cargo.toml の pallet-aura 削除 + 新 pallet 追加

`apps/blockchain/runtime/Cargo.toml`:
- `pallet-aura = ...` 行を削除
- `[dependencies]` に追加:
  ```toml
  pallet-difficulty = { workspace = true }
  pallet-block-reward = { workspace = true }
  pallet-grandpa-authority-election = { workspace = true }
  ```
- `[features]` の `std` から `"pallet-aura/std"` を削除し、3 新 pallet の `/std` を追加
- `try-runtime` / `runtime-benchmarks` も同様に置換

注: `pallet-block-reward` と `pallet-grandpa-authority-election` を workspace deps に追加する必要があれば `apps/blockchain/Cargo.toml` の `[workspace.dependencies]` セクションに追記。

### Step 1.2: lib.rs から pallet_aura 関連を削除

`apps/blockchain/runtime/src/lib.rs`:

削除する箇所:
- `use sp_consensus_aura::sr25519::AuthorityId as AuraId;`
- `pub aura: Aura,` (SessionKeys 内)
- `// Aura設定` ブロック (`impl pallet_aura::Config for Runtime { ... }`)
- `pallet_timestamp::Config` 内の `type OnTimestampSet = Aura;` を `type OnTimestampSet = ();` に
- `construct_runtime!` から `Aura: pallet_aura,`
- `impl_runtime_apis!` から `impl sp_consensus_aura::AuraApi<Block, AuraId>` ブロック全体

### Step 1.3: 新 pallet 3 つの impl を lib.rs に追加

```rust
// pallet_difficulty
parameter_types! {
    pub const TargetBlockTime: Moment = 30_000;       // 30s
    pub const DifficultyAdjustWindow: u32 = 60;
    pub const MinDifficulty: sp_core::U256 = sp_core::U256([10_000, 0, 0, 0]);
}
impl pallet_difficulty::Config for Runtime {
    type TargetBlockTime = TargetBlockTime;
    type DifficultyAdjustWindow = DifficultyAdjustWindow;
    type MinDifficulty = MinDifficulty;
}

// pallet_block_reward
parameter_types! {
    pub const InitialBlockReward: Balance = 5 * UNITS;        // 5 MORAL = 5e12
    pub const HalvingPeriod: BlockNumber = 4_204_800;
    pub const MaxHalvings: u32 = 64;
}
impl pallet_block_reward::Config for Runtime {
    type Currency = Balances;
    type InitialReward = InitialBlockReward;
    type HalvingPeriod = HalvingPeriod;
    type MaxHalvings = MaxHalvings;
    type AuthorOrigin = PowAuthorAdapter;
}

// pallet_grandpa_authority_election
parameter_types! {
    pub const ElectionWindowSize: u32 = 100;
    pub const ElectionAuthorityCount: u32 = 10;
    pub const ElectionRotationPeriod: BlockNumber = 600;   // 5 hours @30s
    pub const ElectionRotationDelay: BlockNumber = 10;     // 5 minutes @30s
}
impl pallet_grandpa_authority_election::Config for Runtime {
    type WindowSize = ElectionWindowSize;
    type AuthorityCount = ElectionAuthorityCount;
    type RotationPeriod = ElectionRotationPeriod;
    type RotationDelay = ElectionRotationDelay;
    type AuthorOrigin = PowAuthorAdapter;
}
```

### Step 1.4: PowAuthorAdapter (FindAuthor) を runtime に追加

PowAuthor は node 側 (apps/blockchain/node/src/pow/author.rs) で `AccountId32` を返す。Runtime は `T::AccountId` を要求するため、adapter を作る:

```rust
/// PoW PreRuntime digest から author AccountId を抽出する。
/// Engine ID は node/src/pow/author.rs と一致させる: b"ANRC"
pub struct PowAuthorAdapter;
impl frame_support::traits::FindAuthor<AccountId> for PowAuthorAdapter {
    fn find_author<'a, I>(digests: I) -> Option<AccountId>
    where I: 'a + IntoIterator<Item = (sp_runtime::ConsensusEngineId, &'a [u8])> {
        const POW_ENGINE_ID: sp_runtime::ConsensusEngineId = *b"ANRC";
        for (id, mut data) in digests {
            if id == POW_ENGINE_ID {
                if let Ok(a) = <AccountId as parity_scale_codec::Decode>::decode(&mut data) {
                    return Some(a);
                }
            }
        }
        None
    }
}
```

### Step 1.5: construct_runtime! を更新

```rust
construct_runtime!(
    pub struct Runtime {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Balances: pallet_balances,
        // Aura: pallet_aura,                          ← 削除
        Grandpa: pallet_grandpa,
        Sudo: pallet_sudo,

        // PoW consensus
        Difficulty: pallet_difficulty,
        BlockReward: pallet_block_reward,
        GrandpaElection: pallet_grandpa_authority_election,

        // 既存 SNS pallets (順序維持)
        Post: pallet_post,
        ...
    }
);
```

### Step 1.6: impl_runtime_apis! に DifficultyApi を追加

```rust
impl pallet_difficulty::DifficultyApi<Block> for Runtime {
    fn difficulty() -> sp_core::U256 {
        Difficulty::current_difficulty()
    }
}
```

注: `pallet_difficulty::CurrentDifficulty::<Runtime>::get()` のヘルパとして `Difficulty::current_difficulty()` を提供する必要があれば pallet 側に `impl<T: Config> Pallet<T> { pub fn current_difficulty() -> U256 { CurrentDifficulty::<T>::get() } }` を追加 (Phase A 範囲に戻る微調整)。

### Step 1.7: SessionKeys から aura 削除

```rust
impl_opaque_keys! {
    pub struct SessionKeys {
        pub grandpa: Grandpa,
        // pub aura: Aura,    ← 削除
    }
}
```

### Step 1.8: ビルド検証

```bash
cd /home/moriwaki-y/self/anarchy/apps/blockchain && cargo build -p anarchy-runtime 2>&1 | tail -20
```

Expected: errors なし。あれば `Moment` 型 (u64) や `Balance`/`UNITS` 定数の参照を既存コードから引き当てて修正。

### Step 1.9: コミット

```bash
git add apps/blockchain/runtime/
git commit -m "feat(runtime): replace pallet_aura with pallet_difficulty + pallet_block_reward + pallet_grandpa_authority_election"
```

---

## Task 2: RandomX 実装 — randomx_algo.rs verify 実装

**Files:**
- Modify: `apps/blockchain/node/src/pow/randomx_algo.rs`

### Step 2.1: RandomX VM 初期化 + epoch 切替ロジックを追加

`randomx_rs` API の概要 (要 `cargo doc -p randomx-rs --open` で確認):
- `RandomXCache::new(flags, key)` でキャッシュ作成
- `RandomXDataset::new(flags, cache, init_thread_count)` で full dataset (2GB) 作成
- `RandomXVM::new(flags, Some(cache), Some(dataset))` で VM 作成
- `vm.calculate_hash(input: &[u8]) -> Vec<u8>` で hash 計算

```rust
use randomx_rs::{RandomXCache, RandomXDataset, RandomXFlag, RandomXVM};

pub struct RandomXVm {
    /// 現在の seed key (= block hash at epoch start)
    current_key: Vec<u8>,
    cache: RandomXCache,
    dataset: Option<RandomXDataset>,  // None = light mode
    vm: RandomXVM,
}

impl RandomXVm {
    pub fn new(key: &[u8], full_mode: bool) -> Result<Self, String> {
        let mut flags = RandomXFlag::default();
        if full_mode {
            flags |= RandomXFlag::FLAG_FULL_MEM;
        }
        flags |= RandomXFlag::FLAG_JIT;
        let cache = RandomXCache::new(flags, key)
            .map_err(|e| format!("randomx cache: {}", e))?;
        let dataset = if full_mode {
            Some(RandomXDataset::new(flags, &cache, 0)
                .map_err(|e| format!("randomx dataset: {}", e))?)
        } else { None };
        let vm = RandomXVM::new(flags, Some(&cache), dataset.as_ref())
            .map_err(|e| format!("randomx vm: {}", e))?;
        Ok(Self { current_key: key.to_vec(), cache, dataset, vm })
    }

    /// epoch 境界で seed が変わったら VM を作り直す。
    pub fn ensure_key(&mut self, key: &[u8], full_mode: bool) -> Result<(), String> {
        if self.current_key.as_slice() == key { return Ok(()); }
        *self = Self::new(key, full_mode)?;
        Ok(())
    }

    pub fn hash(&self, input: &[u8]) -> Vec<u8> {
        self.vm.calculate_hash(input).unwrap_or_default()
    }
}
```

### Step 2.2: PowAlgorithm::verify を実装

```rust
fn verify(
    &self,
    parent: &BlockId<B>,
    pre_hash: &H256,
    _pre_digest: Option<&[u8]>,
    seal: &RawSeal,
    difficulty: Self::Difficulty,
) -> Result<bool, PowError<B>> {
    let seal = PowSeal::decode(&mut seal.as_slice())
        .map_err(|e| PowError::Other(format!("seal decode: {:?}", e)))?;

    // RandomX seed key (= epoch 境界 block hash)
    let seed_key = self.seed_key_for(parent)?;

    // input = pre_hash || nonce
    let mut input = Vec::with_capacity(32 + 8);
    input.extend_from_slice(pre_hash.as_bytes());
    input.extend_from_slice(&seal.nonce.to_le_bytes());

    let mut vm = self._vm.lock().map_err(|_| PowError::Other("vm lock".into()))?;
    vm.ensure_key(&seed_key, self.full_mode)
        .map_err(|e| PowError::Other(format!("randomx ensure_key: {}", e)))?;
    let work_hash_bytes = vm.hash(&input);

    // work_hash <= U256::MAX / difficulty で判定 (Bitcoin 流派)
    let work_hash = U256::from_big_endian(&work_hash_bytes);
    let target = U256::MAX / difficulty;
    Ok(work_hash <= target)
}
```

### Step 2.3: seed_key_for(parent) ヘルパを追加

```rust
fn seed_key_for(&self, parent: &BlockId<B>) -> Result<Vec<u8>, PowError<B>> {
    use sc_client_api::BlockBackend;
    // epoch_boundary = floor(parent_number / RANDOMX_EPOCH_BLOCKS) * RANDOMX_EPOCH_BLOCKS
    // その epoch_boundary ブロックの hash を seed key にする
    let parent_hash = self.diff_client.client_arc().header(*parent.as_ref())
        .map_err(|e| PowError::Environment(format!("header: {:?}", e)))?
        .ok_or_else(|| PowError::Environment("parent header missing".into()))?;
    // ... (epoch 計算と client.block_hash 呼び出し — Phase B で詳細化)

    // Phase A 残り stub: parent_hash を使う簡易実装。
    Ok(parent_hash.hash().as_bytes().to_vec())
}
```

注: epoch boundary 計算は `BlockNumberFor<B>` 演算が必要。実装時に `num_traits::Saturating` や `sp_runtime::traits::SaturatedConversion` を使う。

### Step 2.4: full_mode フィールド追加 + new() 改修

```rust
pub struct RandomXAlgorithm<B: BlockT, C> {
    diff_client: DifficultyClient<C>,
    _vm: Arc<Mutex<RandomXVm>>,
    full_mode: bool,
    _phantom: std::marker::PhantomData<B>,
}

impl<B: BlockT, C> RandomXAlgorithm<B, C> where ... {
    pub fn new(client: Arc<C>, full_mode: bool, initial_seed: &[u8]) -> Self {
        let vm = RandomXVm::new(initial_seed, full_mode).expect("randomx init");
        Self {
            diff_client: DifficultyClient::new(client),
            _vm: Arc::new(Mutex::new(vm)),
            full_mode,
            _phantom: std::marker::PhantomData,
        }
    }
}
```

### Step 2.5: Cargo.toml で randomx-rs を default-features=true にし std を有効化

`apps/blockchain/node/Cargo.toml`:
```toml
randomx-rs = { workspace = true, default-features = true }  # 既に Phase A で設定済み
```

### Step 2.6: ビルド検証

```bash
cd /home/moriwaki-y/self/anarchy/apps/blockchain && cargo build -p anarchy-node 2>&1 | tail -20
```

Expected: 警告は OK、errors なし。`randomx-rs` の link error が出る場合は dev 環境に `libstdc++` または `cmake` が要求されることがある。

### Step 2.7: 単体テスト追加

`randomx_algo.rs` 末尾の `#[cfg(test)] mod tests` に:

```rust
#[test]
fn randomx_hash_is_deterministic() {
    let key = b"phase-b-test-key";
    let vm1 = RandomXVm::new(key, false).expect("init1");
    let vm2 = RandomXVm::new(key, false).expect("init2");
    let input = b"some input";
    assert_eq!(vm1.hash(input), vm2.hash(input));
}
```

注: light mode (full_mode=false) を使う — full mode は CI で 2GB メモリを食うため。

### Step 2.8: コミット

```bash
git add apps/blockchain/node/src/pow/randomx_algo.rs apps/blockchain/node/Cargo.toml
git commit -m "feat(node/pow): implement RandomX verify with VM init + epoch seed rotation"
```

---

## Task 3: service.rs 改修 — Aura → PoW 切替

**Files:**
- Modify: `apps/blockchain/node/src/service.rs`

### Step 3.1: Aura 関連の import / 型を削除

削除する `use` 文:
- `sc_consensus_aura::{ImportQueueParams, SlotProportion, StartAuraParams};`
- `sp_consensus_aura::sr25519::AuthorityPair as AuraPair;`

追加する `use` 文:
- `sc_consensus_pow::{PowBlockImport, start_mining_worker, MiningHandle};`
- `crate::pow::{RandomXAlgorithm, POW_ENGINE_ID};`
- `sp_runtime::AccountId32;`

### Step 3.2: import_queue を PoW に置換

旧 (削除):
```rust
let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
let import_queue = sc_consensus_aura::import_queue::<AuraPair, _, _, _, _, _>(...)?;
```

新 (追加):
```rust
let pow_algo = RandomXAlgorithm::new(
    client.clone(),
    false,                     // CI でも build できるよう default は light mode
    client.info().genesis_hash.as_bytes(),
);

let pow_block_import = PowBlockImport::new(
    grandpa_block_import.clone(),
    client.clone(),
    pow_algo.clone(),
    0,                         // check_inherents_after
    select_chain.clone(),
    move |_, ()| async {
        let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
        Ok(timestamp)
    },
);

let import_queue = sc_consensus_pow::import_queue(
    Box::new(pow_block_import.clone()),
    None,                      // GRANDPA は justification として別途 import
    pow_algo.clone(),
    &task_manager.spawn_essential_handle(),
    config.prometheus_registry(),
)?;
```

### Step 3.3: start_aura → start_mining_worker

旧 (削除):
```rust
let aura = sc_consensus_aura::start_aura::<AuraPair, ...>(StartAuraParams { ... })?;
task_manager.spawn_essential_handle().spawn_blocking("aura", Some("block-authoring"), aura);
```

新 (追加, `cli.run.mine` フラグ true 時のみ):
```rust
if cli.run.mine {
    let coinbase: AccountId32 = cli.run.coinbase.clone()
        .ok_or_else(|| sc_service::Error::Other("--coinbase required when mining".into()))?
        .parse()
        .map_err(|e| sc_service::Error::Other(format!("invalid coinbase: {}", e)))?;
    let pre_runtime = vec![(POW_ENGINE_ID, coinbase.encode())];

    let proposer = sc_basic_authorship::ProposerFactory::new(
        task_manager.spawn_handle(),
        client.clone(),
        transaction_pool.clone(),
        config.prometheus_registry(),
        telemetry.as_ref().map(|t| t.handle()),
    );

    let (_handle, worker_task) = start_mining_worker(
        Box::new(pow_block_import),
        client.clone(),
        select_chain,
        pow_algo,
        proposer,
        sync_service.clone(),
        sync_service.clone(),
        Some(pre_runtime),
        move |_, ()| async {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
            Ok(timestamp)
        },
        std::time::Duration::from_secs(10),
        std::time::Duration::from_secs(5),
    );

    task_manager.spawn_essential_handle()
        .spawn_blocking("pow-mining-worker", Some("block-authoring"), worker_task);
}
```

### Step 3.4: GRANDPA voter は維持 (現状通り)

`enable_grandpa` ブロックは触らない。Phase A の `pallet_grandpa_authority_election` が `schedule_change` を呼ぶことで authority set が動的に更新される。

### Step 3.5: ビルド検証

```bash
cd /home/moriwaki-y/self/anarchy/apps/blockchain && cargo build -p anarchy-node 2>&1 | tail -30
```

`sc_consensus_pow::PowBlockImport::new` シグネチャは stable2503 で異なる可能性あり。errors が出たら `cargo doc -p sc-consensus-pow --open` で確認して引数を合わせる。

### Step 3.6: コミット

```bash
git add apps/blockchain/node/src/service.rs
git commit -m "feat(service): replace Aura with sc_consensus_pow + RandomX mining worker"
```

---

## Task 4: CLI + chain_spec

**Files:**
- Modify: `apps/blockchain/node/src/cli.rs`
- Modify: `apps/blockchain/node/src/chain_spec.rs`

### Step 4.1: cli.rs に PoW 用フラグ追加

```rust
#[derive(Debug, Clone, clap::Parser)]
pub struct RunCmd {
    #[clap(flatten)]
    pub base: sc_cli::RunCmd,

    /// マイニングを有効化する。
    #[arg(long)]
    pub mine: bool,

    /// マイナー報酬を受け取るアカウント (SS58 アドレス)。
    /// `--mine` 指定時は必須。
    #[arg(long)]
    pub coinbase: Option<String>,

    /// RandomX のメモリモード。`fast` (full 2GB) または `light` (256MB)。
    #[arg(long, default_value = "light")]
    pub randomx_mode: String,
}
```

注: 既存 cli.rs の構造に合わせて統合する。`Cli` struct と `Subcommand` enum 構成は触らない。

### Step 4.2: chain_spec.rs から aura authorities を削除

`authority_keys_from_seed` を以下に変更:
```rust
pub fn authority_keys_from_seed(s: &str) -> GrandpaId {
    get_from_seed::<GrandpaId>(s)
}
```

`testnet_genesis` から aura 関連 (`AuraConfig`, `aura authorities`) を削除し、初期 difficulty を追加:
```rust
fn testnet_genesis(
    initial_authorities: Vec<GrandpaId>,    // (AuraId, GrandpaId) → GrandpaId のみに
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
) -> serde_json::Value {
    serde_json::json!({
        "balances": { "balances": ... },
        "grandpa": {
            "authorities": initial_authorities.iter().map(|x| (x.clone(), 1u64)).collect::<Vec<_>>(),
        },
        "sudo": { "key": Some(root_key) },
        "difficulty": {
            "initialDifficulty": "0x186a0",      // 100_000、Task 9 の bench で再決定
            "_phantom": null
        },
        // pallet_block_reward / pallet_grandpa_authority_election は genesis なし
    })
}
```

### Step 4.3: production_config を追加

```rust
pub fn production_config() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        wasm_binary_unwrap(),
        None,
    )
    .with_name("Anarchy")
    .with_id("anarchy")
    .with_chain_type(ChainType::Live)
    .with_genesis_config_patch(testnet_genesis(
        vec![/* genesis bootstrap miner GRANDPA key を投入時に焼き込み */],
        /* sudo key — mainnet では None 推奨 */ None,
        vec![],
    ))
    .with_protocol_id("anarchy")
    .with_properties(chain_properties())
    .build())
}
```

### Step 4.4: command.rs から `--chain production` ルーティングを追加

(cli.rs / command.rs 両方を修正する場合あり)

### Step 4.5: ビルド検証

```bash
cd /home/moriwaki-y/self/anarchy/apps/blockchain && cargo build --release -p anarchy-node 2>&1 | tail -20
```

### Step 4.6: コミット

```bash
git add apps/blockchain/node/src/cli.rs apps/blockchain/node/src/chain_spec.rs apps/blockchain/node/src/command.rs
git commit -m "feat(node): add --mine / --coinbase / --randomx-mode CLI + production chain_spec (PoW genesis)"
```

---

## Task 5: 1 ノード dev mining smoke テスト

**Files:** なし (実機検証のみ)

### Step 5.1: dev chain で mining を起動

```bash
cd /home/moriwaki-y/self/anarchy/apps/blockchain
./target/release/anarchy-node --dev --mine \
    --coinbase 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY \
    --randomx-mode light \
    --tmp 2>&1 | head -50
```

(coinbase は //Alice の SS58 アドレス。--tmp で chain dir を毎回 clean に。)

Expected: 30 秒以内に最初のブロックが finalized される。stdout に:
```
🔨 Imported #1 (0x...→0x...)
🏆 Successfully mined block on top of #0
```

### Step 5.2: 5 分間放置してブロック生成継続を確認

```bash
# 5 分後 Ctrl-C で停止
# block height が 5+ 程度に達していれば PASS
```

GRANDPA finality が進行することも確認 (`👴 Finalized #N` ログ)。

### Step 5.3: 失敗時のデバッグ

- 「block stalls」: difficulty が高すぎる → chain_spec の `initialDifficulty` を 1_000 に下げて再試行
- 「verify returned false」: RandomX hash 計算と target 比較のロジックを debug log で確認
- 「No author found」: PreRuntime digest の coinbase encode/decode を確認

### Step 5.4: smoke 通過したらコミット (実機ログを記録)

```bash
# smoke の証跡として PR description に貼るためログをファイルに保存
./target/release/anarchy-node --dev --mine ... > /tmp/smoke.log 2>&1 &
sleep 300 && pkill anarchy-node
git add docs/operations/pow-smoke-evidence.md   # smoke ログのサマリを書いておく
git commit -m "docs: capture PoW dev mining smoke evidence (5min stable @light mode)"
```

---

## Task 6: CI 統合 — pallet unit + 1 ノード light smoke

**Files:**
- Create or Modify: `.github/workflows/pow-smoke.yml`

### Step 6.1: 既存 CI 構成を確認

```bash
ls /home/moriwaki-y/self/anarchy/.github/workflows/
cat /home/moriwaki-y/self/anarchy/.github/workflows/<existing>.yml
```

PoW smoke ジョブを既存ワークフローに足すか、新ファイルを切るか判断。

### Step 6.2: pow-smoke.yml を作成

```yaml
name: PoW Smoke

on:
  pull_request:
    paths:
      - 'apps/blockchain/**'
      - '.github/workflows/pow-smoke.yml'
  push:
    branches: [main]

jobs:
  pallet-units:
    runs-on: ubuntu-latest
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
          targets: wasm32v1-none
      - uses: Swatinem/rust-cache@v2
      - run: cd apps/blockchain && cargo test -p pallet-difficulty -p pallet-block-reward -p pallet-grandpa-authority-election

  one-node-light-smoke:
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32v1-none
      - uses: Swatinem/rust-cache@v2
      - run: cd apps/blockchain && cargo build --release -p anarchy-node
      - run: |
          cd apps/blockchain
          ./target/release/anarchy-node --dev --mine \
            --coinbase 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY \
            --randomx-mode light --tmp > /tmp/smoke.log 2>&1 &
          NODE_PID=$!
          sleep 300
          kill $NODE_PID || true
          # block height が 5 以上に達していること
          BLOCK=$(grep -oP 'Imported #\K[0-9]+' /tmp/smoke.log | tail -1)
          test -n "$BLOCK" && test "$BLOCK" -ge 5 || (cat /tmp/smoke.log; exit 1)
```

### Step 6.3: コミット + push して CI 実行確認

```bash
git add .github/workflows/pow-smoke.yml
git commit -m "ci: add PoW smoke (pallet units + 1-node light mining 5min)"
git push
# GitHub Actions の job 結果を確認
gh run watch
```

CI 失敗時はログを見て修正。よくある問題: build キャッシュ不足で 30 分超過 → cargo cache ヒット率を上げる、or smoke を 60s に短縮。

---

## Task 7: Staging Integration Tests (5 シナリオ)

**Files:**
- Create: `apps/blockchain/tests/integration/pow/multi_miner.sh`
- Create: `apps/blockchain/tests/integration/pow/hashrate_jump.sh`
- Create: `apps/blockchain/tests/integration/pow/authority_rotation.sh`
- Create: `apps/blockchain/tests/integration/pow/selfish_mining.sh`
- Create: `apps/blockchain/tests/integration/pow/coinbase_inject.sh`
- Create: `apps/blockchain/tests/integration/pow/README.md`

### Step 7.1: 既存 integration test の流儀確認

```bash
ls /home/moriwaki-y/self/anarchy/apps/blockchain/tests/integration/
cat <既存 .sh の代表例>
```

### Step 7.2: multi_miner.sh

```bash
#!/usr/bin/env bash
set -euo pipefail

# 3 ノード (各 --mine 別 coinbase, full mode) で 30 分稼働、reorg 観察、
# GRANDPA finality 各ノード一致を確認。
# 要 16GB+ RAM (3 × 2GB scratchpad + node 状態)。

BIN="${BIN:-./target/release/anarchy-node}"
COINBASE_A="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"  # //Alice
COINBASE_B="5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"  # //Bob
COINBASE_C="5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy"  # //Charlie

# ノード起動 (省略 — 詳細はスクリプト本体)
# 各ノードの finalized height を取得して比較
# divergence > 3 blocks ならエラー
```

(完全な内容は実装時にエンジニアが既存 integration スクリプトに合わせて書く)

### Step 7.3〜7.6: 残り 4 シナリオを同様に作成

- `hashrate_jump.sh`: 起動済 3 ノードに +5 ノードを途中投入 → DAA が 60 ブロック以内に target に再収束
- `authority_rotation.sh`: 10 ノードに `register_grandpa_key` を発行 → 600 ブロック後 authority set がローテすることを `state_call` 経由で確認
- `selfish_mining.sh`: 2 ノード (1 公開、1 隠匿) で 6 ブロック先行 → publish 時に reorg 観察、ただし finalized は守られる
- `coinbase_inject.sh`: 不正な PreRuntime digest を持つ block を sync → reject されることを確認 (これは実装が PowAuthor の AccountId32 decode を行うため、garbled bytes は自動で None になり block reward が出ない経路を通る)

### Step 7.7: README.md

```markdown
# PoW Integration Tests

mainnet 投入前ゲートで運用者が手動実行するシナリオ集。
最低 16GB RAM 推奨 (3 ノード × 2GB RandomX scratchpad)。

## 前提
- `apps/blockchain` を release build 済み (`cargo build --release -p anarchy-node`)
- `randomx_mode = fast` (full 2GB dataset) で実行

## 実行手順
```
cd apps/blockchain
./tests/integration/pow/multi_miner.sh
./tests/integration/pow/hashrate_jump.sh
./tests/integration/pow/authority_rotation.sh
./tests/integration/pow/selfish_mining.sh
./tests/integration/pow/coinbase_inject.sh
```

各スクリプト終了コード 0 で PASS、それ以外で FAIL。
```

### Step 7.8: 各シナリオを手動実行して PASS 確認

実機で 5 シナリオすべてを通す。Ngecept failed なら PR をブロック。

### Step 7.9: コミット

```bash
git add apps/blockchain/tests/integration/pow/
git commit -m "test(integration): add 5 PoW staging scenarios (multi_miner / hashrate_jump / authority_rotation / selfish_mining / coinbase_inject)"
```

---

## Task 8: 本番チューニング — Prometheus + bench-randomx.sh + 初期 difficulty

**Files:**
- Create: `scripts/bench-randomx.sh`
- Modify: `apps/blockchain/node/src/service.rs` (Prometheus metrics)
- Modify: `apps/blockchain/node/src/chain_spec.rs` (production_config の initialDifficulty)

### Step 8.1: bench-randomx.sh

```bash
#!/usr/bin/env bash
# Reference HW (8-core CPU) で RandomX hashrate を実測し、
# 30s/block 1 ノード mining 想定の初期 difficulty を出力する。

set -euo pipefail
DURATION_S=60

# /tmp/bench-anarchy で chain init して mining を回す
TMPDIR=$(mktemp -d)
./target/release/anarchy-node --dev --mine \
    --coinbase 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY \
    --randomx-mode fast \
    --base-path "$TMPDIR" 2>&1 > /tmp/bench-randomx.log &
PID=$!
sleep "$DURATION_S"
kill $PID

# log から hashrate 推定: ブロック数 / DURATION_S × difficulty
BLOCKS=$(grep -c "Imported #" /tmp/bench-randomx.log)
echo "Blocks in ${DURATION_S}s: $BLOCKS"
# initial_difficulty = blocks_per_target_time × current_difficulty
```

### Step 8.2: Prometheus metrics 追加

`service.rs` または専用モジュール (`apps/blockchain/node/src/metrics.rs` 新規) で:
- `anarchy_pow_hashrate_estimate` (gauge)
- `anarchy_pow_block_time_seconds` (histogram)
- `anarchy_pow_orphan_blocks_total` (counter)
- `anarchy_pow_difficulty` (gauge)
- `anarchy_grandpa_authority_rotations_total` (counter)
- `anarchy_grandpa_authority_set_size` (gauge)

サブスタンス的に既存の `prometheus_registry()` に register する。

### Step 8.3: bench で得られた値で chain_spec.rs の initialDifficulty を更新

```rust
"difficulty": {
    "initialDifficulty": "0x...",   // bench-randomx.sh の出力値を 16 進で
    ...
}
```

### Step 8.4: コミット

```bash
git add scripts/bench-randomx.sh apps/blockchain/node/src/service.rs apps/blockchain/node/src/chain_spec.rs
git commit -m "feat(node): add Prometheus PoW metrics + bench-randomx.sh + tuned initial difficulty"
```

---

## Task 9: Docs (3 files)

**Files:**
- Create: `docs/security/pow-threat-model.md`
- Create: `docs/operations/pow-mining-setup.md`
- Create: `docs/operations/pow-mainnet-runbook.md`

### Step 9.1: pow-threat-model.md

[`docs/superpowers/specs/2026-05-06-pow-migration-design.md`](../specs/2026-05-06-pow-migration-design.md) §11 を expand して書く。各脅威 (51% / Selfish / Time warp / GRANDPA sybil / RandomX seed DoS / Long-range / Equivocation) に対し:
- 攻撃シナリオ
- コスト試算
- 緩和策の現状
- 残存リスクと対応方針

### Step 9.2: pow-mining-setup.md

運用者向けマニング ノード setup ガイド:
- ハードウェア要件 (CPU / RAM 16GB+ / SSD)
- Linux: large pages 設定 (`vm.nr_hugepages=1280`)
- Windows: SeLockMemoryPrivilege 付与
- `--randomx-mode fast` 推奨
- coinbase 用 SS58 アドレス生成 (subkey)
- systemd unit のサンプル

### Step 9.3: pow-mainnet-runbook.md

[`docs/superpowers/specs/2026-05-06-pow-migration-design.md`](../specs/2026-05-06-pow-migration-design.md) §12 を expand:
- 投入前ゲート (release checklist)
- 投入手順 (chain reset 方式)
- ローンチ後の監視項目
- インシデント対応

### Step 9.4: コミット

```bash
git add docs/security/pow-threat-model.md docs/operations/pow-mining-setup.md docs/operations/pow-mainnet-runbook.md
git commit -m "docs: PoW threat model + mining setup guide + mainnet runbook"
```

---

## Task 10: TODO.md / CONCEPTS.md 更新 + Phase B PR

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/CONCEPTS.md`

### Step 10.1: TODO.md §4.7 PoW 移行検討 を完了マーク

```markdown
- [X] **PoW移行検討** (完了 2026-05-NN, PR #XX)
  - [X] アルゴリズム選定: RandomX 採用 (ASIC 耐性 / Anarchy 原則と整合)
  - [X] ASIC耐性の要否判断: 必要 (匿名・誰でも参加できる原則)
  - [X] 難易度調整アルゴリズム実装: LWMA-3 (Kulupu 流派, unweighted harmonic mean)
  - [X] ファイナリティ方式変更: PoW + Permissionless GRANDPA (top-K miner rotation)

- [X] **NPoS（Hybrid）検討** → 不採用 (Permissionless GRANDPA で代替)
- [X] **移行計画** → mainnet runbook (docs/operations/pow-mainnet-runbook.md) 完成
```

### Step 10.2: CONCEPTS.md 「コンセンサス方式の検討」を完了マーク

```markdown
## ~~コンセンサス方式の検討（PoA → PoW）~~ → 完了 (2026-05-NN)

実装済み: RandomX PoW + Permissionless GRANDPA (top-K miner rotation)
詳細: `docs/superpowers/specs/2026-05-06-pow-migration-design.md`
```

### Step 10.3: ワークスペース全体の最終 cargo test / clippy

```bash
cd /home/moriwaki-y/self/anarchy/apps/blockchain
cargo test --workspace 2>&1 | tail -5
cargo clippy -p pallet-difficulty -p pallet-block-reward -p pallet-grandpa-authority-election -p anarchy-runtime -p anarchy-node -- -D warnings 2>&1 | tail -10
```

Expected: 全件 PASS、新コードに clippy error なし。

### Step 10.4: コミット + push + PR

```bash
git add docs/TODO.md docs/CONCEPTS.md
git commit -m "docs: mark PoW migration § as completed (TODO.md §4.7 + CONCEPTS.md)"
git push -u origin feature/pow-migration-cutover

gh pr create --base main --title "PoW migration Phase B: runtime cutover + tests + docs" --body "$(cat <<'EOF'
## Summary

Phase A (#52) で追加した pallet 3 つと node/pow モジュールを実際に runtime と service に配線し、Aura/GRANDPA PoA から RandomX PoW + Permissionless GRANDPA への consensus 切替を完了します。**マージ時点で dev chain は PoW で動作するようになり、chain reset (新 genesis 投入) が必須**になります。

詳細仕様: docs/superpowers/specs/2026-05-06-pow-migration-design.md
実装プラン: docs/superpowers/plans/2026-05-06-pow-migration-phase-b.md

## 主な変更

- runtime/src/lib.rs: pallet_aura 撤廃、新 pallet 3 つ統合、DifficultyApi 実装、PowAuthorAdapter (FindAuthor) 追加
- node/src/service.rs: sc_consensus_aura → sc_consensus_pow (PowBlockImport + start_mining_worker)
- node/src/pow/randomx_algo.rs: verify を実 RandomX hash 計算に置換 (VM init + epoch seed rotation)
- node/src/cli.rs: --mine / --coinbase / --randomx-mode フラグ
- node/src/chain_spec.rs: aura keys 削除、production_config 追加、初期 difficulty (bench で実測)
- 5 staging integration scenarios (multi_miner / hashrate_jump / authority_rotation / selfish_mining / coinbase_inject)
- CI: pallet unit + 1-node light smoke (5min)
- Prometheus metrics (hashrate / block_time / orphan / difficulty / authority rotations)
- 3 docs (threat model / mining setup / mainnet runbook)
- TODO.md §4.7 を完了マーク、CONCEPTS.md コンセンサス検討を完了マーク

## ⚠️ 破壊的変更

main にマージすると **dev chain は新 genesis で PoW として起動する**。既存の dev DB / chainspec は破棄して `--tmp` または `--base-path <new_dir>` でゼロから起動してください (CLAUDE.md ポリシー: migration code は書かない)。

## Test plan

- [x] cargo test --workspace 通過
- [x] cargo clippy 新コードで -D warnings clean
- [x] 1-node dev mining smoke (5min, light mode) PASS
- [x] 5 staging integration scenarios PASS (full mode, 16GB+ RAM 環境)
- [x] CI: pallet-units + one-node-light-smoke PASS
- [ ] レビュアー: docs/security/pow-threat-model.md レビュー
- [ ] レビュアー: docs/operations/pow-mainnet-runbook.md レビュー (mainnet 運用者観点)

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

**Spec coverage** (spec §1〜§14):
- §1 確定パラメータ → Task 1 (runtime config), Task 4 (chain_spec)
- §4.5 SessionKeys / Runtime APIs → Task 1.7 / 1.6
- §5.1 RandomXAlgorithm verify → Task 2
- §5.3 service.rs → Task 3
- §5.5 CLI → Task 4.1
- §7 chain_spec → Task 4.2-4.3
- §9.3 CI vs Staging → Task 6 / Task 7
- §10 本番チューニング → Task 8
- §11 脅威モデル → Task 9.1
- §12 mainnet runbook → Task 9.3
- §13.2 Phase B M6-M13 → Task 1-10 で 1:1 対応

**Placeholder scan**: "後で" / "TBD" / "省略" は実装ステップに残っているが、それぞれ「エンジニアが既存パターンに合わせる」「実機検証」など、自己完結した指示を伴う。

**Type consistency**:
- POW_ENGINE_ID = b"ANRC" (Phase A node author.rs と Task 1.4 PowAuthorAdapter で一致)
- AccountId / Balance / BlockNumber は既存 runtime/lib.rs の型エイリアスを参照
- ConsensusEngineId は sp_runtime 経由 (stable2503 の場所)

**Phase A 引継 issue 対応**:
- RandomX verify stub → Task 2 で実装 ✓
- GRANDPA rotation 実 chain test → Task 7 authority_rotation.sh で対応 ✓
- cargo fmt → Task 0 として `rustup component add rustfmt` を Task 1 着手前に必ず実行

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-05-06-pow-migration-phase-b.md`.**

Phase B は Phase A に比べ:
- 実機検証 (Task 5, Task 7) を含むため subagent だけでは完結しない
- service.rs / runtime/lib.rs の改修は副作用が広く慎重なレビューが必須

推奨アプローチ:
1. Task 1, 2, 3, 4 (Rust 改修) は subagent-driven で進める
2. Task 5, 7 (実機 smoke / staging) は人間が手で実行 + 結果を貼り付け
3. Task 6 (CI) は subagent でファイル作成 + push、結果を gh CLI で確認
4. Task 8, 9 (チューニング / docs) は subagent で原案 → 人間でレビュー
5. Task 10 (PR) は subagent

Phase A PR (#52) のレビュー / マージ完了を待ってから Phase B 着手すること。

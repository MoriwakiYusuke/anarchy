//! Anarchy Runtime
//!
//! 匿名分散型SNSのランタイム実装

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use parity_scale_codec::Decode;
use sp_api::impl_runtime_apis;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
    generic, impl_opaque_keys, Perbill, Permill,
    traits::{BlakeTwo256, Block as BlockT, IdentifyAccount, NumberFor, One, Verify},
    transaction_validity::{TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, MultiSignature,
};
use sp_std::borrow::Cow;
use sp_std::prelude::*;
use sp_version::RuntimeVersion;

/// LazyBlock type alias
pub type LazyBlock = sp_runtime::generic::LazyBlock<Header, UncheckedExtrinsic>;

use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{ConstU128, ConstU16, ConstU32, ConstU64, ConstU8},
    weights::{ConstantMultiplier, Weight},
};
use pallet_grandpa::AuthorityList as GrandpaAuthorityList;
use pallet_transaction_payment::{ConstFeeMultiplier, FungibleAdapter, Multiplier};

/// アカウントの署名に使用する型
pub type Signature = MultiSignature;

/// 署名から派生するアカウント識別子の型
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// 残高の型
pub type Balance = u128;

/// ブロック番号の型
pub type BlockNumber = u32;

/// インデックスの型
pub type Nonce = u32;

/// ブロックハッシュの型
pub type Hash = sp_core::H256;

/// Opaque types
pub mod opaque {
    use super::*;
    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    pub type BlockId = generic::BlockId<Block>;

    impl_opaque_keys! {
        pub struct SessionKeys {
            pub grandpa: Grandpa,
        }
    }
}

/// Runtime version
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: Cow::Borrowed("anarchy"),
    impl_name: Cow::Borrowed("anarchy"),
    authoring_version: 1,
    spec_version: 108,  // Bumped: TSTS economic model v1 (block_reward fan-out + tail, base_fee, storage_stake, dynamic γ, stealth wiring, faucet cap)
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 3,  // New extrinsics: lock_for_rewards / unlock_reactor / bond / request_release / finalize_release
    system_version: 1,
};

/// Block時間（ミリ秒）— PoW ターゲット 30 秒
pub const MILLISECS_PER_BLOCK: u64 = 30_000;

/// Slot間隔
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

/// トランザクション処理に使用可能なブロック時間の割合（75%）
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// ブロックサイズ制限（5MB）
pub const MAXIMUM_BLOCK_LENGTH: u32 = 5 * 1024 * 1024;

/// ブロック重み制限
/// - ref_time: 2秒分 (2_000_000_000_000 picoseconds)
/// - proof_size: 5MB (ステートプルーフ上限)
pub const MAXIMUM_BLOCK_WEIGHT: Weight =
    Weight::from_parts(2_000_000_000_000, 5 * 1024 * 1024);

/// Native version
#[cfg(feature = "std")]
pub fn native_version() -> sp_version::NativeVersion {
    sp_version::NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

// Frame System設定
parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    /// BlockWeights: ブロック重み制限を設定
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::builder()
            .base_block(Weight::from_parts(5_000_000, 0))
            .for_class(frame_support::dispatch::DispatchClass::all(), |weights| {
                weights.base_extrinsic = Weight::from_parts(125_000_000, 0);
            })
            .for_class(frame_support::dispatch::DispatchClass::Normal, |weights| {
                weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
            })
            .for_class(frame_support::dispatch::DispatchClass::Operational, |weights| {
                weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
                weights.reserved = Some(
                    MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
                );
            })
            .avg_block_initialization(Perbill::from_percent(10))
            .build_or_panic();
    /// BlockLength: ブロックサイズ制限を設定
    pub BlockLength: frame_system::limits::BlockLength =
        frame_system::limits::BlockLength::max_with_normal_ratio(
            MAXIMUM_BLOCK_LENGTH,
            NORMAL_DISPATCH_RATIO,
        );
}

#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
    type Block = Block;
    type BlockWeights = BlockWeights;
    type BlockLength = BlockLength;
    type AccountId = AccountId;
    type Nonce = Nonce;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type Lookup = sp_runtime::traits::AccountIdLookup<AccountId, ()>;
    // (#38-MED-4) headroom for additional pallets (storage / messaging / stealth / nickname).
    // Bumped from 16 → 64 so accounts referenced by many pallets are not rejected with
    // `TooManyConsumers` once we add features beyond the original 16-pallet baseline.
    type MaxConsumers = ConstU32<64>;
    type AccountData = pallet_balances::AccountData<Balance>;
    type SS58Prefix = ConstU16<42>; // Substrate generic (5で始まるアドレス)
}

// Grandpa設定
// NOTE: EquivocationReportSystem = () で二重署名報告は無効化。
// 将来的にPoWコンセンサスへ移行予定のため、スラッシング機構は不要。
// PoWでは計算コストが攻撃抑止力となり、Equivocation対策は本質的に解決される。
impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = ConstU32<32>;
    type MaxNominators = ConstU32<0>;
    type MaxSetIdSessionEntries = ConstU64<0>;
    type KeyOwnerProof = sp_core::Void;
    type EquivocationReportSystem = ();
}

// Timestamp設定
impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = ();   // Aura を削除済み。PoW では timestamp が強制される
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = ();
}

// PoW difficulty (LWMA-3)
parameter_types! {
    pub const TargetBlockTime: u64 = MILLISECS_PER_BLOCK;       // 30_000ms
    pub const DifficultyAdjustWindow: u32 = 60;
    // 注: 100 は dev/smoke 向け floor。WSL2 等で RandomX が ~30H/s しか出ない環境でも
    // ブロックが進むよう保守的に設定。production 投入前に bench-randomx.sh で再算出する
    // (spec §1 では 10_000 推奨)。
    pub const MinDifficulty: sp_core::U256 = sp_core::U256([100, 0, 0, 0]);
}
impl pallet_difficulty::Config for Runtime {
    type TargetBlockTime = TargetBlockTime;
    type DifficultyAdjustWindow = DifficultyAdjustWindow;
    type MinDifficulty = MinDifficulty;
}

/// PoW author 抽出アダプタ。PreRuntime ダイジェストから sc_consensus_pow が書き込む
/// engine ID `b"pow_"` (= `sp_consensus_pow::POW_ENGINE_ID`) を探し、SCALE デコードで
/// AccountId を取り出す。chain ローカルな別 ID (例: `b"ANRC"`) を使うと sc_consensus_pow
/// のマイニングワーカが書き込む PreRuntime と一致せず author が抽出できない。
pub struct PowAuthorAdapter;
impl frame_support::traits::FindAuthor<AccountId> for PowAuthorAdapter {
    fn find_author<'a, I>(digests: I) -> Option<AccountId>
    where
        I: 'a + IntoIterator<Item = (sp_runtime::ConsensusEngineId, &'a [u8])>,
    {
        // sp_consensus_pow::POW_ENGINE_ID = [b'p', b'o', b'w', b'_'] と同値。
        // runtime は sp-consensus-pow に依存させたくないので生バイトで記述する。
        const POW_ENGINE_ID: sp_runtime::ConsensusEngineId = *b"pow_";
        for (id, mut data) in digests {
            if id == POW_ENGINE_ID {
                if let Ok(a) = AccountId::decode(&mut data) {
                    return Some(a);
                }
            }
        }
        None
    }
}

// Block reward (TSTS v1: 3-way fan-out + tail emission)
//
// 旧モデル (v0): 100% miner mint, halving 64 回後に 0 → 51% 攻撃コスト 0
// 新モデル (v1): 50% miner / 30% storage pool / 20% reaction pool, tail = 0.5 MORAL
//
// 詳細: docs/economic_model_proposal.md §3.2.1
parameter_types! {
    pub const InitialBlockReward: Balance = 5_000_000_000_000;       // 5 MORAL = 5e12
    pub const TailEmission: Balance = 500_000_000_000;                // 0.5 MORAL = 5e11 (永続)
    pub const HalvingPeriod: BlockNumber = 4_204_800;                 // ~4 years @30s
    pub const MaxHalvings: u32 = 64;
    pub MinerSharePermill: Permill = Permill::from_percent(50);
    pub StorageSharePermill: Permill = Permill::from_percent(30);
    pub ReactionSharePermill: Permill = Permill::from_percent(20);
}

/// Storage プール sink: pallet_storage::do_deposit_to_reward_pool に転送する adapter。
///
/// `pallet_block_reward::PoolDeposit` が要求する単純な `do_deposit(u128)` を
/// `pallet_storage::StorageInterface` の同名メソッドに直接 forward する。
pub struct StoragePoolSinkAdapter;
impl pallet_block_reward::pallet::PoolDeposit for StoragePoolSinkAdapter {
    fn do_deposit(amount: u128) {
        <pallet_storage::Pallet<Runtime> as pallet_storage::StorageInterface<
            AccountId,
            BlockNumber,
        >>::do_deposit_to_reward_pool(amount);
    }
}

/// Reaction プール sink: pallet_reaction::do_deposit_to_reaction_pool に転送する adapter。
pub struct ReactionPoolSinkAdapter;
impl pallet_block_reward::pallet::PoolDeposit for ReactionPoolSinkAdapter {
    fn do_deposit(amount: u128) {
        <pallet_reaction::Pallet<Runtime> as pallet_reaction::ReactionInterface>::do_deposit_to_reaction_pool(amount);
    }
}

impl pallet_block_reward::Config for Runtime {
    type Currency = Balances;
    type InitialReward = InitialBlockReward;
    type TailEmission = TailEmission;
    type HalvingPeriod = HalvingPeriod;
    type MaxHalvings = MaxHalvings;
    type AuthorOrigin = PowAuthorAdapter;
    type MinerSharePermill = MinerSharePermill;
    type StorageSharePermill = StorageSharePermill;
    type ReactionSharePermill = ReactionSharePermill;
    type StoragePoolSink = StoragePoolSinkAdapter;
    type ReactionPoolSink = ReactionPoolSinkAdapter;
}

// GRANDPA authority election (top-K miner rotation)
parameter_types! {
    pub const ElectionWindowSize: u32 = 100;
    pub const ElectionAuthorityCount: u32 = 10;
    pub const ElectionRotationPeriod: BlockNumber = 600;     // 5h @30s
    pub const ElectionRotationDelay: BlockNumber = 10;       // 5min @30s
}
impl pallet_grandpa_authority_election::Config for Runtime {
    type WindowSize = ElectionWindowSize;
    type AuthorityCount = ElectionAuthorityCount;
    type RotationPeriod = ElectionRotationPeriod;
    type RotationDelay = ElectionRotationDelay;
    type AuthorOrigin = PowAuthorAdapter;
}

// Balances設定（ネイティブトークン用）
impl pallet_balances::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;  // 最小値1（0だとアカウント作成に問題）
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type FreezeIdentifier = ();
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type MaxFreezes = ConstU32<0>;
    type DoneSlashHandler = ();
}

// Transaction Payment設定
// 手数料は0（$moralの投稿コストでスパム対策）
parameter_types! {
    pub FeeMultiplier: Multiplier = Multiplier::one();
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = FungibleAdapter<Balances, ()>;
    type OperationalFeeMultiplier = ConstU8<5>;
    /// 手数料を0に設定（Weight -> 0）
    type WeightToFee = ConstantMultiplier<Balance, ConstU128<0>>;
    /// 手数料を0に設定（Length -> 0）
    type LengthToFee = ConstantMultiplier<Balance, ConstU128<0>>;
    type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
    type WeightInfo = ();
}

// Sudo設定
impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = ();
}

// Storage Stake (TSTS P4): Storage node の skin-in-the-game.
// 詳細: docs/economic_model_proposal.md §3.2.5
parameter_types! {
    pub const BondPerGB: Balance = 10_000_000_000_000;       // 10 MORAL/GB
    pub const MinDeclaredCapacity: u64 = 1_073_741_824;       // 1 GB
    pub const BondReleaseDelay: BlockNumber = 100_800;        // 7 days @ 30s
    pub SlashBurnSharePermill: Permill = Permill::from_percent(30);
}
impl pallet_storage_stake::Config for Runtime {
    type Currency = Balances;
    type BondPerGB = BondPerGB;
    type MinDeclaredCapacity = MinDeclaredCapacity;
    type BondReleaseDelay = BondReleaseDelay;
    type SlashBurnSharePermill = SlashBurnSharePermill;
}

// Base Fee (TSTS P2): EIP-1559 風動的手数料.
// 詳細: docs/economic_model_proposal.md §3.2.2
parameter_types! {
    pub const GasTargetBytesPerBlock: u32 = 50_000;            // 50 KB target
    pub const BaseFeeMin: u128 = 100;                           // 1e-10 MORAL/byte
    pub const BaseFeeMax: u128 = 100_000_000_000;                // 0.1 MORAL/byte cap
    pub const BaseFeeInit: u128 = 10_000;                        // 1e-8 MORAL/byte (genesis)
}
impl pallet_base_fee::Config for Runtime {
    type GasTargetBytesPerBlock = GasTargetBytesPerBlock;
    type BaseFeeMin = BaseFeeMin;
    type BaseFeeMax = BaseFeeMax;
    type BaseFeeInit = BaseFeeInit;
}

/// pallet_post / pallet_messaging 用の薄い adapter.
/// それぞれの pallet が独自の `BaseFeeProvider` trait を持つので、ここで両方を満たす。
pub struct BaseFeeAdapter;
impl pallet_post::pallet::BaseFeeProvider for BaseFeeAdapter {
    fn current_base_fee() -> u128 {
        <pallet_base_fee::Pallet<Runtime> as pallet_base_fee::BaseFeeProvider>::current_base_fee()
    }
    fn record_gas(bytes: u32) {
        <pallet_base_fee::Pallet<Runtime> as pallet_base_fee::BaseFeeProvider>::record_gas(bytes)
    }
}
impl pallet_messaging::BaseFeeProvider for BaseFeeAdapter {
    fn current_base_fee() -> u128 {
        <pallet_base_fee::Pallet<Runtime> as pallet_base_fee::BaseFeeProvider>::current_base_fee()
    }
    fn record_gas(bytes: u32) {
        <pallet_base_fee::Pallet<Runtime> as pallet_base_fee::BaseFeeProvider>::record_gas(bytes)
    }
}

// Post Pallet設定 (TSTS P2: BaseFee 適用)
impl pallet_post::Config for Runtime {
    type NativeToken = Balances;  // $moral = ネイティブトークン
    type Storage = Storage;  // Storage Pallet for atomic fragment registration (FR-401)
    type Reaction = Reaction;  // Reaction Pallet for reward pool deposits
    type MaxContentLength = ConstU32<1_073_741_824>; // 1GB (画像含むコンテンツ対応)
    /// 基本コスト: 100 MORAL
    type PostBaseCost = ConstU128<100_000_000_000_000>;
    /// バイト単価: 0.001 MORAL/byte
    type PostByteCost = ConstU128<1_000_000_000>;
    type Popularity = Popularity;
    /// TSTS P2: EIP-1559 base fee (混雑時のみ高くなる、平常時は ~0)
    type BaseFee = BaseFeeAdapter;
}

// Faucet Pallet設定 (TSTS P7: 累積発行上限を導入)
impl pallet_faucet::Config for Runtime {
    type NativeToken = Balances;  // $moral = ネイティブトークン
    /// 初期難易度: 18ビット（約3秒）
    type BaseDifficulty = ConstU8<18>;
    /// スケーリングファクター: 1000アカウントごとに+1ビット
    type DifficultyScalingFactor = ConstU64<1000>;
    /// 難易度上限: 28ビット（約3分）
    type MaxDifficulty = ConstU8<28>;
    /// 報酬量: 100 MORAL
    type RewardAmount = ConstU128<100_000_000_000_000>;
    /// チャレンジ有効期限: 100ブロック (BlockNumber = u32)
    type ChallengeValidity = ConstU32<100>;
    /// 累積発行上限: 100,000 MORAL (1000 claims) — bootstrap 専用 (TSTS sunset).
    /// この値到達後は `FaucetCapReached` で claim が失敗する。
    type TotalCap = ConstU128<100_000_000_000_000_000>;
}

// Storage Pallet設定
impl pallet_storage::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    /// ネイティブトークン: Balances (報酬ミント用)
    type NativeToken = Balances;
    /// 断片最大サイズ: 1GB
    type MaxFragmentSize = ConstU32<1_073_741_824>;
    /// PeerID最大長: 64バイト
    type MaxPeerIdLen = ConstU32<64>;
    /// 断片あたり最大保持者数: 100
    type MaxHoldersPerFragment = ConstU32<100>;
    /// ノードあたり最大断片数: 10,000
    type MaxFragmentsPerNode = ConstU32<10_000>;
    // === New security constants (FR-405-411) ===
    /// PeerID最小長: 38バイト
    type MinPeerIdLen = ConstU32<38>;
    /// ブロックあたり最大ノード登録数: 5
    type MaxRegistrationsPerBlock = ConstU32<5>;
    /// ブロック・ノードあたり最大宣言数: 10
    type MaxDeclarationsPerBlockPerNode = ConstU32<10>;
    /// ノード最小容量: 1GB
    type MinNodeCapacity = ConstU64<1_073_741_824>;
    /// PoW観測期間: 10ブロック
    type PowObservationPeriod = ConstU32<10>;
    /// 基本PoW難易度: 12ビット
    type BasePowDifficulty = ConstU8<12>;
    /// HTTP URL最大長: 256バイト
    type MaxHttpUrlLen = ConstU32<256>;
    /// バイトあたり基本報酬: 5_000_000_000 = 5e-3 MORAL/byte / 1000 = 5 nano-MORAL/byte.
    ///
    /// TSTS P3 で旧 1 unit (= 1e-12 MORAL/byte) から 5_000_000_000 に強化。
    /// シミュレーションで storage 支払総額が ~1,580× に増え、ノード参加インセンティブが立つ。
    /// プールが target 以下のときは線形 decay されるので過剰流出も起きない。
    type BaseRewardPerByte = ConstU128<5_000_000_000>;
    /// 報酬対象スコア閾値: 100
    type ScoreThreshold = ConstU64<100>;
    /// スコアヒステリシスマージン: 20 (回復には閾値+20必要)
    type ScoreHysteresisMargin = ConstU64<20>;
    /// ブロックあたりの最大チャレンジ発行数: 10 (スパム防止)
    type MaxChallengesPerBlock = ConstU32<10>;
    /// 報酬引き出し下限: 500 MORAL (500_000_000_000_000 with 12 decimals)
    type MinWithdrawalAmount = ConstU128<500_000_000_000_000>;
    /// Storage pool target balance: 500,000 MORAL.
    ///
    /// `calculate_reward_v2` でプール残高がこれ以下になると線形に payout を絞る。
    /// TSTS P3: σ_storage がターゲットを下回ったときに自動的に支払率が下がるため、
    /// プール枯渇前のソフトランディングが効く。
    type StoragePoolTarget = ConstU128<500_000_000_000_000_000>;
}

// Nickname Pallet設定
impl pallet_nickname::Config for Runtime {
    /// ニックネーム最大長: 128バイト
    type MaxNicknameLength = ConstU32<128>;
}

// Stealth Pallet設定
impl pallet_stealth::Config for Runtime {
    type Currency = Balances;
    /// ブロックあたり最大エフェメラルキー登録数: 100
    type MaxEntriesPerBlock = ConstU32<100>;
    type WeightInfo = pallet_stealth::weights::SubstrateWeight<Runtime>;
}

// Reaction Pallet設定
/// PostAuthorProvider implementation for reaction pallet
pub struct PostAuthorProviderImpl;
impl pallet_reaction::PostAuthorProvider<AccountId> for PostAuthorProviderImpl {
    fn get_post_author(post_id: u64) -> Option<AccountId> {
        pallet_post::Posts::<Runtime>::get(post_id).map(|post| post.author)
    }
}

// 1 MORAL = 1_000_000_000_000 (12 decimals)
//
// TSTS P5: Reaction Pallet を動的 γ + reactor decay + reactor lock に拡張。
// 詳細: docs/economic_model_proposal.md §3.2.6
parameter_types! {
    /// 旧固定報酬 (γ_max=0 時の fallback)。動的計算が有効なら未使用。
    pub const ReactionReward: Balance = 1_000_000_000_000;
}

/// TotalIssuance を pallet_balances から取得する adapter (TSTS P5).
///
/// pallet_reaction が pallet_balances に直接依存しないようにするための薄い橋渡し。
pub struct BalancesTotalIssuance;
impl pallet_reaction::pallet::TotalIssuanceProvider for BalancesTotalIssuance {
    fn total_issuance() -> u128 {
        use sp_runtime::traits::SaturatedConversion;
        let v = pallet_balances::Pallet::<Runtime>::total_issuance();
        v.saturated_into()
    }
}

impl pallet_reaction::Config for Runtime {
    /// Native token ($moral) for reward payouts
    type NativeToken = Balances;
    /// ReservableCurrency: reactor_lock の reserve / unreserve 用
    type ReservableCurrency = Balances;
    /// Provider for getting post authors
    type PostAuthorProvider = PostAuthorProviderImpl;
    /// Fixed reward fallback (γ_max=0 時のみ使用)
    type ReactionReward = ReactionReward;
    /// Base PoW difficulty: 16 leading zero bits
    type BaseDifficulty = ConstU8<16>;
    /// Minimum difficulty: 8 bits
    type MinDifficulty = ConstU8<8>;
    /// Maximum difficulty: 32 bits
    type MaxDifficulty = ConstU8<32>;
    /// Challenge validity: 100 blocks
    type ChallengeValidity = ConstU32<100>;
    /// Target reactions per block: 10
    type TargetReactionRate = ConstU32<10>;
    /// Adjustment window: 10 blocks
    type AdjustmentWindow = ConstU32<10>;
    /// Adjustment divisor: 4 (smooth changes)
    type AdjustmentDivisor = ConstU32<4>;
    type Popularity = Popularity;
    /// TSTS P5: TotalIssuance を pallet_balances から取得する adapter
    type TotalIssuanceProvider = BalancesTotalIssuance;
    /// TSTS P5: γ_max = 1% (10000 ppm). 0 にすると ReactionReward に縮退する
    type GammaMaxPpm = ConstU32<10_000>;
    /// TSTS P5: reactor decay K = 100 (100 反応で √2 倍薄まる)
    type ReactorDecayK = ConstU32<100>;
    /// TSTS P5: per-block payout cap = pool × 17 ppm (≒ 5%/day at 2880 blocks/day)
    /// 5% × 1e6 / 2880 ≒ 17 ppm. Sybil 大量攻撃でも 1 day で pool の 5% 以上は流出しない。
    type PerBlockPayoutCapPpm = ConstU32<17>;
    /// TSTS P5: reactor lock minimum = 0.1 MORAL = 1e11 units
    type ReactorLockMin = ConstU128<100_000_000_000>;
    /// TSTS P5: reactor lock duration = 24h = 2880 blocks @ 30s
    type ReactorLockDuration = ConstU32<2_880>;
}

// Popularity Pallet設定 — pallet-post / pallet-reaction の push 通知を受けて
// PostScores を維持し、人気度ベースのストレージ解放対象を識別する。
parameter_types! {
    /// 1 ブロックあたり 0.005% (= 999_950 / 1_000_000) 減衰。
    /// 6s/block ≒ 1 時間で約 3% 減 → 半減期 ~23 時間 (= ln(0.5)/ln(0.99995) × 6s)。
    pub PopularityDecayRate: sp_runtime::Permill = sp_runtime::Permill::from_parts(999_950);
}

impl pallet_popularity::Config for Runtime {
    type InitialScore = ConstU64<100_000>;
    type LikeWeight = ConstU64<100>;
    type DislikeWeight = ConstU64<50>;
    type DecayRatePermill = PopularityDecayRate;
    type LowPopularityThreshold = ConstU64<1_000>;
    type HysteresisMargin = ConstU64<500>;
    /// 7 days * 24 h * 600 blocks/h (6s/block) = 100_800
    type GracePeriod = ConstU32<100_800>;
    type MaxPostsScannedPerBlock = ConstU32<8>;
    type MaxDeletionsPerBlock = ConstU32<4>;
    /// 4x MaxDeletionsPerBlock — amortizes blake2-keyed scan over partially-eligible queues.
    type MaxDeletionScanReads = ConstU32<16>;
    type MaxDecaySteps = ConstU32<1_000_000>;
    type PostCountProvider = Post;
    type PostMutator = Post;
    type StorageReleaser = Storage;
}

// Messaging (DM) Pallet設定 — contracts/pallet-messaging-extrinsics.md §Dependencies
//
// TSTS P6: StealthReward 還流先を pallet_stealth に配線済み。20% が stealth pool に流入し、
// 受信エフェメラル公開鍵ごとの受信回数も記録される。詳細: docs/economic_model_proposal.md §3.2.4
parameter_types! {
    pub const DmBaseCost: Balance = 1_000_000_000_000;      // 1 MORAL
    pub const DmByteCost: Balance = 50_000_000_000;         // 0.05 MORAL / byte
    pub const MaxDmCiphertextLen: u64 = 262_144;
}

/// pallet_messaging::StealthRewardInterface を pallet_stealth::Pallet に橋渡しする adapter。
///
/// 直接 `impl<T: pallet_stealth::Config> StealthRewardInterface for pallet_stealth::Pallet<T>`
/// を書くと pallet_stealth crate が pallet_messaging に依存することになり循環するため、
/// runtime crate 側でこの薄い adapter を実装する。
pub struct StealthRewardAdapter;
impl pallet_messaging::StealthRewardInterface for StealthRewardAdapter {
    fn do_deposit_to_stealth_reward_pool(amount: u128) {
        pallet_stealth::Pallet::<Runtime>::deposit_to_reward_pool(amount);
    }
    fn record_recipient_receive(ephemeral_pubkey: [u8; 32]) {
        pallet_stealth::Pallet::<Runtime>::record_recipient_receive(ephemeral_pubkey);
    }
}

impl pallet_messaging::Config for Runtime {
    type NativeToken = Balances;
    type Storage = Storage;
    type StealthReward = StealthRewardAdapter;
    type MaxDispatchesPerBlock = ConstU32<256>;
    type DmBaseCost = DmBaseCost;
    type DmByteCost = DmByteCost;
    type MaxDmCiphertextLen = MaxDmCiphertextLen;
    type WeightInfo = pallet_messaging::weights::SubstrateWeight<Runtime>;
    /// TSTS P2: EIP-1559 base fee
    type BaseFee = BaseFeeAdapter;
}

// Runtime構築
construct_runtime!(
    pub struct Runtime {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Grandpa: pallet_grandpa,
        Difficulty: pallet_difficulty,
        BlockReward: pallet_block_reward,
        GrandpaElection: pallet_grandpa_authority_election,
        Balances: pallet_balances,
        TransactionPayment: pallet_transaction_payment,
        Sudo: pallet_sudo,
        // カスタムパレット (Storage must be before Post for tight coupling)
        BaseFee: pallet_base_fee,
        StorageStake: pallet_storage_stake,
        Storage: pallet_storage,
        Post: pallet_post,
        Faucet: pallet_faucet,
        Nickname: pallet_nickname,
        Stealth: pallet_stealth,
        Reaction: pallet_reaction,
        Messaging: pallet_messaging,
        Popularity: pallet_popularity,
    }
);

/// Block Header型
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block型
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// Unchecked Extrinsic型
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<sp_runtime::MultiAddress<AccountId, ()>, RuntimeCall, Signature, SignedExtra>;

/// Executive型
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
>;

/// Signed extras
pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    // ChargeTransactionPayment を削除 - TX手数料は完全無料
    // スパム対策は $moral の投稿コストで実施
);

// Runtime APIs実装
impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: LazyBlock) {
            Executive::execute_block(block);
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> sp_std::vec::Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: LazyBlock,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> GrandpaAuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> sp_consensus_grandpa::SetId {
            Grandpa::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            _equivocation_proof: sp_consensus_grandpa::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            _key_owner_proof: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            None
        }

        fn generate_key_ownership_proof(
            _set_id: sp_consensus_grandpa::SetId,
            _authority_id: GrandpaId,
        ) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
            None
        }
    }

    impl pallet_difficulty::DifficultyApi<Block> for Runtime {
        fn difficulty() -> sp_core::U256 {
            Difficulty::current_difficulty()
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            frame_support::genesis_builder_helper::build_state::<RuntimeGenesisConfig>(config)
        }

        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            frame_support::genesis_builder_helper::get_preset::<RuntimeGenesisConfig>(id, |_| None)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            vec![]
        }
    }

    impl pallet_post::PostApi<Block> for Runtime {
        fn get_content_by_merkle_root(merkle_root: [u8; 32]) -> Option<pallet_post::PostContentInfo> {
            let post_id = pallet_post::MerkleRootToPostId::<Runtime>::get(merkle_root)?;
            let content = pallet_post::ContentRefs::<Runtime>::get(post_id)?;
            Some(pallet_post::PostContentInfo {
                root: content.root,
                k: content.k,
                n: content.n,
                size: content.size,
            })
        }

        fn get_content_by_post_id(post_id: u64) -> Option<pallet_post::PostContentInfo> {
            let content = pallet_post::ContentRefs::<Runtime>::get(post_id)?;
            Some(pallet_post::PostContentInfo {
                root: content.root,
                k: content.k,
                n: content.n,
                size: content.size,
            })
        }
    }

    impl pallet_popularity::PopularityApi<Block> for Runtime {
        fn get_effective_score(post_id: u64) -> Option<u64> {
            let p = pallet_popularity::pallet::PostScores::<Runtime>::get(post_id)?;
            Some(pallet_popularity::pallet::Pallet::<Runtime>::effective_score_now_public(&p))
        }

        fn get_net_count(post_id: u64) -> Option<i64> {
            let p = pallet_popularity::pallet::PostScores::<Runtime>::get(post_id)?;
            Some(p.like_count as i64 - p.dislike_count as i64)
        }

        fn get_post_popularity(post_id: u64) -> Option<pallet_popularity::PostPopularityRpc> {
            let p = pallet_popularity::pallet::PostScores::<Runtime>::get(post_id)?;
            let eff = pallet_popularity::pallet::Pallet::<Runtime>::effective_score_now_public(&p);
            Some(pallet_popularity::PostPopularityRpc {
                effective_score: eff,
                like_count: p.like_count,
                dislike_count: p.dislike_count,
                net_count: p.like_count as i64 - p.dislike_count as i64,
                marked_for_deletion_at: p.marked_for_deletion_at,
                last_touched: p.last_touched,
            })
        }
    }

    impl pallet_messaging::DmScanApi<Block, AccountId> for Runtime {
        fn dispatches_at(block_number: u32) -> Vec<pallet_messaging::DmDispatch<AccountId>> {
            let bn: BlockNumber = block_number.into();
            pallet_messaging::DmDispatchesByBlock::<Runtime>::get(bn).into_inner()
        }

        fn reception_key(account: AccountId) -> Option<pallet_messaging::DmMetaAddress> {
            pallet_messaging::DmReceptionKeys::<Runtime>::get(&account)
        }

        fn dispatches_range(
            from_block: u32,
            to_block: u32,
        ) -> Vec<(u32, Vec<pallet_messaging::DmDispatch<AccountId>>)> {
            // 過剰スキャン防止: 1024 ブロック超の範囲は空配列を返す。
            if to_block < from_block || to_block - from_block > 1_024 {
                return Vec::new();
            }
            // 空ブロックは除外する (RPC payload 削減 / scanner-side semantics と一致)。
            // 参照: pallets/messaging/src/tests/runtime_api.rs `dispatches_range_within_limit_returns_entries_per_block`
            (from_block..=to_block)
                .filter_map(|bn| {
                    let block_no: BlockNumber = bn.into();
                    let entries = pallet_messaging::DmDispatchesByBlock::<Runtime>::get(block_no)
                        .into_inner();
                    if entries.is_empty() {
                        None
                    } else {
                        Some((bn, entries))
                    }
                })
                .collect()
        }
    }

    impl pallet_storage::StorageApi<Block> for Runtime {
        fn get_all_storage_nodes() -> Vec<pallet_storage::StorageNodeInfoRpc> {
            use sp_runtime::SaturatedConversion;
            pallet_storage::StorageNodes::<Runtime>::iter()
                .map(|(peer_id, info)| {
                    // Convert AccountId to [u8; 32]
                    let operator_bytes: [u8; 32] = info.operator.into();
                    pallet_storage::StorageNodeInfoRpc {
                        operator: operator_bytes,
                        capacity: info.capacity,
                        registered_at: info.registered_at.saturated_into::<u32>(),
                        pow_nonce: info.pow_nonce,
                        http_url: info.http_url.into_inner(),
                        peer_id: peer_id.into_inner(),
                    }
                })
                .collect()
        }

        fn get_kzg_fragment(content_hash: pallet_storage::ContentHash) -> Option<pallet_storage::KzgFragmentInfoRpc> {
            use sp_runtime::SaturatedConversion;
            pallet_storage::KzgFragments::<Runtime>::get(content_hash).map(|fragment| {
                let owner_bytes: [u8; 32] = fragment.owner.into();
                pallet_storage::KzgFragmentInfoRpc {
                    owner: owner_bytes,
                    commitment: fragment.commitment.into_inner(),
                    data_size: fragment.data_size,
                    fragment_count: fragment.fragment_count,
                    threshold: fragment.threshold,
                    created_at: fragment.created_at.saturated_into::<u32>(),
                }
            })
        }

        fn get_reward_pool_balance() -> u128 {
            pallet_storage::RewardPoolBalance::<Runtime>::get()
        }
        
        fn get_forgetting_candidates(content_hashes: Vec<pallet_storage::ContentHash>) -> Vec<(pallet_storage::ContentHash, bool)> {
            content_hashes
                .into_iter()
                .map(|hash| {
                    let is_candidate = pallet_storage::ForgettingCandidates::<Runtime>::contains_key(&hash);
                    (hash, is_candidate)
                })
                .collect()
        }
        
        fn is_registered_storage_node(operator: [u8; 32]) -> bool {
            // Convert operator bytes to AccountId
            let account_id: AccountId = operator.into();

            // (#27-HIGH-3) Only verify that the operator HAS a registered node.
            // The previous URL match leaked the stored URL via brute-force probing.
            pallet_storage::OperatorNodes::<Runtime>::get(&account_id)
                .and_then(|peer_id| pallet_storage::StorageNodes::<Runtime>::get(&peer_id))
                .is_some()
        }
        
        // ============ Self-Repair APIs (013-slashing-repair T027-T028) ============
        
        fn get_at_risk_fragments() -> Vec<pallet_storage::ContentHash> {
            pallet_storage::FragmentStates::<Runtime>::iter()
                .filter(|(_, state)| state.kind == pallet_storage::pallet::FragmentStateKind::AtRisk)
                .map(|(content_hash, _)| content_hash)
                .collect()
        }
        
        fn get_fragment_state(content_hash: pallet_storage::ContentHash) -> Option<pallet_storage::FragmentStateRpc> {
            use sp_runtime::SaturatedConversion;
            
            // Check if fragment exists
            if !pallet_storage::KzgFragments::<Runtime>::contains_key(content_hash) {
                return None;
            }
            
            let state = pallet_storage::FragmentStates::<Runtime>::get(content_hash);
            let kind_u8 = match state.kind {
                pallet_storage::pallet::FragmentStateKind::Active => 0u8,
                pallet_storage::pallet::FragmentStateKind::AtRisk => 1u8,
                pallet_storage::pallet::FragmentStateKind::Repairing => 2u8,
                pallet_storage::pallet::FragmentStateKind::Lost => 3u8,
            };
            
            Some(pallet_storage::FragmentStateRpc {
                kind: kind_u8,
                changed_at: state.changed_at.saturated_into::<u32>(),
            })
        }
        
        // T057: Get eviction candidates for a fragment
        fn get_eviction_candidates(content_hash: pallet_storage::ContentHash) -> Vec<pallet_storage::EvictionCandidateRpc> {
            use sp_runtime::SaturatedConversion;
            
            let candidates = Storage::compute_eviction_candidates(content_hash);
            candidates.into_iter().map(|c| {
                // Convert AccountId32 to [u8; 32]
                let account_bytes: [u8; 32] = c.account_id.into();
                
                pallet_storage::EvictionCandidateRpc {
                    account_id: account_bytes,
                    priority_score: c.priority_score,
                    share_index: c.share_index,
                    is_slashed: c.is_slashed,
                    last_proved_at: c.last_proved_at.saturated_into::<u32>(),
                }
            }).collect()
        }
        
        // T058: Get fragments with excess holders
        fn get_fragments_with_excess_holders() -> Vec<pallet_storage::ContentHash> {
            pallet_storage::KzgFragments::<Runtime>::iter()
                .filter(|(_, fragment)| {
                    fragment.holders.len() as u8 > fragment.fragment_count
                })
                .map(|(content_hash, _)| content_hash)
                .collect()
        }
    }
}

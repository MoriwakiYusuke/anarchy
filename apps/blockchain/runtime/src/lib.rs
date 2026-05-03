//! Anarchy Runtime
//!
//! 匿名分散型SNSのランタイム実装

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use sp_api::impl_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
    generic, impl_opaque_keys, Perbill,
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
    traits::{ConstBool, ConstU128, ConstU16, ConstU32, ConstU64, ConstU8},
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
            pub aura: Aura,
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
    spec_version: 104,  // Bumped: $moral = native token, removed pallet_moral
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 2,  // SignedExtra structure changed (no tip)
    system_version: 1,
};

/// Block時間（ミリ秒）
pub const MILLISECS_PER_BLOCK: u64 = 6000;

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

// Aura設定
impl pallet_aura::Config for Runtime {
    type AuthorityId = AuraId;
    type DisabledValidators = ();
    type MaxAuthorities = ConstU32<32>;
    type AllowMultipleBlocksPerSlot = ConstBool<false>;
    type SlotDuration = pallet_aura::MinimumPeriodTimesTwo<Runtime>;
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
    type OnTimestampSet = Aura;
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = ();
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

// Post Pallet設定
impl pallet_post::Config for Runtime {
    type NativeToken = Balances;  // $moral = ネイティブトークン
    type Storage = Storage;  // Storage Pallet for atomic fragment registration (FR-401)
    type Reaction = Reaction;  // Reaction Pallet for reward pool deposits
    type MaxContentLength = ConstU32<1_073_741_824>; // 1GB (画像含むコンテンツ対応)
    /// 基本コスト: 100 MORAL
    type PostBaseCost = ConstU128<100_000_000_000_000>;
    /// バイト単価: 0.001 MORAL/byte
    type PostByteCost = ConstU128<1_000_000_000>;
}

// Faucet Pallet設定
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
    /// バイトあたり基本報酬: 1 unit (12 decimals = 1e-12 MORAL/byte).
    /// NOTE: This is a very conservative default intended primarily for testing.
    ///       Production deployments should adjust this value based on network
    ///       economics (token price, desired incentives) and storage costs.
    type BaseRewardPerByte = ConstU128<1>;
    /// 報酬対象スコア閾値: 100
    type ScoreThreshold = ConstU64<100>;
    /// スコアヒステリシスマージン: 20 (回復には閾値+20必要)
    type ScoreHysteresisMargin = ConstU64<20>;
    /// ブロックあたりの最大チャレンジ発行数: 10 (スパム防止)
    type MaxChallengesPerBlock = ConstU32<10>;
    /// 報酬引き出し下限: 500 MORAL (500_000_000_000_000 with 12 decimals)
    type MinWithdrawalAmount = ConstU128<500_000_000_000_000>;
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
parameter_types! {
    pub const ReactionReward: Balance = 1_000_000_000_000;
}

impl pallet_reaction::Config for Runtime {
    /// Native token ($moral) for reward payouts
    type NativeToken = Balances;
    /// Provider for getting post authors
    type PostAuthorProvider = PostAuthorProviderImpl;
    /// Fixed reward: 1 MORAL per reaction
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
}

// Messaging (DM) Pallet設定 — contracts/pallet-messaging-extrinsics.md §Dependencies
// StealthReward 還流先は pallet-stealth に reward pool trait が追加されるまで no-op (())。
// 10% の還流分は暫定的に burn と同等の扱いになる (追加 burn としてドキュメント化)。
parameter_types! {
    pub const DmBaseCost: Balance = 1_000_000_000_000;      // 1 MORAL
    pub const DmByteCost: Balance = 50_000_000_000;         // 0.05 MORAL / byte
    pub const MaxDmCiphertextLen: u64 = 262_144;
}

impl pallet_messaging::Config for Runtime {
    type NativeToken = Balances;
    type Storage = Storage;
    type StealthReward = ();
    type MaxDispatchesPerBlock = ConstU32<256>;
    type DmBaseCost = DmBaseCost;
    type DmByteCost = DmByteCost;
    type MaxDmCiphertextLen = MaxDmCiphertextLen;
    type WeightInfo = pallet_messaging::weights::SubstrateWeight<Runtime>;
}

// Runtime構築
construct_runtime!(
    pub struct Runtime {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Aura: pallet_aura,
        Grandpa: pallet_grandpa,
        Balances: pallet_balances,
        TransactionPayment: pallet_transaction_payment,
        Sudo: pallet_sudo,
        // カスタムパレット (Storage must be before Post for tight coupling)
        Storage: pallet_storage,
        Post: pallet_post,
        Faucet: pallet_faucet,
        Nickname: pallet_nickname,
        Stealth: pallet_stealth,
        Reaction: pallet_reaction,
        Messaging: pallet_messaging,
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

    impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
        fn slot_duration() -> sp_consensus_aura::SlotDuration {
            sp_consensus_aura::SlotDuration::from_millis(SLOT_DURATION)
        }

        fn authorities() -> Vec<AuraId> {
            pallet_aura::Authorities::<Runtime>::get().into_inner()
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
        
        fn is_registered_storage_node(operator: [u8; 32], http_url: Vec<u8>) -> bool {
            // Convert operator bytes to AccountId
            let account_id: AccountId = operator.into();
            
            // Check if operator has a registered node
            if let Some(peer_id) = pallet_storage::OperatorNodes::<Runtime>::get(&account_id) {
                // Get the node info
                if let Some(node_info) = pallet_storage::StorageNodes::<Runtime>::get(&peer_id) {
                    // Verify the http_url matches
                    return node_info.http_url.to_vec() == http_url;
                }
            }
            
            false
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

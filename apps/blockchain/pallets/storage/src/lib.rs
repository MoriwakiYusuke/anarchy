//! # Storage Pallet
//!
//! Distributed storage for fragment metadata and node registration.
//!
//! ## Overview
//!
//! This pallet provides:
//! - Fragment metadata registration
//! - Storage node management
//! - Holding declaration/revocation
//!
//! ## Integration
//!
//! Other pallets can register fragments atomically by implementing tight coupling
//! via the `StorageInterface` trait (FR-401, FR-402).

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod kzg;
pub mod pow;
pub mod rate_limit;
pub mod rewards;

/// Fragment ID type (Blake2-256 hash)
pub type FragmentId = [u8; 32];

/// Content hash type for KZG-VSS (H256)
pub type ContentHash = [u8; 32];

use alloc::vec::Vec;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// Storage node info for Runtime API (simplified, without generic types)
#[derive(Clone, Encode, Decode, TypeInfo, Debug, PartialEq, Eq)]
pub struct StorageNodeInfoRpc {
    /// Node operator account (32 bytes)
    pub operator: [u8; 32],
    /// Available capacity in bytes
    pub capacity: u64,
    /// Block number when registered
    pub registered_at: u32,
    /// PoW nonce used for registration
    pub pow_nonce: u64,
    /// HTTP endpoint URL for fragment storage
    pub http_url: Vec<u8>,
    /// Peer ID (libp2p)
    pub peer_id: Vec<u8>,
}

/// KZG fragment info for Runtime API (FR-115)
#[derive(Clone, Encode, Decode, TypeInfo, Debug, PartialEq, Eq)]
pub struct KzgFragmentInfoRpc {
    /// Owner account (32 bytes)
    pub owner: [u8; 32],
    /// KZG commitment (48 bytes, compressed G1)
    pub commitment: Vec<u8>,
    /// Data size in bytes
    pub data_size: u32,
    /// Number of shares (n)
    pub fragment_count: u8,
    /// Recovery threshold (k)
    pub threshold: u8,
    /// Block number when created
    pub created_at: u32,
}

/// Fragment state for Runtime API (013-slashing-repair T026)
#[derive(Clone, Encode, Decode, TypeInfo, Debug, PartialEq, Eq)]
pub struct FragmentStateRpc {
    /// Current state kind
    /// 0 = Active, 1 = AtRisk, 2 = Repairing, 3 = Lost
    pub kind: u8,
    /// Block number when state last changed
    pub changed_at: u32,
}

/// Eviction candidate for Runtime API (013-slashing-repair T057)
#[derive(Clone, Encode, Decode, TypeInfo, Debug, PartialEq, Eq)]
pub struct EvictionCandidateRpc {
    /// Account ID of the holder (32 bytes)
    pub account_id: [u8; 32],
    /// Priority score (lower = higher eviction priority)
    pub priority_score: u32,
    /// Share index held
    pub share_index: u8,
    /// Whether the node is slashed
    pub is_slashed: bool,
    /// Block when last proved
    pub last_proved_at: u32,
}

sp_api::decl_runtime_apis! {
    /// Storage Pallet Runtime API
    ///
    /// Provides access to on-chain storage node information for chain node RPC
    pub trait StorageApi {
        /// Get all registered storage nodes with their HTTP URLs
        fn get_all_storage_nodes() -> Vec<StorageNodeInfoRpc>;

        /// Get KZG fragment info by content hash (FR-115)
        /// Returns None if content_hash is not registered
        fn get_kzg_fragment(content_hash: ContentHash) -> Option<KzgFragmentInfoRpc>;
        
        /// Get current reward pool balance (for storage node GC decisions)
        fn get_reward_pool_balance() -> u128;
        
        /// Check which content hashes are forgetting candidates (GC-ready)
        /// Returns a list of (content_hash, is_forgetting_candidate) pairs
        fn get_forgetting_candidates(content_hashes: Vec<ContentHash>) -> Vec<(ContentHash, bool)>;
        
        /// Verify a storage node registration (PR #22 CRITICAL-3).
        ///
        /// (#27-HIGH-3) Previously this also took the candidate `http_url` and
        /// compared it on-chain, which let any RPC caller brute-force the
        /// stored URL of an operator (privacy oracle). The URL parameter has
        /// been removed; verification of operator identity is sufficient and
        /// the URL itself stays a server-side concern.
        fn is_registered_storage_node(operator: [u8; 32]) -> bool;
        
        // ============ Self-Repair APIs (013-slashing-repair T026-T028) ============
        
        /// Get all fragments in AtRisk state (T027)
        /// Returns list of content hashes that need repair
        fn get_at_risk_fragments() -> Vec<ContentHash>;
        
        /// Get fragment state (T028)
        /// Returns the current state (Active/AtRisk/Repairing/Lost) and when it changed
        fn get_fragment_state(content_hash: ContentHash) -> Option<FragmentStateRpc>;
        
        /// Get eviction candidates for a fragment (T057)
        /// Returns holders sorted by eviction priority (lowest priority first)
        fn get_eviction_candidates(content_hash: ContentHash) -> Vec<EvictionCandidateRpc>;
        
        /// Get all fragments with excess holders (T058)
        /// Returns content hashes of fragments that have more holders than fragment_count
        fn get_fragments_with_excess_holders() -> Vec<ContentHash>;
    }
}

use frame_support::dispatch::DispatchResult;
use frame_system::pallet_prelude::BlockNumberFor;

/// Interface for cross-pallet fragment registration (FR-401).
///
/// This trait enables tight coupling between Post Pallet and Storage Pallet,
/// allowing atomic fragment registration within create_post_v2 transaction.
pub trait StorageInterface<AccountId, BlockNumber> {
    /// Register a fragment internally without requiring a signed origin.
    ///
    /// This is called by Post Pallet during create_post_v2 to atomically
    /// register the fragment metadata on-chain.
    ///
    /// # Arguments
    /// * `fragment_id` - Blake2-256 hash of the fragment
    /// * `size` - Size of the fragment in bytes
    /// * `creator` - Account ID of the creator
    /// * `created_at` - Block number when created
    fn do_register_fragment(
        fragment_id: FragmentId,
        size: u32,
        creator: AccountId,
        created_at: BlockNumber,
    ) -> DispatchResult;

    /// Register a KZG fragment internally (Issue 4 fix).
    ///
    /// Called by Post Pallet during create_post_v2 to atomically
    /// register KZG commitment on-chain for proof-of-holding.
    ///
    /// # Arguments
    /// * `owner` - The owner account (verified by calling pallet)
    /// * `content_hash` - Blake2-256 hash of the content
    /// * `commitment` - KZG commitment (48 bytes)
    /// * `data_size` - Original data size in bytes
    /// * `fragment_count` - Total number of shares (n)
    /// * `threshold` - Recovery threshold (k)
    fn do_register_kzg_fragment(
        owner: AccountId,
        content_hash: ContentHash,
        commitment: sp_std::vec::Vec<u8>,
        data_size: u32,
        fragment_count: u8,
        threshold: u8,
    ) -> DispatchResult;

    /// Deposit tokens to the reward pool (FR-113).
    ///
    /// Called by Post Pallet during create_post_v2 to deposit 90% of post fee
    /// to the reward pool. The amount represents "mint rights" that will be
    /// distributed to storage nodes via claim_reward.
    ///
    /// # Arguments
    /// * `amount` - Amount of tokens (in u128) to add to reward pool
    fn do_deposit_to_reward_pool(amount: u128);

    /// Release storage state for a content hash that was deleted under low-popularity policy.
    ///
    /// Called by the Popularity Pallet when a post is removed because its score fell below
    /// the retention threshold. Removes on-chain fragment metadata (`Fragments`/`KzgFragments`),
    /// proof records (`ProofRecords`), forgetting flags (`ForgettingCandidates`) and fragment
    /// state (`FragmentStates`), and emits `ForgottenByPolicy` so off-chain storage nodes can
    /// physically delete the data.
    ///
    /// Idempotent — silent no-op (still returns `Ok`) when the fragment is unknown.
    fn do_release_fragment(content_hash: ContentHash) -> DispatchResult;
}

#[frame_support::pallet]
#[allow(deprecated)]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::sp_runtime::Saturating;
    use frame_support::traits::fungible::{Inspect, Mutate};
    use frame_system::pallet_prelude::*;

    /// Balance type from NativeToken (T084)
    pub type BalanceOf<T> = <<T as Config>::NativeToken as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

    /// Fragment metadata stored on-chain
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
    #[scale_info(skip_type_params(T))]
    pub struct FragmentMetadata<T: Config> {
        /// Size in bytes
        pub size: u32,
        /// Creator account
        pub creator: T::AccountId,
        /// Block number when registered
        pub created_at: BlockNumberFor<T>,
    }

    /// Storage node information
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
    #[scale_info(skip_type_params(T))]
    pub struct StorageNodeInfo<T: Config> {
        /// Node operator account
        pub operator: T::AccountId,
        /// Available capacity in bytes
        pub capacity: u64,
        /// Block number when registered
        pub registered_at: BlockNumberFor<T>,
        /// PoW nonce used for registration (FR-409)
        pub pow_nonce: u64,
        /// HTTP endpoint URL for fragment storage (e.g., "http://127.0.0.1:3030")
        pub http_url: BoundedVec<u8, T::MaxHttpUrlLen>,
    }

    // ============ KZG-VSS Types (011-kzg-proof-rewards) ============

    /// KZG Fragment metadata for proof-of-holding (FR-102)
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
    #[scale_info(skip_type_params(T))]
    pub struct KzgFragment<T: Config> {
        /// Owner (post creator)
        pub owner: T::AccountId,
        /// KZG commitment (compressed G1 point, 48 bytes)
        pub commitment: BoundedVec<u8, ConstU32<48>>,
        /// Data size in bytes
        pub data_size: u32,
        /// Number of shares (n)
        pub fragment_count: u8,
        /// Recovery threshold (k)
        pub threshold: u8,
        /// Block when created
        pub created_at: BlockNumberFor<T>,
        /// Active share holders
        pub holders: BoundedVec<T::AccountId, ConstU32<16>>,
    }

    /// Challenge issued for proof-of-holding (FR-103)
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
    #[scale_info(skip_type_params(T))]
    pub struct Challenge<T: Config> {
        /// Target content hash
        pub content_hash: super::ContentHash,
        /// Target share index (1..n)
        pub share_index: u8,
        /// Challenged node
        pub challenged_node: T::AccountId,
        /// Block when issued
        pub issued_at: BlockNumberFor<T>,
        /// Deadline block
        pub deadline: BlockNumberFor<T>,
    }

    /// Proof record for tracking holding proofs (FR-104, FR-109)
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, Default)]
    pub struct ProofRecord<BlockNumber: Default> {
        /// Last successful proof block
        pub last_proved_at: BlockNumber,
        /// Consecutive success count
        pub success_count: u32,
        /// Consecutive failure count
        pub failure_count: u32,
        /// Slashed flag (013-slashing-repair) - affects GC priority
        pub slashed: bool,
        /// Share index held by this node (1-255, 0=unset) (013-slashing-repair)
        pub share_index: u8,
        // Note: pending_reward field removed (Issue 3 fix)
        // Rewards are now tracked ONLY in PendingRewards storage to prevent double-counting
    }

    /// Eviction candidate info for GC prioritization (013-slashing-repair T016)
    #[derive(Clone, RuntimeDebug, PartialEq, Eq, Encode, Decode, TypeInfo)]
    pub struct EvictionCandidate<AccountId, BlockNumber> {
        pub account_id: AccountId,
        pub priority_score: u32,
        pub share_index: u8,
        pub is_slashed: bool,
        pub last_proved_at: BlockNumber,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ============ Genesis Config ============

    /// Genesis configuration for pallet-storage
    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// Initial reward pool balance (for dev/testnet)
        pub initial_reward_pool: u128,
        #[serde(skip)]
        pub _phantom: core::marker::PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            RewardPoolBalance::<T>::put(self.initial_reward_pool);
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Clear per-block rate limiting counters and process expired challenges (FR-406, FR-410, Issue 2).
        fn on_finalize(block: BlockNumberFor<T>) {
            // Clear registration count for this block
            RegistrationCountPerBlock::<T>::remove(block);
            
            // Clear declaration counts for this block
            // Note: We clear by block number, not iterating over nodes
            let _ = DeclareHoldingCountPerBlock::<T>::clear_prefix(block, u32::MAX, None);

            // Clear challenge counts for this block (rate limiting)
            let _ = ChallengeCountPerBlock::<T>::clear_prefix(block, u32::MAX, None);

            // Process expired challenges (Issue 2, T011)
            let expired_challenges = ChallengesByDeadline::<T>::take(block);
            for (content_hash, share_index) in expired_challenges.into_iter() {
                if let Some(challenge) = PendingChallenges::<T>::take(content_hash, share_index) {
                    // Increment failure count for the challenged node
                    ProofRecords::<T>::mutate(content_hash, &challenge.challenged_node, |record| {
                        record.failure_count = record.failure_count.saturating_add(1);
                    });

                    // Emit ChallengeExpired event
                    Self::deposit_event(Event::ChallengeExpired {
                        content_hash,
                        share_index,
                        challenged_node: challenge.challenged_node.clone(),
                    });

                    // T047: Check if node should be slashed (3 consecutive failures)
                    let record = ProofRecords::<T>::get(content_hash, &challenge.challenged_node);
                    if record.failure_count >= 3 && !record.slashed {
                        // Slash the node - ignore errors (node may have already been slashed)
                        let _ = Self::do_slash_node(challenge.challenged_node, content_hash);
                    }
                }
            }
        }
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Maximum fragment size in bytes (default: 1MB)
        #[pallet::constant]
        type MaxFragmentSize: Get<u32>;

        /// Maximum PeerID length in bytes (default: 64)
        #[pallet::constant]
        type MaxPeerIdLen: Get<u32>;

        /// Maximum holders per fragment (default: 100)
        #[pallet::constant]
        type MaxHoldersPerFragment: Get<u32>;

        /// Maximum fragments per node (default: 10,000)
        #[pallet::constant]
        type MaxFragmentsPerNode: Get<u32>;

        // === New constants for security (FR-405-411) ===

        /// Minimum PeerID length (default: 38)
        #[pallet::constant]
        type MinPeerIdLen: Get<u32>;

        /// Maximum node registrations per block (default: 5)
        #[pallet::constant]
        type MaxRegistrationsPerBlock: Get<u32>;

        /// Maximum holding declarations per block per node (default: 10)
        #[pallet::constant]
        type MaxDeclarationsPerBlockPerNode: Get<u32>;

        /// Minimum node capacity in bytes (default: 1GB = 1_073_741_824)
        #[pallet::constant]
        type MinNodeCapacity: Get<u64>;

        /// PoW observation period in blocks (default: 10)
        #[pallet::constant]
        type PowObservationPeriod: Get<u32>;

        /// Base PoW difficulty (leading zero bits, default: 12)
        #[pallet::constant]
        type BasePowDifficulty: Get<u8>;

        /// Maximum HTTP URL length in bytes (default: 256)
        #[pallet::constant]
        type MaxHttpUrlLen: Get<u32>;

        /// Base reward per byte for holding proofs (T050, FR-109)
        /// Default: 1_000 (0.000001 MORAL per byte)
        #[pallet::constant]
        type BaseRewardPerByte: Get<u128>;

        /// Score threshold for reward eligibility (T059, FR-111)
        /// Content below this score receives 0 rewards
        /// Default: 100
        #[pallet::constant]
        type ScoreThreshold: Get<u64>;

        /// Score hysteresis margin (T072)
        /// Prevents rapid toggling between eligible/ineligible states
        /// Recovery requires score >= ScoreThreshold + ScoreHysteresisMargin 
        /// Default: 20 (20% of threshold=100)
        #[pallet::constant]
        type ScoreHysteresisMargin: Get<u64>;

        /// Maximum challenges per block per issuer (rate limiting)
        /// Prevents challenge spam attacks
        /// Default: 10
        #[pallet::constant]
        type MaxChallengesPerBlock: Get<u32>;

        /// Minimum withdrawal amount for claim_rewards (013-slashing-repair)
        /// Default: 500 MORAL (500_000_000_000_000 with 12 decimals)
        #[pallet::constant]
        type MinWithdrawalAmount: Get<u128>;

        /// Native token for reward distribution (T084)
        /// Used to mint rewards when claim_reward is called
        type NativeToken: Inspect<Self::AccountId> + Mutate<Self::AccountId>;
    }

    // ============ Storage ============

    /// Fragment metadata storage
    #[pallet::storage]
    #[pallet::getter(fn fragments)]
    pub type Fragments<T: Config> =
        StorageMap<_, Blake2_128Concat, FragmentId, FragmentMetadata<T>, OptionQuery>;

    /// Storage node information
    #[pallet::storage]
    #[pallet::getter(fn storage_nodes)]
    pub type StorageNodes<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, T::MaxPeerIdLen>,
        StorageNodeInfo<T>,
        OptionQuery,
    >;

    /// Operator to PeerID reverse lookup
    #[pallet::storage]
    #[pallet::getter(fn operator_nodes)]
    pub type OperatorNodes<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<u8, T::MaxPeerIdLen>, OptionQuery>;

    /// Fragment ID to holder nodes
    #[pallet::storage]
    #[pallet::getter(fn fragment_holders)]
    pub type FragmentHolders<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        FragmentId,
        BoundedVec<BoundedVec<u8, T::MaxPeerIdLen>, T::MaxHoldersPerFragment>,
        ValueQuery,
    >;

    /// Node PeerID to held fragments
    #[pallet::storage]
    #[pallet::getter(fn node_holdings)]
    pub type NodeHoldings<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, T::MaxPeerIdLen>,
        BoundedVec<FragmentId, T::MaxFragmentsPerNode>,
        ValueQuery,
    >;

    // === New storage for rate limiting (FR-406, FR-410) ===

    /// Registration count per block (for PoW dynamic difficulty)
    #[pallet::storage]
    #[pallet::getter(fn registration_count)]
    pub type RegistrationCountPerBlock<T: Config> =
        StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, u32, ValueQuery>;

    /// Declaration count per block per node (for rate limiting)
    #[pallet::storage]
    #[pallet::getter(fn declaration_count)]
    pub type DeclareHoldingCountPerBlock<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        Blake2_128Concat,
        BoundedVec<u8, T::MaxPeerIdLen>,
        u32,
        ValueQuery,
    >;

    // ============ KZG-VSS Storage (011-kzg-proof-rewards) ============

    /// KZG Fragment storage (content_hash -> KzgFragment)
    #[pallet::storage]
    #[pallet::getter(fn kzg_fragments)]
    pub type KzgFragments<T: Config> =
        StorageMap<_, Blake2_128Concat, super::ContentHash, KzgFragment<T>, OptionQuery>;

    /// Pending challenges (content_hash, share_index) -> Challenge
    #[pallet::storage]
    #[pallet::getter(fn pending_challenges)]
    pub type PendingChallenges<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        super::ContentHash,
        Blake2_128Concat,
        u8,  // share_index
        Challenge<T>,
        OptionQuery,
    >;

    /// Proof records (content_hash, holder) -> ProofRecord
    #[pallet::storage]
    #[pallet::getter(fn proof_records)]
    pub type ProofRecords<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        super::ContentHash,
        Blake2_128Concat,
        T::AccountId,
        ProofRecord<BlockNumberFor<T>>,
        ValueQuery,
    >;

    /// Reward pool balance (total accumulated from post fees)
    #[pallet::storage]
    #[pallet::getter(fn reward_pool_balance)]
    pub type RewardPoolBalance<T: Config> = StorageValue<_, u128, ValueQuery>;

    /// Score cache for external score provider
    #[pallet::storage]
    #[pallet::getter(fn score_cache)]
    pub type ScoreCache<T: Config> =
        StorageMap<_, Blake2_128Concat, super::ContentHash, u64, OptionQuery>;

    /// Pending rewards per account (accumulated from prove_holding_kzg)
    #[pallet::storage]
    #[pallet::getter(fn pending_rewards)]
    pub type PendingRewards<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u128, ValueQuery>;

    /// Forgetting candidates - content with score below threshold (FR-110)
    /// Stores the block number when content became a candidate
    #[pallet::storage]
    #[pallet::getter(fn forgetting_candidates)]
    pub type ForgettingCandidates<T: Config> =
        StorageMap<_, Blake2_128Concat, super::ContentHash, BlockNumberFor<T>, OptionQuery>;

    /// Challenge count per block per issuer (rate limiting)
    #[pallet::storage]
    #[pallet::getter(fn challenge_count_per_block)]
    pub type ChallengeCountPerBlock<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    /// Challenge deadline index (block_number -> list of (content_hash, share_index))
    /// Used for efficient expired challenge cleanup in on_finalize (Issue 2)
    #[pallet::storage]
    #[pallet::getter(fn challenges_by_deadline)]
    pub type ChallengesByDeadline<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        BoundedVec<(super::ContentHash, u8), ConstU32<1000>>,
        ValueQuery,
    >;

    // ============ Self-Repair Storage (013-slashing-repair) ============

    /// Fragment state kind (013-slashing-repair)
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, Default)]
    pub enum FragmentStateKind {
        /// Normal state - adequate holder count (>= 5)
        #[default]
        Active,
        /// At risk - holder count dropped (<= 4)
        AtRisk,
        /// Repair in progress
        Repairing,
        /// Lost - too few holders (<= 2) for recovery
        Lost,
    }

    /// Fragment state tracking (013-slashing-repair)
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
    #[scale_info(skip_type_params(T))]
    pub struct FragmentState<T: Config> {
        /// Current state kind
        pub kind: FragmentStateKind,
        /// Block when state changed
        pub changed_at: BlockNumberFor<T>,
    }

    impl<T: Config> Default for FragmentState<T> {
        fn default() -> Self {
            Self {
                kind: FragmentStateKind::Active,
                changed_at: BlockNumberFor::<T>::default(),
            }
        }
    }

    /// Fragment states (content_hash -> FragmentState) (013-slashing-repair)
    #[pallet::storage]
    #[pallet::getter(fn fragment_states)]
    pub type FragmentStates<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        super::ContentHash,
        FragmentState<T>,
        ValueQuery,
    >;

    /// Repair reward pools (content_hash -> Balance) (013-slashing-repair)
    /// Accumulated from slashing penalties, distributed to repair participants
    #[pallet::storage]
    #[pallet::getter(fn repair_reward_pools)]
    pub type RepairRewardPools<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        super::ContentHash,
        u128,
        ValueQuery,
    >;

    // ============ Events ============

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Fragment registered
        FragmentRegistered {
            fragment_id: FragmentId,
            creator: T::AccountId,
            size: u32,
        },

        /// Storage node registered
        NodeRegistered {
            peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
            operator: T::AccountId,
            capacity: u64,
        },

        /// Storage node updated
        NodeUpdated {
            peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
            new_capacity: u64,
        },

        /// Storage node unregistered
        NodeUnregistered {
            peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
            operator: T::AccountId,
        },

        /// Holding declared
        HoldingDeclared {
            peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
            fragment_id: FragmentId,
        },

        /// Holding revoked
        HoldingRevoked {
            peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
            fragment_id: FragmentId,
        },

        /// KZG fragment registered (FR-102)
        KzgFragmentRegistered {
            content_hash: super::ContentHash,
            owner: T::AccountId,
            commitment: BoundedVec<u8, ConstU32<48>>,
            data_size: u32,
            fragment_count: u8,
            threshold: u8,
        },

        /// KZG holding proof submitted successfully (FR-104)
        HoldingProved {
            content_hash: super::ContentHash,
            holder: T::AccountId,
            share_index: u8,
        },

        /// Challenge issued to a storage node (FR-103)
        ChallengeIssued {
            content_hash: super::ContentHash,
            share_index: u8,
            target_node: T::AccountId,
            deadline: BlockNumberFor<T>,
        },

        /// Reward claimed by storage node (FR-108)
        RewardClaimed {
            holder: T::AccountId,
            amount: u128,
        },

        /// Content marked as forgetting candidate (FR-110)
        ForgettingCandidateMarked {
            content_hash: super::ContentHash,
            marked_at: BlockNumberFor<T>,
        },

        /// Content released because the corresponding post was deleted under low-popularity policy.
        /// Storage nodes should observe this event and physically delete the underlying fragment data.
        ForgottenByPolicy {
            content_hash: super::ContentHash,
        },

        /// Content recovered from forgetting candidate (T054)
        ScoreRecovered {
            content_hash: super::ContentHash,
        },

        /// Challenge expired without proof submission (Issue 2)
        ChallengeExpired {
            content_hash: super::ContentHash,
            share_index: u8,
            challenged_node: T::AccountId,
        },

        // ============ Self-Repair Events (013-slashing-repair T018) ============

        /// Fragment entered AtRisk state (holder count <= 4)
        FragmentAtRisk {
            content_hash: super::ContentHash,
            holder_count: u32,
        },

        /// Fragment became Lost (holder count <= 2, recovery impossible)
        FragmentLost {
            content_hash: super::ContentHash,
            holder_count: u32,
        },

        /// Fragment repair completed successfully
        RepairCompleted {
            content_hash: super::ContentHash,
            new_holder: T::AccountId,
            new_share_index: u8,
        },

        /// Storage node slashed for repeated challenge failures
        NodeSlashed {
            content_hash: super::ContentHash,
            node: T::AccountId,
            penalty_amount: u128,
        },

        /// Stale holder evicted during GC
        HolderEvicted {
            content_hash: super::ContentHash,
            evicted_holder: T::AccountId,
        },
    }

    // ============ Errors ============

    #[pallet::error]
    pub enum Error<T> {
        /// Fragment ID already exists
        FragmentAlreadyExists,
        /// Fragment size exceeds maximum
        FragmentTooLarge,
        /// Fragment size is zero
        FragmentTooSmall,
        /// Fragment not found
        FragmentNotFound,

        /// Storage node already registered with this PeerID
        NodeAlreadyRegistered,
        /// Operator already has a registered node
        OperatorAlreadyHasNode,
        /// Storage node not registered
        NodeNotRegistered,
        /// Invalid PeerID format
        InvalidPeerId,
        /// Invalid capacity (zero)
        InvalidCapacity,
        /// Node has active holdings
        NodeHasHoldings,

        /// Already holding this fragment
        AlreadyHolding,
        /// Not holding this fragment
        NotHolding,
        /// Maximum holders reached for this fragment
        TooManyHolders,
        /// Maximum fragments reached for this node
        TooManyFragments,

        // === New errors for security (FR-405-411) ===

        /// PoW nonce does not meet current difficulty
        InvalidPow,
        /// Too many node registrations this block
        TooManyRegistrationsThisBlock,
        /// Too many holding declarations this block for this node
        TooManyDeclarationsThisBlock,
        /// Node capacity below minimum (1GB)
        CapacityTooSmall,
        /// PeerID too short (< MinPeerIdLen bytes)
        PeerIdTooShort,
        /// PeerID too long (> MaxPeerIdLen bytes)
        PeerIdTooLong,
        /// HTTP URL is empty or invalid
        InvalidHttpUrl,

        // === KZG-VSS errors (011-kzg-proof-rewards) ===

        /// KZG fragment already exists for this content hash
        KzgFragmentAlreadyExists,
        /// Invalid KZG commitment length (expected 48 bytes)
        InvalidCommitmentLength,
        /// Invalid threshold: must be 1 <= threshold <= fragment_count
        InvalidKzgThreshold,
        /// Fragment count too small (minimum 2)
        FragmentCountTooSmall,
        /// Commitment conversion failed
        InvalidCommitment,

        // === KZG Proof verification errors (US2) ===

        /// Invalid KZG proof (pairing check failed)
        InvalidKzgProof,
        /// Invalid share value length (expected 32 bytes)
        InvalidShareValue,
        /// Challenge not found
        ChallengeNotFound,
        /// Node not challenged (no active challenge)
        NotChallenged,
        /// Challenge already issued for this node/fragment
        ChallengeAlreadyIssued,
        /// Proof already submitted for this challenge
        ProofAlreadySubmitted,
        /// Invalid challenge index
        InvalidChallengeIndex,
        /// KZG fragment not found
        KzgFragmentNotFound,
        /// Prover is not a registered holder of this fragment
        NotHolder,

        // === Reward errors (US3) ===

        /// No pending reward to claim
        NoPendingReward,
        /// Reward pool has insufficient balance
        InsufficientRewardPool,
        /// Arithmetic overflow during reward calculation (T084)
        ArithmeticOverflow,
        /// Failed to mint reward tokens (T084)
        RewardMintFailed,
        /// Target node is not a holder of the challenged fragment
        NotHolderOfFragment,
        /// Challenge rate limit exceeded for this block
        ChallengeLimitExceeded,
        /// Challenge issuer is not a registered storage node (Issue 1)
        IssuerNotRegisteredNode,

        // === Self-Repair errors (013-slashing-repair T019) ===

        /// Accumulated rewards below minimum withdrawal amount (500 MORAL)
        InsufficientAccruedRewards,
        /// Fragment is not in AtRisk or Repairing state
        FragmentNotAtRisk,
        /// Fragment has too many holders already (> n)
        FragmentHasTooManyHolders,
        /// No excess holders to evict
        NoExcessHolders,
        /// Target account is not a holder of this fragment
        TargetNotHolder,
        /// Target is not the lowest priority holder
        TargetNotLowestPriority,
        /// Node is already slashed for this fragment
        AlreadySlashed,
    }

    // ============ Extrinsics ============

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a fragment on-chain.
        ///
        /// The fragment_id should be the Blake2-256 hash of the fragment data.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().writes(1))]
        pub fn register_fragment(
            origin: OriginFor<T>,
            fragment_id: FragmentId,
            size: u32,
        ) -> DispatchResult {
            let creator = ensure_signed(origin)?;

            // Validate size
            ensure!(size > 0, Error::<T>::FragmentTooSmall);
            ensure!(size <= T::MaxFragmentSize::get(), Error::<T>::FragmentTooLarge);

            // Check for duplicates
            ensure!(!Fragments::<T>::contains_key(fragment_id), Error::<T>::FragmentAlreadyExists);

            // Create metadata
            let metadata = FragmentMetadata {
                size,
                creator: creator.clone(),
                created_at: frame_system::Pallet::<T>::block_number(),
            };

            // Store
            Fragments::<T>::insert(fragment_id, metadata);

            // Emit event
            Self::deposit_event(Event::FragmentRegistered { fragment_id, creator, size });

            Ok(())
        }

        /// Register a storage node with PoW verification (FR-409, FR-410).
        ///
        /// Requires proof-of-work to prevent DoS attacks. The required difficulty
        /// is dynamic based on recent registration activity.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0) + T::DbWeight::get().reads_writes(4, 4))]
        pub fn register_node(
            origin: OriginFor<T>,
            peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
            capacity: u64,
            pow_nonce: u64,
            http_url: BoundedVec<u8, T::MaxHttpUrlLen>,
        ) -> DispatchResult {
            let operator = ensure_signed(origin)?;
            let current_block = frame_system::Pallet::<T>::block_number();

            // Check per-block registration rate limit (FR-410)
            let current_count = RegistrationCountPerBlock::<T>::get(current_block);
            ensure!(
                crate::rate_limit::can_register_node(current_count, T::MaxRegistrationsPerBlock::get()),
                Error::<T>::TooManyRegistrationsThisBlock
            );

            // Validate PeerID (FR-405)
            Self::validate_peer_id(&peer_id)?;

            // Validate capacity (FR-411)
            Self::validate_capacity(capacity)?;

            // Verify PoW (FR-409)
            let difficulty = Self::current_pow_difficulty();
            ensure!(
                crate::pow::verify_pow(&peer_id, pow_nonce, difficulty) == crate::pow::PowResult::Valid,
                Error::<T>::InvalidPow
            );

            // Check for duplicates
            ensure!(
                !StorageNodes::<T>::contains_key(&peer_id),
                Error::<T>::NodeAlreadyRegistered
            );
            ensure!(
                !OperatorNodes::<T>::contains_key(&operator),
                Error::<T>::OperatorAlreadyHasNode
            );

            // Validate HTTP URL (must not be empty)
            ensure!(!http_url.is_empty(), Error::<T>::InvalidHttpUrl);

            // Create node info
            let node_info = StorageNodeInfo {
                operator: operator.clone(),
                capacity,
                registered_at: current_block,
                pow_nonce,
                http_url: http_url.clone(),
            };

            // Store
            StorageNodes::<T>::insert(&peer_id, node_info);
            OperatorNodes::<T>::insert(&operator, peer_id.clone());

            // Increment registration counter for this block
            RegistrationCountPerBlock::<T>::insert(
                current_block,
                crate::rate_limit::increment_registration_count(current_count),
            );

            // Emit event
            Self::deposit_event(Event::NodeRegistered {
                peer_id,
                operator,
                capacity,
            });

            Ok(())
        }

        /// Update storage node capacity.
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(2, 1))]
        pub fn update_node(origin: OriginFor<T>, new_capacity: u64) -> DispatchResult {
            let operator = ensure_signed(origin)?;

            // Validate
            ensure!(new_capacity > 0, Error::<T>::InvalidCapacity);

            // Get operator's node
            let peer_id =
                OperatorNodes::<T>::get(&operator).ok_or(Error::<T>::NodeNotRegistered)?;

            // Update
            StorageNodes::<T>::try_mutate(&peer_id, |maybe_info| -> DispatchResult {
                let info = maybe_info.as_mut().ok_or(Error::<T>::NodeNotRegistered)?;
                info.capacity = new_capacity;
                Ok(())
            })?;

            // Emit event
            Self::deposit_event(Event::NodeUpdated { peer_id, new_capacity });

            Ok(())
        }

        /// Unregister a storage node.
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(2, 2))]
        pub fn unregister_node(origin: OriginFor<T>) -> DispatchResult {
            let operator = ensure_signed(origin)?;

            // Get operator's node
            let peer_id =
                OperatorNodes::<T>::get(&operator).ok_or(Error::<T>::NodeNotRegistered)?;

            // Check for active holdings
            let holdings = NodeHoldings::<T>::get(&peer_id);
            ensure!(holdings.is_empty(), Error::<T>::NodeHasHoldings);

            // Remove
            StorageNodes::<T>::remove(&peer_id);
            OperatorNodes::<T>::remove(&operator);

            // Emit event
            Self::deposit_event(Event::NodeUnregistered { peer_id, operator });

            Ok(())
        }

        /// Declare holding a fragment (FR-406).
        ///
        /// Rate limited to MaxDeclarationsPerBlockPerNode per block per node.
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(25_000_000, 0) + T::DbWeight::get().reads_writes(5, 3))]
        pub fn declare_holding(origin: OriginFor<T>, fragment_id: FragmentId) -> DispatchResult {
            let operator = ensure_signed(origin)?;
            let current_block = frame_system::Pallet::<T>::block_number();

            // Get operator's node
            let peer_id =
                OperatorNodes::<T>::get(&operator).ok_or(Error::<T>::NodeNotRegistered)?;

            // Check per-block per-node declaration rate limit (FR-406)
            let current_count = DeclareHoldingCountPerBlock::<T>::get(current_block, &peer_id);
            ensure!(
                crate::rate_limit::can_declare_holding(current_count, T::MaxDeclarationsPerBlockPerNode::get()),
                Error::<T>::TooManyDeclarationsThisBlock
            );

            // Check fragment exists
            ensure!(Fragments::<T>::contains_key(fragment_id), Error::<T>::FragmentNotFound);

            // Update FragmentHolders — track whether actual modification happened
            let mut modified = false;
            FragmentHolders::<T>::try_mutate(fragment_id, |holders| -> DispatchResult {
                // Idempotency check - if already holding, just return Ok
                if holders.iter().any(|h| h == &peer_id) {
                    return Ok(());
                }

                holders
                    .try_push(peer_id.clone())
                    .map_err(|_| Error::<T>::TooManyHolders)?;
                modified = true;
                Ok(())
            })?;

            // Update NodeHoldings
            NodeHoldings::<T>::try_mutate(&peer_id, |fragments| -> DispatchResult {
                // Idempotency check
                if fragments.contains(&fragment_id) {
                    return Ok(());
                }

                fragments
                    .try_push(fragment_id)
                    .map_err(|_| Error::<T>::TooManyFragments)?;
                modified = true;
                Ok(())
            })?;

            // SECURITY (#26-CRIT-3): only consume rate-limit budget when state actually changed.
            // Previously, an idempotent no-op (already declared) still consumed budget, allowing
            // an adversary to exhaust other operators' rate budget by spamming declared fragments.
            if modified {
                DeclareHoldingCountPerBlock::<T>::insert(
                    current_block,
                    &peer_id,
                    crate::rate_limit::increment_declaration_count(current_count),
                );
            }

            // Emit event only on real modification
            if modified {
                Self::deposit_event(Event::HoldingDeclared { peer_id, fragment_id });
            }

            Ok(())
        }

        /// Revoke holding declaration.
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(2, 2))]
        pub fn revoke_holding(origin: OriginFor<T>, fragment_id: FragmentId) -> DispatchResult {
            let operator = ensure_signed(origin)?;

            // Get operator's node
            let peer_id =
                OperatorNodes::<T>::get(&operator).ok_or(Error::<T>::NodeNotRegistered)?;

            // Remove from FragmentHolders
            FragmentHolders::<T>::try_mutate(fragment_id, |holders| -> DispatchResult {
                let pos = holders.iter().position(|h| h == &peer_id).ok_or(Error::<T>::NotHolding)?;
                holders.swap_remove(pos);
                Ok(())
            })?;

            // Remove from NodeHoldings
            NodeHoldings::<T>::try_mutate(&peer_id, |fragments| -> DispatchResult {
                let pos =
                    fragments.iter().position(|f| f == &fragment_id).ok_or(Error::<T>::NotHolding)?;
                fragments.swap_remove(pos);
                Ok(())
            })?;

            // Emit event
            Self::deposit_event(Event::HoldingRevoked { peer_id, fragment_id });

            Ok(())
        }

        /// Submit KZG proof of holding (US2).
        ///
        /// Storage nodes submit proof in response to a challenge.
        /// The proof demonstrates they hold the correct share value.
        ///
        /// # Arguments
        /// * `content_hash` - The content being proven
        /// * `share_index` - Which share the node holds (1..=n)
        /// * `share_value` - The share value (32 bytes)
        /// * `proof` - KZG opening proof (48 bytes)
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0) + T::DbWeight::get().reads_writes(3, 2))]
        pub fn prove_holding_kzg(
            origin: OriginFor<T>,
            content_hash: super::ContentHash,
            share_index: u8,
            share_value: BoundedVec<u8, ConstU32<32>>,
            proof: BoundedVec<u8, ConstU32<48>>,
        ) -> DispatchResult {
            let prover = ensure_signed(origin)?;

            // SECURITY: Require active challenge to prevent replay attacks (PR #22 CRITICAL-1)
            // Without this check, the same proof could be submitted infinitely for rewards
            let challenge = PendingChallenges::<T>::get(content_hash, share_index)
                .ok_or(Error::<T>::NotChallenged)?;

            // SECURITY: Verify the prover is the challenged node
            ensure!(challenge.challenged_node == prover, Error::<T>::NotChallenged);

            // SECURITY: Prevent duplicate proof submissions within the same block
            let current_block = <frame_system::Pallet<T>>::block_number();
            let existing_record = ProofRecords::<T>::get(content_hash, &prover);
            ensure!(
                existing_record.last_proved_at != current_block,
                Error::<T>::ProofAlreadySubmitted
            );

            // Validate share value length
            ensure!(share_value.len() == 32, Error::<T>::FragmentTooSmall);

            // Validate proof length
            ensure!(proof.len() == 48, Error::<T>::InvalidCommitmentLength);

            // Get KZG fragment
            let kzg_fragment = KzgFragments::<T>::get(content_hash)
                .ok_or(Error::<T>::KzgFragmentNotFound)?;

            // SECURITY: Verify the prover is a registered holder (PR #22 CRITICAL-2)
            // Without this check, any account could submit proofs for others' fragments
            ensure!(kzg_fragment.holders.contains(&prover), Error::<T>::NotHolder);

            // Validate share index
            ensure!(
                share_index >= 1 && share_index <= kzg_fragment.fragment_count,
                Error::<T>::InvalidChallengeIndex
            );

            // Convert BoundedVec to fixed arrays for KZG verification
            let commitment_arr: [u8; 48] = kzg_fragment.commitment.as_slice()
                .try_into()
                .map_err(|_| Error::<T>::InvalidCommitmentLength)?;
            let share_value_arr: [u8; 32] = share_value.as_slice()
                .try_into()
                .map_err(|_| Error::<T>::FragmentTooSmall)?;
            let proof_arr: [u8; 48] = proof.as_slice()
                .try_into()
                .map_err(|_| Error::<T>::InvalidCommitmentLength)?;

            // Verify KZG proof
            // Note: In benchmark mode, we perform the verification for weight measurement
            // but skip the result check since test vectors aren't mathematically valid.
            // Production code ALWAYS verifies the proof.
            let is_valid = super::kzg::verify_kzg_proof(
                &commitment_arr,
                share_index,
                &share_value_arr,
                &proof_arr,
            ).map_err(|_| Error::<T>::InvalidKzgProof)?;

            #[cfg(not(feature = "runtime-benchmarks"))]
            ensure!(is_valid, Error::<T>::InvalidKzgProof);
            
            // In benchmark mode, always treat as valid for weight measurement
            #[cfg(feature = "runtime-benchmarks")]
            let _ = is_valid;

            // Calculate reward (T046-T047)
            let score = ScoreCache::<T>::get(content_hash).unwrap_or(1000); // Default high score
            let score_threshold = T::ScoreThreshold::get();
            let reward = super::rewards::calculate_reward_with_threshold(
                kzg_fragment.data_size,
                T::BaseRewardPerByte::get(),
                score,
                score_threshold,
            );

            // Update proof record (FR-104, FR-109, T042)
            // Note: Reward is accumulated ONLY in PendingRewards (not in ProofRecord.pending_reward)
            // to prevent double-counting (Issue 3 fix)
            ProofRecords::<T>::mutate(content_hash, &prover, |record| {
                record.last_proved_at = current_block;
                record.success_count = record.success_count.saturating_add(1);
                record.failure_count = 0; // Reset failures on success
            });

            // Accumulate reward in PendingRewards for efficient claim (T048)
            if reward > 0 {
                PendingRewards::<T>::mutate(&prover, |pending| {
                    *pending = pending.saturating_add(reward);
                });

                // Score recovered - remove from forgetting candidates if present (T054, T072 hysteresis)
                // To recover from forgetting candidate state, score must exceed threshold + hysteresis margin
                // This prevents rapid toggling between eligible/ineligible states
                let recovery_threshold = score_threshold.saturating_add(T::ScoreHysteresisMargin::get());
                if ForgettingCandidates::<T>::contains_key(content_hash) && score >= recovery_threshold {
                    ForgettingCandidates::<T>::remove(content_hash);
                    Self::deposit_event(Event::ScoreRecovered { content_hash });
                }
            } else {
                // Reward is 0 due to low score - mark as forgetting candidate (T056, FR-110)
                if !ForgettingCandidates::<T>::contains_key(content_hash) {
                    ForgettingCandidates::<T>::insert(content_hash, current_block);
                    Self::deposit_event(Event::ForgettingCandidateMarked {
                        content_hash,
                        marked_at: current_block,
                    });
                }
            }

            // Remove pending challenge if exists
            PendingChallenges::<T>::remove(content_hash, share_index);

            // Emit success event
            Self::deposit_event(Event::HoldingProved {
                content_hash,
                holder: prover,
                share_index,
            });

            Ok(())
        }

        /// Issue a challenge to a storage node (US2).
        ///
        /// Challenges are issued to verify storage nodes still hold data.
        /// Nodes must respond with valid KZG proofs within a time window.
        ///
        /// # Security: Rate-limited per block and validates node is actual holder
        ///
        /// # Arguments
        /// * `content_hash` - The content to challenge
        /// * `node_account` - The storage node operator account (must be in holders list)
        /// * `challenge_index` - Random share index to prove (1..=fragment_count)
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(30_000, 0) + T::DbWeight::get().reads_writes(3, 2))]
        pub fn issue_challenge(
            origin: OriginFor<T>,
            content_hash: super::ContentHash,
            node_account: T::AccountId,
            challenge_index: u8,
        ) -> DispatchResult {
            let issuer = ensure_signed(origin)?;

            // Security: Only registered storage nodes can issue challenges (Issue 1)
            ensure!(
                OperatorNodes::<T>::contains_key(&issuer),
                Error::<T>::IssuerNotRegisteredNode
            );

            let current_block = <frame_system::Pallet<T>>::block_number();

            // Rate limiting: check per-block challenge count for this issuer
            let current_count = ChallengeCountPerBlock::<T>::get(current_block, &issuer);
            ensure!(
                current_count < T::MaxChallengesPerBlock::get(),
                Error::<T>::ChallengeLimitExceeded
            );

            // Get KZG fragment
            let kzg_fragment = KzgFragments::<T>::get(content_hash)
                .ok_or(Error::<T>::KzgFragmentNotFound)?;

            // Security: Verify target node is actually a holder of this fragment
            ensure!(
                kzg_fragment.holders.contains(&node_account),
                Error::<T>::NotHolderOfFragment
            );

            // Validate challenge index
            ensure!(
                challenge_index >= 1 && challenge_index <= kzg_fragment.fragment_count,
                Error::<T>::InvalidChallengeIndex
            );

            // Check if challenge already exists for this share
            ensure!(
                !PendingChallenges::<T>::contains_key(content_hash, challenge_index),
                Error::<T>::ChallengeAlreadyIssued
            );

            // Calculate deadline (100 blocks from now)
            let deadline = current_block.saturating_add(100u32.into());

            // Create and store challenge
            let challenge = Challenge {
                content_hash,
                share_index: challenge_index,
                challenged_node: node_account.clone(),
                issued_at: current_block,
                deadline,
            };

            PendingChallenges::<T>::insert(content_hash, challenge_index, challenge);

            // Index challenge by deadline for efficient expiration cleanup (Issue 2, T010)
            ChallengesByDeadline::<T>::try_mutate(deadline, |challenges| {
                challenges.try_push((content_hash, challenge_index))
            }).map_err(|_| Error::<T>::ChallengeLimitExceeded)?;

            // Increment challenge count for rate limiting
            ChallengeCountPerBlock::<T>::insert(
                current_block,
                &issuer,
                current_count.saturating_add(1),
            );

            // Emit event
            Self::deposit_event(Event::ChallengeIssued {
                content_hash,
                share_index: challenge_index,
                target_node: node_account,
                deadline,
            });

            Ok(())
        }

        /// Claim accumulated rewards (T048, FR-108).
        ///
        /// Transfers pending rewards from PendingRewards storage to caller's balance.
        /// T084: Actually mints tokens to claimer's account.
        /// T040: MinWithdrawalAmount check (500 MORAL minimum)
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(50_000, 0) + T::DbWeight::get().reads_writes(2, 3))]
        pub fn claim_reward(origin: OriginFor<T>) -> DispatchResult {
            let claimer = ensure_signed(origin)?;

            // Get accumulated pending rewards
            let total_reward = PendingRewards::<T>::get(&claimer);
            ensure!(total_reward > 0, Error::<T>::NoPendingReward);

            // T040: MinWithdrawalAmount check (500 MORAL = 500_000_000_000_000 with 12 decimals)
            let min_withdrawal: u128 = T::MinWithdrawalAmount::get();
            ensure!(total_reward >= min_withdrawal, Error::<T>::InsufficientAccruedRewards);

            // Check pool balance
            let pool_balance = RewardPoolBalance::<T>::get();
            let payout = core::cmp::min(total_reward, pool_balance);
            ensure!(payout > 0, Error::<T>::InsufficientRewardPool);

            // Deduct from pool
            RewardPoolBalance::<T>::mutate(|balance| {
                *balance = balance.saturating_sub(payout);
            });

            // T084: Actually mint tokens to claimer's balance
            // Convert u128 payout to BalanceOf<T>
            let payout_balance: BalanceOf<T> = payout.try_into().map_err(|_| Error::<T>::ArithmeticOverflow)?;
            T::NativeToken::mint_into(&claimer, payout_balance)
                .map_err(|_| Error::<T>::RewardMintFailed)?;

            // Clear pending rewards
            // If pro-rata applied (pool exhausted), remainder stays in PendingRewards
            if payout < total_reward {
                // Pro-rata: only clear what was paid out
                PendingRewards::<T>::mutate(&claimer, |pending| {
                    *pending = pending.saturating_sub(payout);
                });
            } else {
                // Full payout: clear all
                PendingRewards::<T>::remove(&claimer);
            }

            // Emit event
            Self::deposit_event(Event::RewardClaimed {
                holder: claimer,
                amount: payout,
            });

            Ok(())
        }

        /// Confirm repair of a fragment share (013-slashing-repair T024).
        ///
        /// A new storage node submits a KZG proof demonstrating they hold a valid
        /// regenerated share. On success, the node is added as a holder of the fragment.
        ///
        /// # Prerequisites
        /// - Fragment must be in AtRisk state (holder_count <= 4)
        /// - Caller must be a registered storage node
        /// - KZG proof must be valid for the share index
        ///
        /// # Arguments
        /// * `content_hash` - The content being repaired
        /// * `share_index` - The new share index (should be > original fragment_count)
        /// * `share_value` - The new share value (32-byte BLS12-381 scalar) — required
        ///   for real KZG verification. (#26-CRIT-1)
        /// * `kzg_proof` - KZG opening proof (48 bytes)
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(100_000_000, 0) + T::DbWeight::get().reads_writes(5, 4))]
        pub fn confirm_repair(
            origin: OriginFor<T>,
            content_hash: super::ContentHash,
            share_index: u8,
            share_value: BoundedVec<u8, ConstU32<32>>,
            kzg_proof: BoundedVec<u8, ConstU32<48>>,
        ) -> DispatchResult {
            let new_holder = ensure_signed(origin)?;

            // Verify caller is a registered storage node
            ensure!(
                OperatorNodes::<T>::contains_key(&new_holder),
                Error::<T>::NodeNotRegistered
            );

            // Get the fragment
            let kzg_fragment = KzgFragments::<T>::get(content_hash)
                .ok_or(Error::<T>::KzgFragmentNotFound)?;

            // Verify fragment is in AtRisk state (needs repair)
            let state = FragmentStates::<T>::get(content_hash);
            ensure!(
                state.kind == FragmentStateKind::AtRisk,
                Error::<T>::FragmentNotAtRisk
            );

            // Verify caller is not already a holder
            ensure!(
                !kzg_fragment.holders.contains(&new_holder),
                Error::<T>::AlreadyHolding
            );

            // (#26-CRIT-1) Real KZG verification — previously this called
            // `verify_share_proof` which was a stub returning `Ok(true)`,
            // letting any storage node claim to hold a freshly repaired share
            // without proving it. Now we pair-check via the same verifier
            // used by `prove_holding_kzg`.
            let commitment_arr: [u8; 48] = kzg_fragment.commitment.as_slice()
                .try_into()
                .map_err(|_| Error::<T>::InvalidCommitmentLength)?;
            let share_value_arr: [u8; 32] = share_value.as_slice()
                .try_into()
                .map_err(|_| Error::<T>::InvalidShareValue)?;
            let proof_arr: [u8; 48] = kzg_proof.as_slice()
                .try_into()
                .map_err(|_| Error::<T>::InvalidKzgProof)?;
            let is_valid = crate::kzg::verify_kzg_proof(
                &commitment_arr,
                share_index,
                &share_value_arr,
                &proof_arr,
            ).map_err(|_| Error::<T>::InvalidKzgProof)?;
            #[cfg(not(feature = "runtime-benchmarks"))]
            ensure!(is_valid, Error::<T>::InvalidKzgProof);
            #[cfg(feature = "runtime-benchmarks")]
            let _ = is_valid;

            // Add new holder to fragment
            KzgFragments::<T>::try_mutate(content_hash, |maybe_fragment| -> DispatchResult {
                let fragment = maybe_fragment.as_mut().ok_or(Error::<T>::KzgFragmentNotFound)?;
                fragment.holders.try_push(new_holder.clone())
                    .map_err(|_| Error::<T>::FragmentHasTooManyHolders)?;
                Ok(())
            })?;

            // Create ProofRecord for new holder
            let current_block = <frame_system::Pallet<T>>::block_number();
            ProofRecords::<T>::insert(content_hash, &new_holder, ProofRecord {
                last_proved_at: current_block,
                success_count: 1,
                failure_count: 0,
                slashed: false,
                share_index,
            });

            // T051: Distribute RepairRewardPool to reporter
            let pool_balance = RepairRewardPools::<T>::take(content_hash);
            if pool_balance > 0 {
                PendingRewards::<T>::mutate(&new_holder, |pending| {
                    *pending = pending.saturating_add(pool_balance);
                });
            }

            // Update fragment state (may transition back to Active)
            Self::update_fragment_state(content_hash);

            // Emit RepairCompleted event
            Self::deposit_event(Event::RepairCompleted {
                content_hash,
                new_holder,
                new_share_index: share_index,
            });

            Ok(())
        }

        /// Evict the lowest priority holder from a fragment with excess holders (T056)
        ///
        /// Called by any storage node to clean up fragments that have more than
        /// fragment_count holders due to repairs while old nodes were offline.
        ///
        /// # Arguments
        /// * `content_hash` - The content hash of the fragment
        ///
        /// # Effects
        /// - Removes the lowest priority holder from the fragment
        /// - Emits HolderEvicted event
        #[pallet::call_index(16)]
        #[pallet::weight(Weight::from_parts(50_000, 0))]
        pub fn evict_stale_holder(
            origin: OriginFor<T>,
            content_hash: super::ContentHash,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            // Get the fragment
            let kzg_fragment = KzgFragments::<T>::get(content_hash)
                .ok_or(Error::<T>::KzgFragmentNotFound)?;

            // Check if there are excess holders
            let holder_count = kzg_fragment.holders.len() as u8;
            let max_holders = kzg_fragment.fragment_count;
            ensure!(holder_count > max_holders, Error::<T>::NoExcessHolders);

            // Get eviction candidates sorted by priority
            let candidates = Self::compute_eviction_candidates(content_hash);
            ensure!(!candidates.is_empty(), Error::<T>::NoExcessHolders);

            // Evict the lowest priority holder (first in sorted list)
            let evicted_holder = candidates[0].account_id.clone();

            // Remove from holders list
            KzgFragments::<T>::try_mutate(content_hash, |maybe_fragment| -> DispatchResult {
                let fragment = maybe_fragment.as_mut().ok_or(Error::<T>::KzgFragmentNotFound)?;
                fragment.holders.retain(|h| *h != evicted_holder);
                Ok(())
            })?;

            // Remove ProofRecord for evicted holder
            ProofRecords::<T>::remove(content_hash, &evicted_holder);

            // Emit event
            Self::deposit_event(Event::HolderEvicted {
                content_hash,
                evicted_holder,
            });

            Ok(())
        }
    }

    // ============ Helper Functions ============

    impl<T: Config> Pallet<T> {
        /// Validate PeerID format (FR-405).
        /// libp2p PeerIDs are typically 38-52 bytes for Ed25519.
        fn validate_peer_id(peer_id: &BoundedVec<u8, T::MaxPeerIdLen>) -> DispatchResult {
            let len = peer_id.len() as u32;
            // Minimum length check (38 bytes for Ed25519 multihash)
            ensure!(len >= T::MinPeerIdLen::get(), Error::<T>::PeerIdTooShort);
            // Maximum length check (bounded by MaxPeerIdLen but explicit error)
            ensure!(len <= T::MaxPeerIdLen::get(), Error::<T>::PeerIdTooLong);
            Ok(())
        }

        /// Validate node capacity meets minimum requirement (FR-411).
        fn validate_capacity(capacity: u64) -> DispatchResult {
            ensure!(capacity >= T::MinNodeCapacity::get(), Error::<T>::CapacityTooSmall);
            Ok(())
        }

        /// Get the recent registration count for PoW difficulty calculation.
        /// Sums registrations over the last PowObservationPeriod blocks.
        fn get_recent_registrations() -> u32 {
            let current_block = frame_system::Pallet::<T>::block_number();
            let period = T::PowObservationPeriod::get();
            let mut count = 0u32;

            for i in 0..period {
                if let Some(block) = current_block.checked_sub(&i.into()) {
                    count = count.saturating_add(RegistrationCountPerBlock::<T>::get(block));
                }
            }
            count
        }

        /// Get current PoW difficulty based on recent registrations (FR-409).
        pub fn current_pow_difficulty() -> u8 {
            let recent = Self::get_recent_registrations();
            crate::pow::calculate_difficulty(recent, T::BasePowDifficulty::get())
        }

        /// Internal fragment registration for cross-pallet calls (FR-401, FR-402).
        ///
        /// This bypasses the signed origin requirement since the caller pallet
        /// (e.g., Post Pallet) has already verified the origin.
        pub fn do_register_fragment_internal(
            fragment_id: FragmentId,
            size: u32,
            creator: T::AccountId,
            created_at: BlockNumberFor<T>,
        ) -> DispatchResult {
            // Validate size
            ensure!(size > 0, Error::<T>::FragmentTooSmall);
            ensure!(size <= T::MaxFragmentSize::get(), Error::<T>::FragmentTooLarge);

            // Check for duplicates
            ensure!(!Fragments::<T>::contains_key(fragment_id), Error::<T>::FragmentAlreadyExists);

            // Create metadata
            let metadata = FragmentMetadata {
                size,
                creator: creator.clone(),
                created_at,
            };

            // Store
            Fragments::<T>::insert(fragment_id, metadata);

            // Emit event
            Self::deposit_event(Event::FragmentRegistered { fragment_id, creator, size });

            Ok(())
        }

        /// Register a KZG fragment for proof-of-holding (FR-102, Issue 4 fix).
        ///
        /// Internal function - NOT callable as extrinsic.
        /// Called by Post pallet via StorageInterface during create_post_v2.
        ///
        /// # Arguments
        /// * `owner` - The owner account (verified by calling pallet)
        /// * `content_hash` - Blake2-256 hash of the content
        /// * `commitment` - KZG commitment (48 bytes, compressed G1 point)
        /// * `data_size` - Original data size in bytes
        /// * `fragment_count` - Total number of shares (n)
        /// * `threshold` - Recovery threshold (k)
        pub fn do_register_kzg_fragment(
            owner: T::AccountId,
            content_hash: super::ContentHash,
            commitment: BoundedVec<u8, ConstU32<48>>,
            data_size: u32,
            fragment_count: u8,
            threshold: u8,
        ) -> DispatchResult {
            // Validate commitment length (must be exactly 48 bytes)
            ensure!(commitment.len() == 48, Error::<T>::InvalidCommitmentLength);

            // Validate fragment count (minimum 2)
            ensure!(fragment_count >= 2, Error::<T>::FragmentCountTooSmall);

            // Validate threshold (1 <= k <= n)
            ensure!(
                threshold >= 1 && threshold <= fragment_count,
                Error::<T>::InvalidKzgThreshold
            );

            // Validate data size
            ensure!(data_size > 0, Error::<T>::FragmentTooSmall);

            // Check for duplicates
            ensure!(
                !KzgFragments::<T>::contains_key(content_hash),
                Error::<T>::KzgFragmentAlreadyExists
            );

            // Create KZG fragment metadata
            let kzg_fragment = KzgFragment::<T> {
                owner: owner.clone(),
                commitment: commitment.clone(),
                data_size,
                fragment_count,
                threshold,
                created_at: frame_system::Pallet::<T>::block_number(),
                holders: BoundedVec::default(),
            };

            // Store
            KzgFragments::<T>::insert(content_hash, kzg_fragment);

            // Emit event
            Self::deposit_event(Event::KzgFragmentRegistered {
                content_hash,
                owner,
                commitment,
                data_size,
                fragment_count,
                threshold,
            });

            Ok(())
        }

        // ============ Self-Repair Helper Functions (013-slashing-repair) ============

        /// Update fragment state based on current holder count (T015)
        ///
        /// Called after holder count changes to transition between states:
        /// - Active: holder_count >= 5
        /// - AtRisk: 3 <= holder_count <= 4
        /// - Lost: holder_count <= 2
        pub fn update_fragment_state(content_hash: super::ContentHash) {
            let fragment = match KzgFragments::<T>::get(content_hash) {
                Some(f) => f,
                None => return,
            };
            
            let holder_count = fragment.holders.len() as u32;
            let current_block = frame_system::Pallet::<T>::block_number();
            let current_state = FragmentStates::<T>::get(content_hash);
            
            let new_kind = if holder_count >= 5 {
                FragmentStateKind::Active
            } else if holder_count >= 3 {
                FragmentStateKind::AtRisk
            } else {
                FragmentStateKind::Lost
            };
            
            // Only update if state changed
            if new_kind != current_state.kind {
                FragmentStates::<T>::insert(content_hash, FragmentState::<T> {
                    kind: new_kind.clone(),
                    changed_at: current_block,
                });
                
                // Emit appropriate event
                match new_kind {
                    FragmentStateKind::AtRisk => {
                        Self::deposit_event(Event::FragmentAtRisk {
                            content_hash,
                            holder_count,
                        });
                    }
                    FragmentStateKind::Lost => {
                        Self::deposit_event(Event::FragmentLost {
                            content_hash,
                            holder_count,
                        });
                    }
                    _ => {}
                }
            }
        }

        /// Compute eviction candidates sorted by priority (T016)
        ///
        /// Lower priority_score = higher eviction priority
        /// Score factors:
        /// - Slashed nodes: score = 0 (highest eviction priority)
        /// - Old share indices (1-5): +0
        /// - New share indices (6+): +100
        /// - Recent proofs: higher score (less likely to evict)
        pub fn compute_eviction_candidates(
            content_hash: super::ContentHash,
        ) -> sp_std::vec::Vec<EvictionCandidate<T::AccountId, BlockNumberFor<T>>> {
            let fragment = match KzgFragments::<T>::get(content_hash) {
                Some(f) => f,
                None => return sp_std::vec::Vec::new(),
            };
            
            let mut candidates: sp_std::vec::Vec<EvictionCandidate<T::AccountId, BlockNumberFor<T>>> = 
                fragment.holders.iter().map(|holder| {
                    let record = ProofRecords::<T>::get(content_hash, holder);
                    
                    // Calculate priority score (lower = more likely to evict)
                    let mut score: u32 = 0;
                    
                    // Slashed nodes get lowest score (evict first)
                    if !record.slashed {
                        score += 1000;
                    }
                    
                    // Original indices (1-5) are more valuable than new indices (6+)
                    if record.share_index <= 5 && record.share_index > 0 {
                        // Do not add anything for original indices
                    } else {
                        score += 100;
                    }
                    
                    // Recent proofs get higher score (evict later)
                    // Convert BlockNumber to u32 for scoring
                    let last_proved: u32 = record.last_proved_at.try_into().unwrap_or(0);
                    score += core::cmp::min(last_proved / 100, 500);
                    
                    EvictionCandidate {
                        account_id: holder.clone(),
                        priority_score: score,
                        share_index: record.share_index,
                        is_slashed: record.slashed,
                        last_proved_at: record.last_proved_at,
                    }
                }).collect();
            
            // Sort by priority_score ascending (lowest first = evict first)
            candidates.sort_by_key(|c| c.priority_score);
            
            candidates
        }

        // (#26-CRIT-1) The previous `verify_share_proof` stub has been removed.
        // Repair / holding declaration paths now call `crate::kzg::verify_kzg_proof`
        // directly (the same verifier used by `prove_holding_kzg`), which performs
        // the real BLS12-381 pairing check against the embedded τ G2 from the
        // Ethereum KZG ceremony.

        /// Slash a node for failed challenges (T046)
        ///
        /// Called when a node fails to respond to 3 consecutive challenges.
        /// Applies 50% penalty to pending rewards and moves funds to RepairRewardPool.
        ///
        /// # Arguments
        /// * `node` - Account ID of the node to slash
        /// * `content_hash` - Content hash for the fragment
        ///
        /// # Effects
        /// - Sets `slashed = true` on ProofRecord
        /// - Confiscates 50% of PendingRewards
        /// - Adds confiscated amount to RepairRewardPools
        /// - Emits NodeSlashed event
        pub fn do_slash_node(
            node: T::AccountId,
            content_hash: ContentHash,
        ) -> DispatchResult {
            // SECURITY (#27-HIGH-4): use mutate() for atomic read-modify-write on ProofRecords
            // to avoid inter-block races where two slash paths could both observe slashed=false.
            let penalty: u128 = ProofRecords::<T>::try_mutate(
                content_hash,
                &node,
                |record| -> Result<u128, DispatchError> {
                    if record.slashed {
                        return Err(Error::<T>::AlreadySlashed.into());
                    }
                    record.slashed = true;
                    let pending = PendingRewards::<T>::get(&node);
                    Ok(pending / 2)
                },
            )?;

            // Apply penalty: reduce pending rewards
            PendingRewards::<T>::mutate(&node, |p| {
                *p = p.saturating_sub(penalty);
            });

            // Move penalty to RepairRewardPool for this content
            RepairRewardPools::<T>::mutate(content_hash, |pool| {
                *pool = pool.saturating_add(penalty);
            });

            // Emit event
            Self::deposit_event(Event::NodeSlashed {
                node: node.clone(),
                content_hash,
                penalty_amount: penalty,
            });

            Ok(())
        }
    }
}

// ============ StorageInterface Implementation ============

impl<T: Config> StorageInterface<T::AccountId, BlockNumberFor<T>> for Pallet<T> {
    fn do_register_fragment(
        fragment_id: FragmentId,
        size: u32,
        creator: T::AccountId,
        created_at: BlockNumberFor<T>,
    ) -> DispatchResult {
        Self::do_register_fragment_internal(fragment_id, size, creator, created_at)
    }

    fn do_register_kzg_fragment(
        owner: T::AccountId,
        content_hash: ContentHash,
        commitment: sp_std::vec::Vec<u8>,
        data_size: u32,
        fragment_count: u8,
        threshold: u8,
    ) -> DispatchResult {
        // Convert Vec<u8> to BoundedVec<u8, ConstU32<48>>
        let bounded_commitment = frame_support::BoundedVec::try_from(commitment)
            .map_err(|_| pallet::Error::<T>::InvalidCommitmentLength)?;
        Self::do_register_kzg_fragment(owner, content_hash, bounded_commitment, data_size, fragment_count, threshold)
    }

    fn do_deposit_to_reward_pool(amount: u128) {
        RewardPoolBalance::<T>::mutate(|balance| {
            *balance = balance.saturating_add(amount);
        });
    }

    fn do_release_fragment(content_hash: ContentHash) -> DispatchResult {
        // Idempotent: if no on-chain trace of this fragment exists, return silently
        // without emitting an event. Otherwise drop every related storage entry and
        // emit `ForgottenByPolicy` so storage nodes know to physically delete the data.
        //
        // Note: `FragmentId` and `ContentHash` are both `[u8; 32]` and pallet-post passes
        // the same hash (the Merkle root) to both `do_register_fragment` and
        // `do_register_kzg_fragment`, so we clear both maps using `content_hash`.
        let mut existed = false;
        if Fragments::<T>::take(content_hash).is_some() {
            existed = true;
        }
        if KzgFragments::<T>::take(content_hash).is_some() {
            existed = true;
        }
        // ProofRecords is StorageDoubleMap<ContentHash, AccountId, _>; clear all (h, *).
        // u32::MAX upper bound is fine: the set is bounded by holders per fragment in practice.
        let _ = ProofRecords::<T>::clear_prefix(content_hash, u32::MAX, None);
        // Drop forgetting flags + fragment state (presence implies prior registration).
        if ForgettingCandidates::<T>::take(content_hash).is_some() {
            existed = true;
        }
        if FragmentStates::<T>::contains_key(content_hash) {
            FragmentStates::<T>::remove(content_hash);
            existed = true;
        }

        if existed {
            Self::deposit_event(Event::ForgottenByPolicy { content_hash });
        }
        Ok(())
    }
}

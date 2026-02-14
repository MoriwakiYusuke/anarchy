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

pub mod pow;
pub mod rate_limit;

/// Fragment ID type (Blake2-256 hash)
pub type FragmentId = [u8; 32];

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

sp_api::decl_runtime_apis! {
    /// Storage Pallet Runtime API
    ///
    /// Provides access to on-chain storage node information for chain node RPC
    pub trait StorageApi {
        /// Get all registered storage nodes with their HTTP URLs
        fn get_all_storage_nodes() -> Vec<StorageNodeInfoRpc>;
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
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

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

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Clear per-block rate limiting counters at block finalization (FR-406, FR-410).
        fn on_finalize(block: BlockNumberFor<T>) {
            // Clear registration count for this block
            RegistrationCountPerBlock::<T>::remove(block);
            
            // Clear declaration counts for this block
            // Note: We clear by block number, not iterating over nodes
            let _ = DeclareHoldingCountPerBlock::<T>::clear_prefix(block, u32::MAX, None);
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

            // Update FragmentHolders
            FragmentHolders::<T>::try_mutate(fragment_id, |holders| -> DispatchResult {
                // Idempotency check - if already holding, just return Ok
                if holders.iter().any(|h| h == &peer_id) {
                    return Ok(());
                }

                holders
                    .try_push(peer_id.clone())
                    .map_err(|_| Error::<T>::TooManyHolders)?;
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
                Ok(())
            })?;

            // Increment declaration counter for this block and node
            DeclareHoldingCountPerBlock::<T>::insert(
                current_block,
                &peer_id,
                crate::rate_limit::increment_declaration_count(current_count),
            );

            // Emit event
            Self::deposit_event(Event::HoldingDeclared { peer_id, fragment_id });

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
}

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

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

/// Fragment ID type (Blake2-256 hash)
pub type FragmentId = [u8; 32];

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
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

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

        /// Register a storage node.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().writes(2))]
        pub fn register_node(
            origin: OriginFor<T>,
            peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
            capacity: u64,
        ) -> DispatchResult {
            let operator = ensure_signed(origin)?;

            // Validate
            ensure!(capacity > 0, Error::<T>::InvalidCapacity);
            Self::validate_peer_id(&peer_id)?;

            // Check for duplicates
            ensure!(
                !StorageNodes::<T>::contains_key(&peer_id),
                Error::<T>::NodeAlreadyRegistered
            );
            ensure!(
                !OperatorNodes::<T>::contains_key(&operator),
                Error::<T>::OperatorAlreadyHasNode
            );

            // Create node info
            let node_info = StorageNodeInfo {
                operator: operator.clone(),
                capacity,
                registered_at: frame_system::Pallet::<T>::block_number(),
            };

            // Store
            StorageNodes::<T>::insert(&peer_id, node_info);
            OperatorNodes::<T>::insert(&operator, peer_id.clone());

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

        /// Declare holding a fragment.
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(3, 2))]
        pub fn declare_holding(origin: OriginFor<T>, fragment_id: FragmentId) -> DispatchResult {
            let operator = ensure_signed(origin)?;

            // Get operator's node
            let peer_id =
                OperatorNodes::<T>::get(&operator).ok_or(Error::<T>::NodeNotRegistered)?;

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
        /// Validate PeerID format.
        /// libp2p PeerIDs are typically 38-52 bytes for Ed25519.
        fn validate_peer_id(peer_id: &BoundedVec<u8, T::MaxPeerIdLen>) -> DispatchResult {
            // Minimum length check (multihash prefix + key)
            ensure!(peer_id.len() >= 2, Error::<T>::InvalidPeerId);
            Ok(())
        }
    }
}

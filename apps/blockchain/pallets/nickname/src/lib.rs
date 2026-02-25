//! # Nickname Pallet
//!
//! Lightweight nickname management pallet.
//! Users can set, update, and clear display names for their AccountIds.
//!
//! ## Features
//! - Set/update nickname (max 128 bytes UTF-8)
//! - Clear nickname
//! - UTF-8 validation
//! - No fees (gas only)

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use alloc::vec::Vec;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Maximum nickname length in bytes (UTF-8)
        #[pallet::constant]
        type MaxNicknameLength: Get<u32>;
    }

    /// Nickname storage: AccountId → Optional<BoundedVec<u8>>
    #[pallet::storage]
    #[pallet::getter(fn nicknames)]
    pub type Nicknames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u8, T::MaxNicknameLength>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Nickname set or updated
        NicknameSet {
            who: T::AccountId,
            nickname: Vec<u8>,
        },
        /// Nickname cleared
        NicknameCleared {
            who: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Nickname exceeds maximum length
        NicknameTooLong,
        /// Nickname is empty
        NicknameEmpty,
        /// Nickname contains invalid UTF-8
        InvalidUtf8,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set or update nickname for the caller's account.
        ///
        /// # Arguments
        /// * `origin` - Signed origin
        /// * `nickname` - UTF-8 encoded nickname (1-128 bytes)
        ///
        /// # Errors
        /// * `NicknameEmpty` - If nickname is empty
        /// * `NicknameTooLong` - If nickname exceeds MaxNicknameLength
        /// * `InvalidUtf8` - If nickname is not valid UTF-8
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(0, 1))]
        pub fn set_nickname(origin: OriginFor<T>, nickname: Vec<u8>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Validate: not empty
            ensure!(!nickname.is_empty(), Error::<T>::NicknameEmpty);

            // Validate: UTF-8
            core::str::from_utf8(&nickname).map_err(|_| Error::<T>::InvalidUtf8)?;

            // Validate: length and convert to bounded
            let bounded: BoundedVec<u8, T::MaxNicknameLength> =
                nickname.clone().try_into().map_err(|_| Error::<T>::NicknameTooLong)?;

            // Store
            Nicknames::<T>::insert(&who, bounded);

            // Emit event
            Self::deposit_event(Event::NicknameSet {
                who,
                nickname,
            });

            Ok(())
        }

        /// Clear nickname for the caller's account.
        ///
        /// This operation is idempotent - it succeeds even if no nickname was set.
        ///
        /// # Arguments
        /// * `origin` - Signed origin
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(0, 1))]
        pub fn clear_nickname(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Remove from storage
            Nicknames::<T>::remove(&who);

            // Emit event
            Self::deposit_event(Event::NicknameCleared { who });

            Ok(())
        }
    }
}

//! # Faucet Pallet
//!
//! PoW Faucet for anonymous account initialization.
//! Users can obtain initial $moral tokens by completing a Proof-of-Work challenge.
//!
//! ## Features
//! - Blake2b-256 based PoW verification
//! - 1 account = 1 claim limit (Sybil attack prevention)
//! - Dynamic difficulty adjustment based on total claims
//! - No IP logging (preserves anonymity)

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::traits::fungible::{Inspect, Mutate};
    use frame_system::pallet_prelude::*;
    use sp_io::hashing::blake2_256;
    use sp_runtime::Saturating;
    use sp_runtime::transaction_validity::{
        InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
    };

    /// Balance type for $moral token
    pub type BalanceOf<T> =
        <<T as Config>::NativeToken as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

    /// Faucet claim record
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
    pub struct FaucetClaimRecord<T: Config> {
        /// Block number when claimed
        pub block_number: BlockNumberFor<T>,
        /// Amount received
        pub amount: BalanceOf<T>,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        // RuntimeEvent is inferred from frame_system::Config bound

        /// Native token ($moral) for minting rewards
        type NativeToken: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

        /// Base PoW difficulty (number of leading zero bits required)
        #[pallet::constant]
        type BaseDifficulty: Get<u8>;

        /// Scaling factor for difficulty adjustment (difficulty increases by 1 for every scaling_factor claims)
        #[pallet::constant]
        type DifficultyScalingFactor: Get<u64>;

        /// Maximum PoW difficulty cap
        #[pallet::constant]
        type MaxDifficulty: Get<u8>;

        /// Reward amount per successful claim (in planck)
        #[pallet::constant]
        type RewardAmount: Get<BalanceOf<Self>>;

        /// Challenge validity period (in blocks)
        #[pallet::constant]
        type ChallengeValidity: Get<BlockNumberFor<Self>>;
    }

    /// Faucet claim records - tracks which accounts have claimed
    #[pallet::storage]
    #[pallet::getter(fn faucet_claims)]
    pub type FaucetClaims<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, FaucetClaimRecord<T>, OptionQuery>;

    /// Total number of successful claims (used for difficulty adjustment)
    #[pallet::storage]
    #[pallet::getter(fn total_claims)]
    pub type TotalClaims<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Faucet claim successful
        FaucetClaimed {
            who: T::AccountId,
            amount: BalanceOf<T>,
            block_number: BlockNumberFor<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// This account has already claimed from the faucet
        AlreadyClaimed,
        /// The challenge has expired (block number too old)
        ChallengeExpired,
        /// The PoW proof is invalid (does not meet difficulty requirement)
        InvalidProof,
        /// The specified block number does not exist
        BlockNotFound,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Claim faucet reward by submitting a valid PoW proof (unsigned transaction).
        ///
        /// This is an unsigned extrinsic - no signature or account balance required.
        /// The PoW proof authenticates the request.
        ///
        /// # Arguments
        /// * `account` - The account to receive the faucet reward
        /// * `block_number` - The block number used to generate the challenge
        /// * `nonce` - The nonce that satisfies the PoW difficulty requirement
        ///
        /// # Errors
        /// * `AlreadyClaimed` - Account has already claimed from faucet
        /// * `ChallengeExpired` - Block number is too old
        /// * `InvalidProof` - Nonce does not satisfy difficulty requirement
        /// * `BlockNotFound` - Block number does not exist
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2, 2))]
        pub fn claim(
            origin: OriginFor<T>,
            account: T::AccountId,
            block_number: BlockNumberFor<T>,
            nonce: u64,
        ) -> DispatchResult {
            ensure_none(origin)?;

            // Check if already claimed
            ensure!(
                !FaucetClaims::<T>::contains_key(&account),
                Error::<T>::AlreadyClaimed
            );

            // Get current block number
            let current_block = frame_system::Pallet::<T>::block_number();

            // Check challenge validity (not expired)
            let validity = T::ChallengeValidity::get();
            let max_valid_block = block_number.saturating_add(validity);
            ensure!(current_block <= max_valid_block, Error::<T>::ChallengeExpired);

            // Get block hash for challenge generation
            let block_hash = frame_system::Pallet::<T>::block_hash(block_number);
            ensure!(
                block_hash != Default::default(),
                Error::<T>::BlockNotFound
            );

            // Compute challenge and verify proof
            let challenge = Self::compute_challenge(&block_hash, &account);
            let difficulty = Self::calculate_difficulty();
            ensure!(
                Self::verify_proof(&challenge, nonce, difficulty),
                Error::<T>::InvalidProof
            );

            // Mint reward to account
            let amount = T::RewardAmount::get();
            T::NativeToken::mint_into(&account, amount)?;

            // Record claim
            let record = FaucetClaimRecord {
                block_number,
                amount,
            };
            FaucetClaims::<T>::insert(&account, record);

            // Increment total claims
            TotalClaims::<T>::mutate(|c| *c = c.saturating_add(1));

            // Emit event
            Self::deposit_event(Event::FaucetClaimed {
                who: account,
                amount,
                block_number,
            });

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::claim { account, block_number, nonce } => {
                    // Check if already claimed
                    if FaucetClaims::<T>::contains_key(account) {
                        return InvalidTransaction::Custom(1).into();
                    }

                    // Get current block number
                    let current_block = frame_system::Pallet::<T>::block_number();

                    // Check challenge validity
                    let validity = T::ChallengeValidity::get();
                    let max_valid_block = block_number.saturating_add(validity);
                    if current_block > max_valid_block {
                        return InvalidTransaction::Custom(2).into();
                    }

                    // Get block hash
                    let block_hash = frame_system::Pallet::<T>::block_hash(*block_number);
                    if block_hash == Default::default() {
                        return InvalidTransaction::Custom(3).into();
                    }

                    // Verify PoW proof
                    let challenge = Self::compute_challenge(&block_hash, account);
                    let difficulty = Self::calculate_difficulty();
                    if !Self::verify_proof(&challenge, *nonce, difficulty) {
                        return InvalidTransaction::Custom(4).into();
                    }

                    ValidTransaction::with_tag_prefix("FaucetClaim")
                        .priority(100)
                        .and_provides((account.clone(), block_number))
                        .longevity(validity.try_into().unwrap_or(64))
                        .propagate(true)
                        .build()
                }
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// Calculate current difficulty based on total claims.
        /// Formula: min(base + floor(log2(1 + total_claims / scaling_factor)), max)
        pub fn calculate_difficulty() -> u8 {
            let total_claims = TotalClaims::<T>::get();
            let base = T::BaseDifficulty::get();
            let scaling_factor = T::DifficultyScalingFactor::get();
            let max = T::MaxDifficulty::get();

            if scaling_factor == 0 {
                return base.min(max);
            }

            // Calculate log2(1 + total_claims / scaling_factor)
            let ratio = 1u64.saturating_add(total_claims / scaling_factor);
            let log2_value = (64 - ratio.leading_zeros()) as u8;
            let adjustment = log2_value.saturating_sub(1); // log2(1) = 0, log2(2) = 1, etc.

            base.saturating_add(adjustment).min(max)
        }

        /// Compute challenge from block hash and account ID.
        /// challenge = blake2_256(block_hash || scale_encode(account_id))
        pub fn compute_challenge(
            block_hash: &T::Hash,
            account_id: &T::AccountId,
        ) -> [u8; 32] {
            let mut data = block_hash.as_ref().to_vec();
            data.extend(account_id.encode());
            blake2_256(&data)
        }

        /// Verify PoW proof.
        /// hash = blake2_256(challenge || nonce.to_le_bytes())
        /// valid = leading_zeros(hash) >= difficulty
        pub fn verify_proof(challenge: &[u8; 32], nonce: u64, difficulty: u8) -> bool {
            let mut data = challenge.to_vec();
            data.extend(nonce.to_le_bytes());
            let hash = blake2_256(&data);
            Self::count_leading_zero_bits(&hash) >= difficulty
        }

        /// Count leading zero bits in a 32-byte hash.
        pub fn count_leading_zero_bits(hash: &[u8; 32]) -> u8 {
            let mut count = 0u8;
            for byte in hash.iter() {
                if *byte == 0 {
                    count += 8;
                } else {
                    count += byte.leading_zeros() as u8;
                    break;
                }
            }
            count
        }
    }
}

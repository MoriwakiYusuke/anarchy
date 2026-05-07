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
    use primitives_pow::{compute_challenge as pow_compute_challenge, verify_proof as pow_verify_proof};
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

        /// Faucet 全体の累積発行上限 (TSTS P7)。
        ///
        /// `TotalMinted` がこの値以上に達すると claim は `FaucetCapReached` で失敗する。
        /// mainnet 投入後に永続インフレ尾を残さないための bootstrap 用 sunset 機構。
        /// 0 を指定すると cap 無効 (旧挙動)。spec §3.2.7 で 100,000 MORAL 推奨。
        #[pallet::constant]
        type TotalCap: Get<BalanceOf<Self>>;
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

    /// Total minted MORAL via faucet (TSTS P7).
    ///
    /// `TotalCap` との比較で sunset 制御に使う。`u128` 化して `BalanceOf<T>` 変換コストを抑える。
    #[pallet::storage]
    #[pallet::getter(fn total_minted)]
    pub type TotalMinted<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

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
        /// Faucet 累積発行上限に到達した (TSTS P7 sunset)
        FaucetCapReached,
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

            // Validate claim parameters using shared helper
            Self::validate_claim_params(&account, block_number, nonce).map_err(|e| match e {
                ClaimValidationError::AlreadyClaimed => Error::<T>::AlreadyClaimed,
                ClaimValidationError::ChallengeExpired => Error::<T>::ChallengeExpired,
                ClaimValidationError::BlockNotFound => Error::<T>::BlockNotFound,
                ClaimValidationError::InvalidProof => Error::<T>::InvalidProof,
                ClaimValidationError::CapReached => Error::<T>::FaucetCapReached,
            })?;

            // Mint reward to account
            let amount = T::RewardAmount::get();
            T::NativeToken::mint_into(&account, amount)?;

            // Record claim
            let record = FaucetClaimRecord {
                block_number,
                amount,
            };
            FaucetClaims::<T>::insert(&account, record);

            // Increment total claims and total minted (TSTS P7)
            TotalClaims::<T>::mutate(|c| *c = c.saturating_add(1));
            TotalMinted::<T>::mutate(|m| *m = m.saturating_add(amount));

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
                    // Validate claim parameters using shared helper
                    if let Err(e) = Self::validate_claim_params(account, *block_number, *nonce) {
                        return match e {
                            ClaimValidationError::AlreadyClaimed => InvalidTransaction::Custom(1).into(),
                            ClaimValidationError::ChallengeExpired => InvalidTransaction::Custom(2).into(),
                            ClaimValidationError::BlockNotFound => InvalidTransaction::Custom(3).into(),
                            ClaimValidationError::InvalidProof => InvalidTransaction::Custom(4).into(),
                            ClaimValidationError::CapReached => InvalidTransaction::Custom(5).into(),
                        };
                    }

                    let validity = T::ChallengeValidity::get();
                    // SECURITY: dedupe by account only — using (account, block, nonce) allowed
                    // multiple unsigned txs for the same account in one block to enter the pool.
                    // On-chain `claim` re-validates so double-claim is impossible, but pool spam
                    // is now mitigated at the validate_unsigned stage.
                    ValidTransaction::with_tag_prefix("FaucetClaim")
                        .priority(100)
                        .and_provides(account.clone())
                        .longevity(validity.try_into().unwrap_or(64))
                        .propagate(true)
                        .build()
                }
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    /// Pre-validation result for faucet claims (shared between claim and validate_unsigned)
    #[derive(Debug)]
    pub enum ClaimValidationError {
        AlreadyClaimed,
        ChallengeExpired,
        BlockNotFound,
        InvalidProof,
        /// Faucet 累積発行上限に到達 (TSTS P7)
        CapReached,
    }

    impl<T: Config> Pallet<T> {
        /// Validate faucet claim parameters (shared logic for claim and validate_unsigned).
        ///
        /// Returns Ok(()) if valid, or Err(ClaimValidationError) if invalid.
        pub fn validate_claim_params(
            account: &T::AccountId,
            block_number: BlockNumberFor<T>,
            nonce: u64,
        ) -> Result<(), ClaimValidationError> {
            // Check faucet sunset cap (TSTS P7).
            // TotalCap=0 を「上限なし」と解釈する (旧挙動互換)。それ以外は
            // `TotalMinted + RewardAmount > TotalCap` で予防的に弾く。
            let cap = T::TotalCap::get();
            if cap > BalanceOf::<T>::default() {
                let minted = TotalMinted::<T>::get();
                let reward = T::RewardAmount::get();
                if minted.saturating_add(reward) > cap {
                    return Err(ClaimValidationError::CapReached);
                }
            }

            // Check if already claimed
            if FaucetClaims::<T>::contains_key(account) {
                return Err(ClaimValidationError::AlreadyClaimed);
            }

            // Get current block number
            let current_block = frame_system::Pallet::<T>::block_number();

            // Check challenge validity (not expired)
            let validity = T::ChallengeValidity::get();
            let max_valid_block = block_number.saturating_add(validity);
            if current_block > max_valid_block {
                return Err(ClaimValidationError::ChallengeExpired);
            }

            // Get block hash for challenge generation
            let block_hash = frame_system::Pallet::<T>::block_hash(block_number);
            if block_hash == Default::default() {
                return Err(ClaimValidationError::BlockNotFound);
            }

            // Compute challenge and verify proof
            let challenge = Self::compute_challenge(&block_hash, account);
            let difficulty = Self::calculate_difficulty();
            if !Self::verify_proof(&challenge, nonce, difficulty) {
                return Err(ClaimValidationError::InvalidProof);
            }

            Ok(())
        }

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
            pow_compute_challenge(block_hash.as_ref(), &account_id.encode())
        }

        /// Verify PoW proof.
        /// hash = blake2_256(challenge || nonce.to_le_bytes())
        /// valid = leading_zeros(hash) >= difficulty
        pub fn verify_proof(challenge: &[u8; 32], nonce: u64, difficulty: u8) -> bool {
            pow_verify_proof(challenge, nonce, difficulty)
        }

        /// Count leading zero bits in a 32-byte hash.
        /// Uses saturating_add to prevent overflow (32 bytes * 8 bits = 256 > u8::MAX).
        pub fn count_leading_zero_bits(hash: &[u8; 32]) -> u8 {
            primitives_pow::count_leading_zero_bits(hash)
        }
    }
}

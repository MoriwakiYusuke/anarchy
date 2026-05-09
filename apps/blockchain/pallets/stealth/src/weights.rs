//! Weight definitions for the stealth pallet

use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight trait for the stealth pallet
pub trait WeightInfo {
    fn send_to_stealth() -> Weight;
    /// TSTS F2/F2.5/F10: claim_stealth_reward dedicated weight (Copilot review #3199031138).
    fn claim_stealth_reward() -> Weight;
}

/// Default weights implementation
impl WeightInfo for () {
    fn send_to_stealth() -> Weight {
        // Read: sender balance + ephemeral keys for current block
        // Write: sender balance + recipient balance + ephemeral keys
        Weight::from_parts(50_000_000, 0)
            .saturating_add(Weight::from_parts(0, 5_000))
    }

    fn claim_stealth_reward() -> Weight {
        // claim_stealth_reward is materially heavier than send_to_stealth:
        //   - ed25519 signature verify (sp_io host call, ~120k ref_time)
        //   - 1 read: RecipientReceiveCount (eph key)
        //   - 1 read: ClaimedReceiveCount (eph key)
        //   - 1 read: StealthRewardPool
        //   - 1 read: TotalReceivedCount
        //   - 1 read + 1 write: pallet_balances::Account (recipient mint)
        //   - 1 write: StealthRewardPool (deduct)
        //   - 1 write: ClaimedReceiveCount (advance)
        //   - 1 event emit
        //   - optional CorrespondenceVerifier::verify (no-op で 0)
        // 概算: 80M ref_time, 6 reads, 3 writes
        Weight::from_parts(80_000_000, 0)
            .saturating_add(Weight::from_parts(0, 6_000))
    }
}

/// Substrate weights implementation
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn send_to_stealth() -> Weight {
        // Read: sender balance + ephemeral keys for current block
        // Write: sender balance + recipient balance + ephemeral keys
        Weight::from_parts(50_000_000, 0)
            .saturating_add(Weight::from_parts(0, 5_000))
            // Database reads: sender balance + ephemeral keys
            .saturating_add(T::DbWeight::get().reads(2))
            // Database writes: sender balance + recipient balance + ephemeral keys
            .saturating_add(T::DbWeight::get().writes(3))
    }

    fn claim_stealth_reward() -> Weight {
        Weight::from_parts(80_000_000, 0)
            .saturating_add(Weight::from_parts(0, 6_000))
            // Reads: RecipientReceiveCount, ClaimedReceiveCount, StealthRewardPool,
            //        TotalReceivedCount, pallet_balances Account = 5 reads, +1 余裕
            .saturating_add(T::DbWeight::get().reads(6))
            // Writes: StealthRewardPool, ClaimedReceiveCount, pallet_balances Account
            .saturating_add(T::DbWeight::get().writes(3))
    }
}

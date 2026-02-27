//! Weight definitions for the stealth pallet

use frame_support::weights::Weight;

/// Weight trait for the stealth pallet
pub trait WeightInfo {
    fn send_to_stealth() -> Weight;
}

/// Default weights implementation
impl WeightInfo for () {
    fn send_to_stealth() -> Weight {
        // Read: sender balance + ephemeral keys for current block
        // Write: sender balance + recipient balance + ephemeral keys
        Weight::from_parts(50_000_000, 0)
            .saturating_add(Weight::from_parts(0, 5_000))
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
    }
}

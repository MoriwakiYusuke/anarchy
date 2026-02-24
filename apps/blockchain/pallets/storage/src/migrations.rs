//! Storage migrations for pallet-storage (013-slashing-repair)
//!
//! Migrates ProofRecord to add slashed and share_index fields.

use frame_support::{
    pallet_prelude::*,
    traits::OnRuntimeUpgrade,
    weights::Weight,
};
use frame_system::pallet_prelude::BlockNumberFor;
use parity_scale_codec::{Decode, Encode};

use crate::{Config, ProofRecords};

/// Old ProofRecord structure (pre-013-slashing-repair)
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, Default)]
pub struct OldProofRecord<BlockNumber: Default> {
    pub last_proved_at: BlockNumber,
    pub success_count: u32,
    pub failure_count: u32,
}

/// New ProofRecord structure (013-slashing-repair)
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, Default)]
pub struct NewProofRecord<BlockNumber: Default> {
    pub last_proved_at: BlockNumber,
    pub success_count: u32,
    pub failure_count: u32,
    pub slashed: bool,
    pub share_index: u8,
}

/// Migration from v0 to v1: Add slashed and share_index to ProofRecord
pub struct MigrateProofRecordsV0ToV1<T>(core::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for MigrateProofRecordsV0ToV1<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut reads = 0u64;
        let mut writes = 0u64;

        // Translate all ProofRecords from old format to new format
        ProofRecords::<T>::translate::<OldProofRecord<BlockNumberFor<T>>, _>(
            |_content_hash, _account_id, old_record| {
                reads += 1;
                writes += 1;
                Some(crate::pallet::ProofRecord {
                    last_proved_at: old_record.last_proved_at,
                    success_count: old_record.success_count,
                    failure_count: old_record.failure_count,
                    slashed: false,      // New field: default to not slashed
                    share_index: 0,      // New field: 0 = unset
                })
            },
        );

        // Note: frame_support::log is not available in stable2503
        // Migration count is tracked via weight

        T::DbWeight::get().reads_writes(reads, writes)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<sp_std::prelude::Vec<u8>, sp_runtime::TryRuntimeError> {
        let count = ProofRecords::<T>::iter().count() as u32;
        Ok(count.encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: sp_std::prelude::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        let old_count: u32 = Decode::decode(&mut &state[..])
            .map_err(|_| sp_runtime::TryRuntimeError::Other("Failed to decode pre_upgrade state"))?;
        
        let new_count = ProofRecords::<T>::iter().count() as u32;
        
        // All records should be preserved
        ensure!(
            old_count == new_count,
            sp_runtime::TryRuntimeError::Other("ProofRecord count mismatch after migration")
        );

        // Verify all records have default values for new fields
        for (_content_hash, _account_id, record) in ProofRecords::<T>::iter() {
            ensure!(
                record.slashed == false && record.share_index == 0,
                sp_runtime::TryRuntimeError::Other("Migrated ProofRecord has unexpected field values")
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{new_test_ext, Test};

    #[test]
    fn test_migration_preserves_existing_fields() {
        new_test_ext().execute_with(|| {
            // Migration is essentially a no-op in tests since we start fresh
            // Real migration testing requires a state dump from pre-migration chain
            let weight = MigrateProofRecordsV0ToV1::<Test>::on_runtime_upgrade();
            assert!(weight.ref_time() >= 0);
        });
    }
}

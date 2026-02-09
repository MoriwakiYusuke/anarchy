//! Benchmarking for pallet-storage
//!
//! These benchmarks measure the computational cost of each extrinsic
//! to determine appropriate weights.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    /// Benchmark for `register_fragment`
    /// 
    /// Complexity: O(1) - single storage write
    #[benchmark]
    fn register_fragment() {
        let caller: T::AccountId = whitelisted_caller();
        let fragment_id: FragmentId = [1u8; 32];
        let size: u32 = 1024;

        #[extrinsic_call]
        register_fragment(RawOrigin::Signed(caller), fragment_id, size);
    }

    /// Benchmark for `register_node`
    /// 
    /// Complexity: O(1) - two storage writes (StorageNodes + OperatorNodes)
    #[benchmark]
    fn register_node() {
        let caller: T::AccountId = whitelisted_caller();
        let peer_id: BoundedVec<u8, T::MaxPeerIdLen> = 
            vec![1u8; 52].try_into().expect("peer_id within bounds");
        let capacity: u64 = 10 * 1024 * 1024 * 1024; // 10GB

        #[extrinsic_call]
        register_node(RawOrigin::Signed(caller), peer_id, capacity);
    }

    /// Benchmark for `update_node`
    /// 
    /// Complexity: O(1) - single storage read + write
    #[benchmark]
    fn update_node() {
        let caller: T::AccountId = whitelisted_caller();
        let peer_id: BoundedVec<u8, T::MaxPeerIdLen> = 
            vec![2u8; 52].try_into().expect("peer_id within bounds");
        
        // Setup: register node first
        let _ = Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            peer_id.clone(),
            1024,
        );
        
        let new_capacity: u64 = 20 * 1024 * 1024 * 1024; // 20GB

        #[extrinsic_call]
        update_node(RawOrigin::Signed(caller), new_capacity);
    }

    /// Benchmark for `unregister_node`
    /// 
    /// Complexity: O(1) - storage reads + deletes
    #[benchmark]
    fn unregister_node() {
        let caller: T::AccountId = whitelisted_caller();
        let peer_id: BoundedVec<u8, T::MaxPeerIdLen> = 
            vec![3u8; 52].try_into().expect("peer_id within bounds");
        
        // Setup: register node first
        let _ = Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            peer_id.clone(),
            1024,
        );

        #[extrinsic_call]
        unregister_node(RawOrigin::Signed(caller));
    }

    /// Benchmark for `declare_holding`
    /// 
    /// Complexity: O(h) where h = number of existing holders for fragment
    #[benchmark]
    fn declare_holding() {
        let caller: T::AccountId = whitelisted_caller();
        let peer_id: BoundedVec<u8, T::MaxPeerIdLen> = 
            vec![4u8; 52].try_into().expect("peer_id within bounds");
        let fragment_id: FragmentId = [4u8; 32];
        
        // Setup: register node and fragment
        let _ = Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            peer_id.clone(),
            1024,
        );
        let _ = Pallet::<T>::register_fragment(
            RawOrigin::Signed(caller.clone()).into(),
            fragment_id,
            512,
        );

        #[extrinsic_call]
        declare_holding(RawOrigin::Signed(caller), fragment_id);
    }

    /// Benchmark for `revoke_holding`
    /// 
    /// Complexity: O(h + f) where h = holders, f = node's holdings
    #[benchmark]
    fn revoke_holding() {
        let caller: T::AccountId = whitelisted_caller();
        let peer_id: BoundedVec<u8, T::MaxPeerIdLen> = 
            vec![5u8; 52].try_into().expect("peer_id within bounds");
        let fragment_id: FragmentId = [5u8; 32];
        
        // Setup: register node, fragment, and holding
        let _ = Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            peer_id.clone(),
            1024,
        );
        let _ = Pallet::<T>::register_fragment(
            RawOrigin::Signed(caller.clone()).into(),
            fragment_id,
            512,
        );
        let _ = Pallet::<T>::declare_holding(
            RawOrigin::Signed(caller.clone()).into(),
            fragment_id,
        );

        #[extrinsic_call]
        revoke_holding(RawOrigin::Signed(caller), fragment_id);
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}

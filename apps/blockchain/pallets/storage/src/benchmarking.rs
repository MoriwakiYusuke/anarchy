//! Benchmarking for pallet-storage
//!
//! These benchmarks measure the computational cost of each extrinsic
//! to determine appropriate weights.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::{BoundedVec, traits::ConstU32};
use frame_system::RawOrigin;

/// Helper to create a valid http_url for benchmarks
fn make_http_url<T: Config>() -> BoundedVec<u8, T::MaxHttpUrlLen> {
    b"http://127.0.0.1:3030".to_vec().try_into().expect("http_url within bounds")
}

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
        let pow_nonce: u64 = 0; // Benchmarks skip PoW verification
        let http_url = make_http_url::<T>();

        #[extrinsic_call]
        register_node(RawOrigin::Signed(caller), peer_id, capacity, pow_nonce, http_url);
    }

    /// Benchmark for `update_node`
    /// 
    /// Complexity: O(1) - single storage read + write
    #[benchmark]
    fn update_node() {
        let caller: T::AccountId = whitelisted_caller();
        let peer_id: BoundedVec<u8, T::MaxPeerIdLen> = 
            vec![2u8; 52].try_into().expect("peer_id within bounds");
        let pow_nonce: u64 = 0;
        let http_url = make_http_url::<T>();
        
        // Setup: register node first
        let _ = Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            peer_id.clone(),
            1024,
            pow_nonce,
            http_url,
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
        let pow_nonce: u64 = 0;
        let http_url = make_http_url::<T>();
        
        // Setup: register node first
        let _ = Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            peer_id.clone(),
            1024,
            pow_nonce,
            http_url,
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
        let pow_nonce: u64 = 0;
        let http_url = make_http_url::<T>();
        
        // Setup: register node and fragment
        let _ = Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            peer_id.clone(),
            1024,
            pow_nonce,
            http_url,
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
        let pow_nonce: u64 = 0;
        let http_url = make_http_url::<T>();
        
        // Setup: register node, fragment, and holding
        let _ = Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            peer_id.clone(),
            1024,
            pow_nonce,
            http_url,
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

    /// Benchmark for `prove_holding_kzg` (T069: SC-002 - verify <10ms on-chain)
    /// 
    /// Success Criteria SC-002: KZG proof検証がオンチェーンで10ms未満
    /// 
    /// Complexity: O(1) - single KZG pairing verification
    #[benchmark]
    fn prove_holding_kzg() {
        let caller: T::AccountId = whitelisted_caller();
        let peer_id: BoundedVec<u8, T::MaxPeerIdLen> = 
            vec![6u8; 52].try_into().expect("peer_id within bounds");
        let pow_nonce: u64 = 0;
        let http_url = make_http_url::<T>();
        
        // Mock content hash and KZG fragment
        let content_hash: [u8; 32] = [6u8; 32];
        
        // Setup: register node
        let _ = Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            peer_id.clone(),
            1024,
            pow_nonce,
            http_url,
        );
        
        // Setup: register KZG fragment using valid G1 generator point
        // This is a valid BLS12-381 G1 point that can be deserialized on-chain
        let kzg_commitment: BoundedVec<u8, ConstU32<48>> = 
            crate::kzg::G1_GENERATOR_COMPRESSED.to_vec().try_into().expect("commitment within bounds");
        let compressed_size: u32 = 10 * 1024; // 10KB
        let fragment_count: u8 = 5;
        let threshold: u8 = 3;
        
        let _ = Pallet::<T>::register_kzg_fragment(
            RawOrigin::Signed(caller.clone()).into(),
            content_hash,
            kzg_commitment,
            compressed_size,
            fragment_count,
            threshold,
        );
        
        // SECURITY FIX: Add caller as holder (PR #22 CRITICAL-2)
        KzgFragments::<T>::mutate(content_hash, |maybe_fragment| {
            if let Some(ref mut fragment) = maybe_fragment {
                let _ = fragment.holders.try_push(caller.clone());
            }
        });
        
        // SECURITY FIX: Must issue challenge before proving (PR #22 CRITICAL-1)
        let share_index: u8 = 1;
        let _ = Pallet::<T>::issue_challenge(
            RawOrigin::Signed(caller.clone()).into(),
            content_hash,
            caller.clone(),
            share_index,
        );
        
        // Valid G1 proof data (G1 generator can be deserialized)
        // Note: Pairing check will fail as commitment/proof aren't mathematically consistent,
        // but this measures the full verification path including deserialization overhead.
        let proof: BoundedVec<u8, ConstU32<48>> = 
            crate::kzg::G1_GENERATOR_COMPRESSED.to_vec().try_into().expect("proof within bounds");
        let share_value: BoundedVec<u8, ConstU32<32>> = 
            vec![0u8; 32].try_into().expect("share_value within bounds");

        // Note: This benchmark measures the extrinsic call overhead.
        // Actual KZG verification will fail with mock data, but the benchmark
        // captures the weight of the call path.
        #[extrinsic_call]
        prove_holding_kzg(
            RawOrigin::Signed(caller),
            content_hash,
            share_index,
            share_value,
            proof,
        );
    }

    /// Benchmark for batch KZG verification (T078: SC-003 - 100-node <1s)
    /// 
    /// Success Criteria SC-003: 100ノードの一括検証が1秒未満
    /// 
    /// This benchmark simulates multiple sequential proof verifications
    /// to measure aggregate performance.
    #[benchmark]
    fn prove_holding_kzg_batch() {
        let caller: T::AccountId = whitelisted_caller();
        let peer_id: BoundedVec<u8, T::MaxPeerIdLen> = 
            vec![7u8; 52].try_into().expect("peer_id within bounds");
        let pow_nonce: u64 = 0;
        let http_url = make_http_url::<T>();
        
        // Setup: register node
        let _ = Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            peer_id.clone(),
            1024 * 1024 * 1024, // 1GB capacity
            pow_nonce,
            http_url,
        );
        
        // Prepare batch of KZG fragments (simulating 100 verifications)
        // Note: In production, this would be 100 separate transactions
        // Here we measure single verification and extrapolate
        let content_hash: [u8; 32] = [7u8; 32];
        let kzg_commitment: BoundedVec<u8, ConstU32<48>> = 
            crate::kzg::G1_GENERATOR_COMPRESSED.to_vec().try_into().expect("commitment within bounds");
        
        let _ = Pallet::<T>::register_kzg_fragment(
            RawOrigin::Signed(caller.clone()).into(),
            content_hash,
            kzg_commitment,
            50 * 1024, // 50KB per fragment
            5,
            3,
        );
        
        // SECURITY FIX: Add caller as holder (PR #22 CRITICAL-2)
        KzgFragments::<T>::mutate(content_hash, |maybe_fragment| {
            if let Some(ref mut fragment) = maybe_fragment {
                let _ = fragment.holders.try_push(caller.clone());
            }
        });
        
        // SECURITY FIX: Must issue challenge before proving (PR #22 CRITICAL-1)
        let share_index: u8 = 1;
        let _ = Pallet::<T>::issue_challenge(
            RawOrigin::Signed(caller.clone()).into(),
            content_hash,
            caller.clone(),
            share_index,
        );
        
        let proof: BoundedVec<u8, ConstU32<48>> = 
            crate::kzg::G1_GENERATOR_COMPRESSED.to_vec().try_into().expect("proof within bounds");
        let share_value: BoundedVec<u8, ConstU32<32>> = 
            vec![1u8; 32].try_into().expect("share_value within bounds");

        // Single verification - total time for 100 = this × 100
        // Target: < 10ms per verification → < 1s for 100
        #[extrinsic_call]
        prove_holding_kzg(
            RawOrigin::Signed(caller),
            content_hash,
            share_index,
            share_value,
            proof,
        );
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}

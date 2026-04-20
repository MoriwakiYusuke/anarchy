//! Weight definitions for `pallet-messaging`.
//!
//! Phase 2 では benchmarking からの自動生成ではなく、保守的なスタブ値を置く。
//! Phase 7 の polish (T082) で `frame_benchmarking` ベースの実測値に置き換える。

use frame_support::weights::Weight;

/// Weight trait for `pallet-messaging`.
pub trait WeightInfo {
    fn publish_dm_key() -> Weight;
    fn revoke_dm_key() -> Weight;
    /// `ciphertext_len` (byte) に応じた線形近似。
    fn send_dm(ciphertext_len: u32) -> Weight;
}

/// Default weights (`()` 実装)。benchmarking 未実装の間はこれを使う。
impl WeightInfo for () {
    fn publish_dm_key() -> Weight {
        Weight::from_parts(40_000_000, 0).saturating_add(Weight::from_parts(0, 3_000))
    }

    fn revoke_dm_key() -> Weight {
        Weight::from_parts(30_000_000, 0).saturating_add(Weight::from_parts(0, 2_000))
    }

    fn send_dm(ciphertext_len: u32) -> Weight {
        Weight::from_parts(80_000_000, 0)
            .saturating_add(Weight::from_parts((ciphertext_len as u64).saturating_mul(1_000), 0))
            .saturating_add(Weight::from_parts(0, 10_000))
    }
}

/// Substrate 向け weight 実装 (runtime から参照)。benchmarking 実装前は上の
/// `()` と同等の値を返す。
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn publish_dm_key() -> Weight {
        <() as WeightInfo>::publish_dm_key()
    }

    fn revoke_dm_key() -> Weight {
        <() as WeightInfo>::revoke_dm_key()
    }

    fn send_dm(ciphertext_len: u32) -> Weight {
        <() as WeightInfo>::send_dm(ciphertext_len)
    }
}

//! `sc_consensus_pow::PowAlgorithm<Block>` の RandomX 実装。
//!
//! Phase A では trait の構造提供のみ — service.rs からは未配線。
//! VM 状態の dataset 切替 (epoch) は Phase B / M11 でチューニング。

use std::sync::{Arc, Mutex};
use parity_scale_codec::{Decode, Encode};
use sc_client_api::HeaderBackend;
use sc_consensus_pow::{Error as PowError, PowAlgorithm};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderMetadata;
use sp_consensus_pow::Seal as RawSeal;
use sp_core::{H256, U256};
use sp_runtime::generic::BlockId;
use sp_runtime::traits::Block as BlockT;

use super::difficulty::DifficultyClient;

/// PoW seal (nonce + work hash payload)。
#[derive(Clone, Encode, Decode, Debug)]
pub struct PowSeal {
    pub nonce: u64,
    pub work: H256,
}

/// RandomX seed の epoch 長 (block 数)。spec §5.4 で 2048 推奨。
pub const RANDOMX_EPOCH_BLOCKS: u32 = 2048;

/// VM cache wrapper (Phase A では未使用、Phase B で実装)。
#[derive(Default)]
pub struct RandomXVm {
    _marker: (),
}

pub struct RandomXAlgorithm<B: BlockT, C> {
    diff_client: DifficultyClient<C>,
    _vm: Arc<Mutex<RandomXVm>>,
    _phantom: std::marker::PhantomData<B>,
}

impl<B: BlockT, C> Clone for RandomXAlgorithm<B, C> {
    fn clone(&self) -> Self {
        Self {
            diff_client: DifficultyClient::new(self.diff_client.client_arc()),
            _vm: Arc::clone(&self._vm),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<B: BlockT, C> RandomXAlgorithm<B, C>
where
    B: BlockT<Hash = H256>,
    C: HeaderBackend<B> + HeaderMetadata<B> + ProvideRuntimeApi<B> + Send + Sync + 'static,
    C::Api: pallet_difficulty::DifficultyApi<B>,
{
    pub fn new(client: Arc<C>) -> Self {
        Self {
            diff_client: DifficultyClient::new(client),
            _vm: Arc::new(Mutex::new(RandomXVm::default())),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<B, C> PowAlgorithm<B> for RandomXAlgorithm<B, C>
where
    B: BlockT<Hash = H256>,
    C: HeaderBackend<B> + HeaderMetadata<B, Error = sp_blockchain::Error>
        + ProvideRuntimeApi<B> + Send + Sync + 'static,
    C::Api: pallet_difficulty::DifficultyApi<B>,
{
    type Difficulty = U256;

    fn difficulty(&self, parent: B::Hash) -> Result<Self::Difficulty, PowError<B>> {
        self.diff_client
            .difficulty_at(parent)
            .map_err(|e| PowError::Environment(format!("difficulty api: {:?}", e)))
    }

    fn verify(
        &self,
        _parent: &BlockId<B>,
        pre_hash: &H256,
        _pre_digest: Option<&[u8]>,
        seal: &RawSeal,
        difficulty: Self::Difficulty,
    ) -> Result<bool, PowError<B>> {
        let seal = PowSeal::decode(&mut seal.as_slice())
            .map_err(|e| PowError::Other(format!("seal decode: {:?}", e)))?;

        // Phase A 用 stub: 実際の RandomX hash は Phase B で実装する。
        let _ = (pre_hash, seal.nonce, seal.work, difficulty);
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parity_scale_codec::Encode;

    #[test]
    fn pow_seal_roundtrip() {
        let seal = PowSeal { nonce: 12345, work: H256::from([0xab; 32]) };
        let encoded = seal.encode();
        let decoded = PowSeal::decode(&mut encoded.as_slice()).expect("decode");
        assert_eq!(decoded.nonce, 12345);
        assert_eq!(decoded.work, H256::from([0xab; 32]));
    }
}

//! `sc_consensus_pow::PowAlgorithm<Block>` の RandomX 実装。
//!
//! Phase B: RandomX hash による実際の verify を実装。
//! genesis hash を seed key として使用 (epoch ベースのローテーションは Phase C)。
//! VM 状態の dataset 切替 (epoch) は Phase C / M11 でチューニング。

use std::sync::{Arc, Mutex};
use parity_scale_codec::{Decode, Encode};
use randomx_rs::{RandomXCache, RandomXDataset, RandomXFlag, RandomXVM};
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
/// 将来 epoch ローテーションを実装するときに使う想定 (現状は genesis hash 固定)。
#[allow(dead_code)]
pub const RANDOMX_EPOCH_BLOCKS: u32 = 2048;

/// RandomX VM ラッパ。
///
/// `RandomXCache` と `RandomXVM` を保持し、seed key 変更時に全体を再構築する。
/// light mode (256 MB) をデフォルトとし、`--randomx-mode full` フラグで
/// full mode (2 GB) に切り替え可能 (service.rs 経由、Phase C で追加予定)。
pub struct RandomXVm {
    /// 現在の seed key (epoch 先頭ブロックのハッシュなど)。
    current_key: Vec<u8>,
    /// full mode かどうか。
    full_mode: bool,
    /// RandomX VM 本体。内部で cache / dataset を保持している。
    vm: RandomXVM,
}

impl RandomXVm {
    /// 新しい RandomXVm を構築する。
    ///
    /// light mode: `full_mode = false` — cache 256 MB、dataset 不要。
    /// full mode:  `full_mode = true`  — dataset 2 GB、高速だがメモリ大。
    pub fn new(key: &[u8], full_mode: bool) -> Result<Self, String> {
        let flags = RandomXFlag::get_recommended_flags();

        if full_mode {
            let full_flags = flags | RandomXFlag::FLAG_FULL_MEM;
            // full mode: cache を dataset 構築に使い、VM には dataset のみ渡す。
            let cache = RandomXCache::new(full_flags, key)
                .map_err(|e| format!("randomx cache (full): {:?}", e))?;
            let dataset = RandomXDataset::new(full_flags, cache, 0)
                .map_err(|e| format!("randomx dataset: {:?}", e))?;
            let vm = RandomXVM::new(full_flags, None, Some(dataset))
                .map_err(|e| format!("randomx vm (full): {:?}", e))?;
            Ok(Self { current_key: key.to_vec(), full_mode, vm })
        } else {
            // light mode: cache のみで VM を構築する。
            let cache = RandomXCache::new(flags, key)
                .map_err(|e| format!("randomx cache (light): {:?}", e))?;
            let vm = RandomXVM::new(flags, Some(cache), None)
                .map_err(|e| format!("randomx vm (light): {:?}", e))?;
            Ok(Self { current_key: key.to_vec(), full_mode, vm })
        }
    }

    /// seed key が変わっていれば VM を再構築する。
    ///
    /// RandomX は in-place での seed 交換をサポートしないため、
    /// key 変更時は VM オブジェクト全体を再生成する。
    pub fn ensure_key(&mut self, key: &[u8]) -> Result<(), String> {
        if self.current_key.as_slice() == key {
            return Ok(());
        }
        let full_mode = self.full_mode;
        *self = Self::new(key, full_mode)?;
        Ok(())
    }

    /// RandomX ハッシュを計算して返す。エラー時は空 Vec を返す。
    pub fn hash(&self, input: &[u8]) -> Vec<u8> {
        self.vm.calculate_hash(input).unwrap_or_default()
    }
}

impl Default for RandomXVm {
    /// genesis-time のプレースホルダ。
    /// `ensure_key()` が最初の verify 呼び出し前に正しい key に差し替える。
    fn default() -> Self {
        Self::new(&[1u8; 32], false).expect("randomx default init failed")
    }
}

// RandomX の C ライブラリ実装はスレッドセーフ (並列 VM はそれぞれ独立した状態を持つ)。
// RandomXVm は Mutex<RandomXVm> でラップされるため、同時アクセスは排他制御される。
// 生ポインタを持つ RandomXVM が自動導出できないため明示的に宣言する。
// Safety: RandomX の各 VM オブジェクトは独立したメモリ領域を持ち、
//         Mutex によりシングルスレッドアクセスが保証されるためスレッドセーフ。
unsafe impl Send for RandomXVm {}
unsafe impl Sync for RandomXVm {}

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
    /// `full_mode` が `true` のとき RandomX full dataset (2 GB) を使用する。
    /// `false` (デフォルト) は light mode (256 MB cache)。
    pub fn new(client: Arc<C>, full_mode: bool) -> Self {
        let vm = RandomXVm::new(&[0u8; 32], full_mode)
            .expect("randomx initial init failed");
        Self {
            diff_client: DifficultyClient::new(client),
            _vm: Arc::new(Mutex::new(vm)),
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

        // RandomX seed key — Phase B では genesis hash を使用する。
        // epoch ベースのローテーション (RANDOMX_EPOCH_BLOCKS ごとに先頭ブロックハッシュへ変更)
        // は Phase C でのチューニング時に追加する予定。
        let seed_bytes = self.diff_client.client_arc().info().genesis_hash.as_ref().to_vec();

        // input = pre_hash || nonce (little-endian u64)
        let mut input = Vec::with_capacity(32 + 8);
        input.extend_from_slice(pre_hash.as_bytes());
        input.extend_from_slice(&seal.nonce.to_le_bytes());

        let mut vm = self._vm.lock()
            .map_err(|_| PowError::Other("randomx vm lock poisoned".into()))?;
        vm.ensure_key(&seed_bytes)
            .map_err(|e| PowError::Other(format!("randomx ensure_key: {}", e)))?;
        let work_hash_bytes = vm.hash(&input);

        if work_hash_bytes.is_empty() {
            return Err(PowError::Other("randomx hash returned empty result".into()));
        }

        if difficulty.is_zero() {
            return Ok(false);
        }

        // Bitcoin スタイルのターゲットチェック: work_hash <= U256::MAX / difficulty
        let work_hash = U256::from_big_endian(&work_hash_bytes);
        let target = U256::MAX / difficulty;
        Ok(work_hash <= target)
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

    #[test]
    fn randomx_hash_is_deterministic() {
        let key = b"phase-b-test-key-32-bytes-padded";
        let vm1 = RandomXVm::new(key, false).expect("init1");
        let vm2 = RandomXVm::new(key, false).expect("init2");
        let input = b"determinism check input";
        let h1 = vm1.hash(input);
        let h2 = vm2.hash(input);
        assert_eq!(h1, h2, "RandomX は同じ key では決定論的であるべき");
        assert_eq!(h1.len(), 32, "RandomX ハッシュ出力は 32 バイトであるべき");
    }

    #[test]
    fn randomx_hash_changes_with_input() {
        let key = b"phase-b-test-key-32-bytes-padded";
        let vm = RandomXVm::new(key, false).expect("init");
        let h1 = vm.hash(b"input one");
        let h2 = vm.hash(b"input two");
        assert_ne!(h1, h2, "異なる input は異なるハッシュを生成するべき");
    }
}

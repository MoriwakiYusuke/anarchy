//! RPC拡張

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use rand::seq::SliceRandom;

use anarchy_runtime::{opaque::Block, AccountId, Balance, Nonce};
use jsonrpsee::RpcModule;
use pallet_post::PostApi as PostRuntimeApi;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::{Error as BlockchainError, HeaderBackend, HeaderMetadata};

pub mod endpoint_policy;
pub mod storage;

pub use endpoint_policy::EndpointPolicy;
pub use storage::{StorageApiServer, Storage, RegisteredStorageNode};

/// Storage Nodeの共有状態（マルチノード対応）
pub type SharedStorageNodes = Arc<RwLock<StorageNodeRegistry>>;

/// Storage Nodeレジストリ
/// 
/// - ノードリスト取得時はランダム順（プライバシー保護）
/// - 断片配置はmerkle_rootベースでPost毎にランダム化（分散と追跡防止）
/// - Issue 7 fix: Maximum registry size enforced with LRU eviction
#[derive(Debug, Default)]
pub struct StorageNodeRegistry {
    /// 登録されたノード一覧
    pub nodes: Vec<RegisteredStorageNode>,
    /// Maximum registry size (Issue 7 fix)
    pub max_size: usize,
    /// (#28-CRIT-3) Per-operator last successful register timestamp (unix secs).
    /// Used by `register_endpoint` to enforce a minimum interval between
    /// re-registration attempts and prevent registry-churn DoS.
    pub last_register_per_operator: HashMap<[u8; 32], u64>,
}

/// Default maximum registry size (Issue 7 fix - DoS prevention)
pub const DEFAULT_MAX_REGISTRY_SIZE: usize = 10_000;

/// (#28-CRIT-3) Minimum seconds between successive register_endpoint calls
/// from the same operator. Set lower than the timestamp-skew window so that
/// honest restarts always succeed but a single key can't churn the registry.
pub const MIN_REREGISTER_INTERVAL_SECS: u64 = 30;

/// (finding #3) `last_register_per_operator` のハードキャップ。
/// オペレーター鍵は自由に生成できるため、無制限に成長すると
/// メモリ枯渇 DoS になる。窓 (MIN_REREGISTER_INTERVAL_SECS) より古い
/// エントリの prune に加え、この上限を超えたら最古エントリを退去する。
pub const MAX_RATE_LIMIT_ENTRIES: usize = 10_000;

impl StorageNodeRegistry {
    /// 新しいレジストリを作成
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            max_size: DEFAULT_MAX_REGISTRY_SIZE,
            last_register_per_operator: HashMap::new(),
        }
    }

    /// Create registry with custom max size (for testing)
    #[allow(dead_code)]
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            nodes: Vec::new(),
            max_size,
            last_register_per_operator: HashMap::new(),
        }
    }

    /// (#28-CRIT-3) Returns Err if the operator registered within the last
    /// `MIN_REREGISTER_INTERVAL_SECS` seconds. On success, records `now` as the
    /// new last-register time for the operator.
    ///
    /// (finding #3) 挿入のたびに rate-limit 窓より古いエントリを prune し、
    /// さらに `MAX_RATE_LIMIT_ENTRIES` を超える場合は最古エントリを退去する。
    /// これにより、使い捨てオペレーター鍵による無制限なメモリ増加を防ぐ。
    pub fn check_and_record_operator_rate(
        &mut self,
        operator: [u8; 32],
        now: u64,
    ) -> Result<(), u64> {
        // 窓より古いエントリは rate limit に影響しないため捨てる
        self.last_register_per_operator
            .retain(|_, &mut last| now.saturating_sub(last) < MIN_REREGISTER_INTERVAL_SECS);

        if let Some(&last) = self.last_register_per_operator.get(&operator) {
            let elapsed = now.saturating_sub(last);
            if elapsed < MIN_REREGISTER_INTERVAL_SECS {
                return Err(MIN_REREGISTER_INTERVAL_SECS - elapsed);
            }
        }

        // prune 後もキャップ超過なら最古エントリを退去 (nodes Vec の LRU と同パターン)
        while self.last_register_per_operator.len() >= MAX_RATE_LIMIT_ENTRIES {
            if let Some(oldest_key) = self
                .last_register_per_operator
                .iter()
                .min_by_key(|(_, &ts)| ts)
                .map(|(k, _)| *k)
            {
                self.last_register_per_operator.remove(&oldest_key);
            } else {
                break;
            }
        }

        self.last_register_per_operator.insert(operator, now);
        Ok(())
    }
    
    /// ノードを登録（重複チェック付き）
    /// Issue 7 fix: Evicts oldest node if registry is full
    pub fn register(&mut self, node: RegisteredStorageNode) -> bool {
        if self.nodes.iter().any(|n| n.endpoint == node.endpoint) {
            return false;
        }
        
        // Issue 7 fix: Check registry size and evict oldest if full
        if self.nodes.len() >= self.max_size {
            // Find oldest node (by registered_at)
            if let Some(oldest_idx) = self.nodes
                .iter()
                .enumerate()
                .min_by_key(|(_, n)| n.registered_at)
                .map(|(idx, _)| idx)
            {
                log::debug!(
                    "Registry full ({}/{}), evicting oldest node: {}",
                    self.nodes.len(),
                    self.max_size,
                    self.nodes[oldest_idx].endpoint
                );
                self.nodes.remove(oldest_idx);
            }
        }
        
        self.nodes.push(node);
        true
    }
    
    /// オンラインノード数を取得
    pub fn online_node_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_online).count()
    }
    
    /// オンラインノードを取得 (FR-105: オフラインノード除外)
    pub fn online_nodes(&self) -> Vec<&RegisteredStorageNode> {
        self.nodes.iter().filter(|n| n.is_online).collect()
    }
    
    /// ランダムで並び替えたオンラインノードを取得（プライバシー優先）
    pub fn online_nodes_shuffled(&self) -> Vec<RegisteredStorageNode> {
        let mut online: Vec<_> = self.nodes.iter()
            .filter(|n| n.is_online)
            .cloned()
            .collect();
        let mut rng = rand::thread_rng();
        online.shuffle(&mut rng);
        online
    }
    
    /// 断片配置をPost単位でランダム化（プライバシー保護）
    /// 
    /// merkle_rootをシードに使用することで:
    /// - 同一Postの断片は異なるノードに分散
    /// - 異なるPostは異なる配置パターン
    /// - merkle_rootがわからない限りパターン予測困難
    pub fn select_node_for_fragment(&self, merkle_root: &[u8; 32], fragment_index: usize) -> Option<&RegisteredStorageNode> {
        let online = self.online_nodes();
        if online.is_empty() {
            return None;
        }
        // merkle_rootから64bitシードを取得
        // SAFETY: merkle_root は [u8; 32] なので [..8] は常に 8 バイト (try_into は infallible)
        let seed = u64::from_le_bytes(
            merkle_root[..8]
                .try_into()
                .expect("invariant: merkle_root[..8] is always 8 bytes (merkle_root is [u8; 32])"),
        );
        // シードと断片インデックスを組み合わせてノード選択
        let node_index = (seed as usize).wrapping_add(fragment_index) % online.len();
        Some(online[node_index])
    }
}

/// 共有Storage Nodeレジストリを作成
pub fn create_shared_storage_nodes() -> SharedStorageNodes {
    Arc::new(RwLock::new(StorageNodeRegistry::new()))
}

/// フルRPC拡張のための依存関係
pub struct FullDeps<C, P> {
    /// クライアント
    pub client: Arc<C>,
    /// トランザクションプール
    pub pool: Arc<P>,
    /// Storage Nodeレジストリ (全接続で共有)
    pub storage_nodes: SharedStorageNodes,
    /// Gossipハンドル (ノード登録のブロードキャスト用)
    pub gossip_handle: crate::gossip::StorageNodeGossipHandle,
    /// チェーンノードSr25519キーペア (X-Chain-Auth署名用, Optionで互換性維持)
    pub chain_keypair: Option<storage::SharedChainKeyPair>,
    /// エンドポイント URL 検証ポリシー (SSRF 対策, finding #1)
    pub endpoint_policy: EndpointPolicy,
}

/// フルRPC拡張をインスタンス化
pub fn create_full<C, P>(
    deps: FullDeps<C, P>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockchainError> + 'static,
    C: Send + Sync + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: sp_block_builder::BlockBuilder<Block>,
    C::Api: PostRuntimeApi<Block>,
    C::Api: pallet_storage::StorageApi<Block>,
    P: TransactionPool + 'static,
{
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};

    let mut module = RpcModule::new(());
    let FullDeps { client, pool, storage_nodes, gossip_handle, chain_keypair, endpoint_policy } = deps;

    module.merge(System::new(client.clone(), pool).into_rpc())?;
    module.merge(TransactionPayment::new(client.clone()).into_rpc())?;

    // Storage RPC (T034: StorageApi登録)
    // Storage Nodeは起動時に自動登録される (storage_registerEndpoint RPC)
    // マルチノード対応：複数ノードを登録し、断片を分散配置
    let storage_rpc = match chain_keypair {
        Some(kp) => Storage::new_with_chain_auth(client, storage_nodes, gossip_handle, kp, endpoint_policy),
        None => Storage::new(client, storage_nodes, gossip_handle, endpoint_policy),
    };
    module.merge(storage_rpc.into_rpc())?;

    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T031: test_registry_size_limit
    /// Verifies that registry enforces max size with LRU eviction
    #[test]
    fn test_registry_size_limit() {
        // Create registry with small max size for testing
        let mut registry = StorageNodeRegistry::with_max_size(3);

        // Register 3 nodes (at capacity)
        for i in 0..3 {
            let node = RegisteredStorageNode {
                endpoint: format!("http://node{}:3030", i),
                node_id: None,
                registered_at: 1000 + i as u64, // Increasing timestamps
                is_online: true,
                last_health_check: 1000 + i as u64,
                latency_ms: None,
                registration_proof: None,
            };
            assert!(registry.register(node), "Node {} should register", i);
        }
        assert_eq!(registry.nodes.len(), 3);

        // Register 4th node - should trigger LRU eviction
        let new_node = RegisteredStorageNode {
            endpoint: "http://node3:3030".to_string(),
            node_id: None,
            registered_at: 2000, // Newest
            is_online: true,
            last_health_check: 2000,
            latency_ms: None,
            registration_proof: None,
        };
        assert!(registry.register(new_node));

        // Should still be at max size
        assert_eq!(registry.nodes.len(), 3);

        // Oldest node (node0) should be evicted
        assert!(!registry.nodes.iter().any(|n| n.endpoint == "http://node0:3030"),
            "Oldest node should be evicted");

        // Newest node should be present
        assert!(registry.nodes.iter().any(|n| n.endpoint == "http://node3:3030"),
            "New node should be registered");
    }

    /// T030: test_connection_limit_constant
    /// Verifies that MAX_CONNECTIONS constant is defined correctly
    #[test]
    fn test_connection_limit_constant() {
        // This test verifies the constant exists and has expected value
        // Actual connection limiting is tested in integration tests
        use crate::gossip::MAX_CONNECTIONS;
        assert_eq!(MAX_CONNECTIONS, 128);
    }

    /// Test duplicate node rejection
    #[test]
    fn test_duplicate_node_rejected() {
        let mut registry = StorageNodeRegistry::new();

        let node = RegisteredStorageNode {
            endpoint: "http://node1:3030".to_string(),
            node_id: None,
            registered_at: 1000,
            is_online: true,
            last_health_check: 1000,
            latency_ms: None,
            registration_proof: None,
        };

        // First registration should succeed
        assert!(registry.register(node.clone()));
        
        // Duplicate should be rejected
        assert!(!registry.register(node));
        
        // Still only one node
        assert_eq!(registry.nodes.len(), 1);
    }

    /// (finding #3) rate-limit map が無制限に成長しないことを検証
    #[test]
    fn test_rate_limit_map_bounded() {
        let mut registry = StorageNodeRegistry::new();
        let now = 1_000_000u64;

        // 窓内に大量の異なるオペレーターを登録してもハードキャップを超えない
        for i in 0..(MAX_RATE_LIMIT_ENTRIES + 500) {
            let mut op = [0u8; 32];
            op[..8].copy_from_slice(&(i as u64).to_le_bytes());
            // 全て同一時刻 (= 窓内) で登録
            let _ = registry.check_and_record_operator_rate(op, now);
        }
        assert!(
            registry.last_register_per_operator.len() <= MAX_RATE_LIMIT_ENTRIES,
            "rate-limit map must be capped: {} > {}",
            registry.last_register_per_operator.len(),
            MAX_RATE_LIMIT_ENTRIES
        );

        // 窓より古いエントリは次の挿入時に prune される
        let later = now + MIN_REREGISTER_INTERVAL_SECS + 1;
        let _ = registry.check_and_record_operator_rate([0xAA; 32], later);
        assert_eq!(
            registry.last_register_per_operator.len(),
            1,
            "stale entries should be pruned on insert"
        );

        // rate limit 自体は引き続き機能する
        assert!(registry.check_and_record_operator_rate([0xAA; 32], later + 1).is_err());
        assert!(registry
            .check_and_record_operator_rate([0xAA; 32], later + MIN_REREGISTER_INTERVAL_SECS)
            .is_ok());
    }

    /// Test default max size
    #[test]
    fn test_default_max_registry_size() {
        assert_eq!(DEFAULT_MAX_REGISTRY_SIZE, 10_000);
        
        let registry = StorageNodeRegistry::new();
        assert_eq!(registry.max_size, DEFAULT_MAX_REGISTRY_SIZE);
    }
}

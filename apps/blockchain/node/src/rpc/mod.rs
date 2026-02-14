//! RPC拡張

use std::sync::Arc;
use tokio::sync::RwLock;
use rand::seq::SliceRandom;

use anarchy_runtime::{opaque::Block, AccountId, Balance, Nonce};
use jsonrpsee::RpcModule;
use pallet_post::PostApi as PostRuntimeApi;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::{Error as BlockchainError, HeaderBackend, HeaderMetadata};

pub mod storage;

pub use storage::{StorageApiServer, Storage, RegisteredStorageNode};

/// ノード選択方式 (FR-101)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionStrategy {
    /// ランダム選択（プライバシー優先、デフォルト）(FR-102)
    #[default]
    Random,
    /// ラウンドロビン選択（均等配置）
    RoundRobin,
    /// 最寄りノード選択（低遅延優先）(FR-104)
    Nearest,
}

/// Storage Nodeの共有状態（マルチノード対応）
pub type SharedStorageNodes = Arc<RwLock<StorageNodeRegistry>>;

/// Storage Nodeレジストリ
#[derive(Debug, Default)]
pub struct StorageNodeRegistry {
    /// 登録されたノード一覧
    pub nodes: Vec<RegisteredStorageNode>,
    /// ラウンドロビン用インデックス
    pub round_robin_index: usize,
    /// ノード選択方式 (FR-101)
    pub selection_strategy: SelectionStrategy,
}

impl StorageNodeRegistry {
    /// 新しいレジストリを作成
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            round_robin_index: 0,
            selection_strategy: SelectionStrategy::Random, // FR-102: デフォルトはランダム
        }
    }
    
    /// ノードを登録（重複チェック付き）
    pub fn register(&mut self, node: RegisteredStorageNode) -> bool {
        if self.nodes.iter().any(|n| n.endpoint == node.endpoint) {
            return false;
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
    
    /// 選択方式を設定
    pub fn set_selection_strategy(&mut self, strategy: SelectionStrategy) {
        self.selection_strategy = strategy;
    }
    
    /// 現在の選択方式を取得
    pub fn get_selection_strategy(&self) -> SelectionStrategy {
        self.selection_strategy
    }
    
    /// 現在の選択方式に基づいてノードを選択
    pub fn select_node(&mut self) -> Option<RegisteredStorageNode> {
        match self.selection_strategy {
            SelectionStrategy::Random => self.next_node_random(),
            SelectionStrategy::RoundRobin => self.next_node_round_robin(),
            SelectionStrategy::Nearest => self.next_node_nearest(),
        }
    }
    
    /// ランダムで次のノードを選択 (FR-101, FR-102)
    pub fn next_node_random(&self) -> Option<RegisteredStorageNode> {
        let online: Vec<_> = self.nodes.iter().filter(|n| n.is_online).collect();
        if online.is_empty() {
            return None;
        }
        let mut rng = rand::thread_rng();
        online.choose(&mut rng).map(|&n| n.clone())
    }
    
    /// ラウンドロビンで次のノードを選択
    pub fn next_node_round_robin(&mut self) -> Option<RegisteredStorageNode> {
        let count = self.online_node_count();
        if count == 0 {
            return None;
        }
        let index = self.round_robin_index % count;
        self.round_robin_index = self.round_robin_index.wrapping_add(1);
        
        // インデックスを更新した後に再度取得（借用問題を回避）
        self.nodes.iter()
            .filter(|n| n.is_online)
            .nth(index)
            .cloned()
    }
    
    /// 最寄りノードを選択 (FR-104: 遅延の小さいノードを優先)
    pub fn next_node_nearest(&self) -> Option<RegisteredStorageNode> {
        let online: Vec<_> = self.nodes.iter().filter(|n| n.is_online).collect();
        if online.is_empty() {
            return None;
        }
        // latency_ms が最小のノードを選択
        online.iter()
            .min_by_key(|n| n.latency_ms.unwrap_or(u64::MAX))
            .map(|&n| n.clone())
    }
    
    /// 断片インデックスに基づいてノードを選択（分散配置）
    pub fn select_node_for_fragment(&self, fragment_index: usize) -> Option<&RegisteredStorageNode> {
        let online = self.online_nodes();
        if online.is_empty() {
            return None;
        }
        // 断片インデックスに基づいて異なるノードを選択
        let node_index = fragment_index % online.len();
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
    let FullDeps { client, pool, storage_nodes, gossip_handle } = deps;

    module.merge(System::new(client.clone(), pool).into_rpc())?;
    module.merge(TransactionPayment::new(client.clone()).into_rpc())?;

    // Storage RPC (T034: StorageApi登録)
    // Storage Nodeは起動時に自動登録される (storage_registerEndpoint RPC)
    // マルチノード対応：複数ノードを登録し、断片を分散配置
    module.merge(Storage::new(client, storage_nodes, gossip_handle).into_rpc())?;

    Ok(module)
}

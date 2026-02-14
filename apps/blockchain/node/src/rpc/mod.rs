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

/// Storage Nodeの共有状態（マルチノード対応）
pub type SharedStorageNodes = Arc<RwLock<StorageNodeRegistry>>;

/// Storage Nodeレジストリ
/// 
/// プライバシー保護のため、ノード選択は常にランダム
#[derive(Debug, Default)]
pub struct StorageNodeRegistry {
    /// 登録されたノード一覧
    pub nodes: Vec<RegisteredStorageNode>,
}

impl StorageNodeRegistry {
    /// 新しいレジストリを作成
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
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
    
    /// 断片インデックスに基づいてノードを選択（分散配置）
    pub fn select_node_for_fragment(&self, fragment_index: usize) -> Option<&RegisteredStorageNode> {
        let online = self.online_nodes();
        if online.is_empty() {
            return None;
        }
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

//! RPC拡張

use std::sync::Arc;
use tokio::sync::RwLock;

use anarchy_runtime::{opaque::Block, AccountId, Balance, Nonce};
use jsonrpsee::RpcModule;
use pallet_post::PostApi as PostRuntimeApi;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::{Error as BlockchainError, HeaderBackend, HeaderMetadata};

pub mod storage;

pub use storage::{StorageApiServer, Storage};

/// Storage Node URLの共有状態
pub type SharedStorageUrl = Arc<RwLock<Option<String>>>;

/// 共有Storage URLを作成
pub fn create_shared_storage_url() -> SharedStorageUrl {
    Arc::new(RwLock::new(None))
}

/// フルRPC拡張のための依存関係
pub struct FullDeps<C, P> {
    /// クライアント
    pub client: Arc<C>,
    /// トランザクションプール
    pub pool: Arc<P>,
    /// Storage Node URL (全接続で共有)
    pub storage_node_url: SharedStorageUrl,
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
    P: TransactionPool + 'static,
{
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};

    let mut module = RpcModule::new(());
    let FullDeps { client, pool, storage_node_url } = deps;

    module.merge(System::new(client.clone(), pool).into_rpc())?;
    module.merge(TransactionPayment::new(client.clone()).into_rpc())?;

    // Storage RPC (T034: StorageApi登録)
    // Storage Nodeは起動時に自動登録される (storage_registerEndpoint RPC)
    // URLは全接続で共有される
    module.merge(Storage::new(client, storage_node_url).into_rpc())?;

    Ok(module)
}

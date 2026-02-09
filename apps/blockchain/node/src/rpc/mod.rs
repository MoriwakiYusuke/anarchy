//! RPC拡張

use std::sync::Arc;

use anarchy_runtime::{opaque::Block, AccountId, Balance, Nonce};
use jsonrpsee::RpcModule;
use pallet_post::PostApi as PostRuntimeApi;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::{Error as BlockchainError, HeaderBackend, HeaderMetadata};

pub mod storage;

pub use storage::{StorageApiServer, Storage};

/// フルRPC拡張のための依存関係
pub struct FullDeps<C, P> {
    /// クライアント
    pub client: Arc<C>,
    /// トランザクションプール
    pub pool: Arc<P>,
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
    let FullDeps { client, pool } = deps;

    module.merge(System::new(client.clone(), pool).into_rpc())?;
    module.merge(TransactionPayment::new(client.clone()).into_rpc())?;

    // Storage RPC (T034: StorageApi登録)
    // Storage Nodeは起動時に自動登録される (storage_registerEndpoint RPC)
    module.merge(Storage::new(client).into_rpc())?;

    Ok(module)
}

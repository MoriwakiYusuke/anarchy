//! Runtime API `pallet_difficulty::DifficultyApi` への client 経由アクセスラッパ。

use std::sync::Arc;
use sc_client_api::HeaderBackend;
use pallet_difficulty::DifficultyApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderMetadata;
use sp_core::U256;
use sp_runtime::traits::Block as BlockT;

pub struct DifficultyClient<C> {
    pub(crate) client: Arc<C>,
}

impl<C> DifficultyClient<C> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client }
    }

    pub(crate) fn client_arc(&self) -> Arc<C> {
        Arc::clone(&self.client)
    }

    pub fn difficulty_at<B>(&self, parent: B::Hash) -> Result<U256, sp_api::ApiError>
    where
        B: BlockT,
        C: HeaderBackend<B> + HeaderMetadata<B> + ProvideRuntimeApi<B> + Send + Sync + 'static,
        C::Api: DifficultyApi<B>,
    {
        self.client.runtime_api().difficulty(parent)
    }
}

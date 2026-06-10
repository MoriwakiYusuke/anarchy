//! Chain interaction module
//!
//! Handles communication with the Anarchy blockchain via RPC.
//! Uses subxt for type-safe chain interaction.

pub mod failover;
pub mod signer;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use anyhow::{Result, bail, Context};
use tracing::{info, warn, debug};

use crate::network::endpoint_cache::{BlockchainEndpoint, EndpointCache};
use crate::storage::FragmentId;
use failover::FailoverManager;
pub use signer::Signer;

// subxt imports
use subxt::{OnlineClient, SubstrateConfig};
use subxt::dynamic::Value;
use subxt_signer::sr25519::Keypair as SubxtKeypair;

/// extrinsic 提出〜finalize 待ちに適用する上限時間（秒）。
/// チェーンが停止・無応答でも main イベントループの select! アームが
/// 無期限に固まらないようにするための保険。
const EXTRINSIC_TIMEOUT_SECS: u64 = 120;

/// Rate limiter for declare_holding calls (FR-108)
pub struct RateLimiter {
    /// Maximum calls per period
    max_calls: u32,
    /// Period duration
    period: Duration,
    /// Call timestamps within current period
    call_times: Mutex<Vec<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(max_calls_per_minute: u32) -> Self {
        Self {
            max_calls: max_calls_per_minute,
            period: Duration::from_secs(60),
            call_times: Mutex::new(Vec::new()),
        }
    }

    /// Check if a call is allowed and record it
    pub async fn try_acquire(&self) -> bool {
        let mut times = self.call_times.lock().await;
        let now = Instant::now();
        
        // Remove expired entries
        times.retain(|t| now.duration_since(*t) < self.period);
        
        if times.len() >= self.max_calls as usize {
            return false;
        }
        
        times.push(now);
        true
    }

    /// Get remaining quota
    pub async fn remaining(&self) -> u32 {
        let times = self.call_times.lock().await;
        let now = Instant::now();
        let active = times.iter().filter(|t| now.duration_since(**t) < self.period).count();
        self.max_calls.saturating_sub(active as u32)
    }
}

/// Retry configuration for RPC reconnection (Issue 10 fix)
#[derive(Clone, Debug)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay before first retry (seconds)
    pub initial_delay_secs: u64,
    /// Maximum delay between retries (seconds)
    pub max_delay_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 10,
            initial_delay_secs: 1,
            max_delay_secs: 60,
        }
    }
}

/// Chain client for interacting with Anarchy blockchain
/// 
/// Uses subxt for type-safe chain interaction and extrinsic submission.
/// Supports automatic failover to backup endpoints.
pub struct ChainClient {
    /// RPC endpoint URL (initial endpoint)
    #[allow(dead_code)]
    endpoint: String,
    /// Failover manager for blockchain connections (FR-510, FR-511)
    failover_manager: Arc<FailoverManager>,
    /// Endpoint cache for peer discovery (FR-507)
    endpoint_cache: Arc<EndpointCache>,
    /// Rate limiter for declare_holding
    rate_limiter: RateLimiter,
    /// Sr25519 signer (schnorrkel) for message signing
    signer: Signer,
    /// subxt keypair for extrinsic submission
    subxt_keypair: SubxtKeypair,
    /// subxt OnlineClient for chain interaction
    subxt_client: Mutex<Option<OnlineClient<SubstrateConfig>>>,
    /// Track holdings: hash → (post_id, index)
    holding_map: Mutex<HashMap<FragmentId, (u64, u32)>>,
    /// 接続済みチェーンの genesis hash。最初に subxt 接続が成功した時点で
    /// 確定し、以後の再接続・failover standby 候補の検証に使う。
    /// gossip 由来のエンドポイントは chain_id を自己申告するだけ
    /// (dev は周知の [0u8; 32]) なので、昇格前に実際に接続して
    /// genesis を照合しないと偽チェーンへ failover してしまう。
    expected_genesis: std::sync::Mutex<Option<subxt::utils::H256>>,
    /// Retry configuration for reconnection (Issue 10 fix)
    retry_config: RetryConfig,
    /// チェーンノード向け HTTP JSON-RPC 用の共有 reqwest クライアント。
    /// タイムアウト未設定の Client::new() を都度生成していると、相手が
    /// 無応答のとき main イベントループの select! アームごと固まるため、
    /// connect 5s / 全体 30s を設定して全呼び出しで使い回す。
    http_client: reqwest::Client,
}

impl ChainClient {
    /// Create a new chain client
    pub async fn new(
        endpoint: &str,
        declare_rate_limit: u32,
        signer_seed: &str,
        failover_manager: Arc<FailoverManager>,
        endpoint_cache: Arc<EndpointCache>,
    ) -> Result<Self> {
        info!(endpoint = endpoint, "Connecting to chain");
        
        // Create signer from seed
        let signer = Signer::from_seed_hex(signer_seed)?;
        info!(
            account = %signer.ss58_address(),
            "Signer initialized for extrinsic signing"
        );
        
        // Create subxt keypair from same seed
        let seed_bytes = hex::decode(signer_seed)
            .context("Invalid hex seed for subxt keypair")?;
        let seed_arr: [u8; 32] = seed_bytes.try_into()
            .map_err(|_| anyhow::anyhow!("Seed must be 32 bytes"))?;
        let subxt_keypair = SubxtKeypair::from_secret_key(seed_arr)
            .map_err(|e| anyhow::anyhow!("Failed to create subxt keypair: {}", e))?;
        
        // Set initial endpoint as primary in failover manager
        let initial_endpoint = BlockchainEndpoint {
            url: endpoint.to_string(),
            chain_id: [0u8; 32], // Dev chain_id
            last_verified: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            latency_ms: 0,
            ttl_secs: 300,
        };
        failover_manager.set_primary(initial_endpoint).await;
        
        // Try to connect via subxt (may fail if node not running)
        let subxt_client = match OnlineClient::<SubstrateConfig>::from_url(endpoint).await {
            Ok(client) => {
                info!(endpoint = endpoint, "Connected to chain via subxt");
                Some(client)
            }
            Err(e) => {
                warn!(endpoint = endpoint, error = %e, "Failed to connect via subxt, will retry on first use");
                None
            }
        };

        // 接続できていれば genesis hash を記録（failover 候補の検証用）
        let expected_genesis = subxt_client.as_ref().map(|c| c.genesis_hash());

        // 共有 HTTP クライアント（タイムアウト付き）
        let http_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build reqwest client")?;

        Ok(Self {
            endpoint: endpoint.to_string(),
            failover_manager,
            endpoint_cache,
            rate_limiter: RateLimiter::new(declare_rate_limit),
            signer,
            subxt_keypair,
            subxt_client: Mutex::new(subxt_client),
            holding_map: Mutex::new(HashMap::new()),
            expected_genesis: std::sync::Mutex::new(expected_genesis),
            retry_config: RetryConfig::default(),
            http_client,
        })
    }

    /// Ensure we have a connected subxt client, reconnecting if necessary
    async fn ensure_subxt_client(&self) -> Result<OnlineClient<SubstrateConfig>> {
        let mut client_guard = self.subxt_client.lock().await;
        
        if let Some(ref client) = *client_guard {
            // Client exists, assume still valid (subxt handles reconnection)
            return Ok(client.clone());
        }
        
        // Need to connect/reconnect
        let endpoint = self.active_endpoint().await
            .ok_or_else(|| anyhow::anyhow!("No primary endpoint available"))?;
        
        info!(endpoint = %endpoint, "Connecting to chain via subxt");
        let client = OnlineClient::<SubstrateConfig>::from_url(&endpoint)
            .await
            .context("Failed to connect via subxt")?;

        // 再接続先（failover 後の新 primary を含む）が同一チェーンであることを
        // genesis hash で確認する。初回接続なら記録する。
        let genesis = client.genesis_hash();
        {
            let mut expected = self.expected_genesis.lock()
                .expect("expected_genesis mutex poisoned");
            match *expected {
                Some(e) if e != genesis => {
                    bail!(
                        "Genesis hash mismatch at {}: expected {}, got {} — refusing connection",
                        endpoint,
                        hex::encode(e.0),
                        hex::encode(genesis.0)
                    );
                }
                Some(_) => {}
                None => *expected = Some(genesis),
            }
        }

        *client_guard = Some(client.clone());
        Ok(client)
    }

    /// Invalidate current subxt client and force reconnection (Issue 10 fix)
    pub async fn reconnect(&self) -> Result<()> {
        let mut client_guard = self.subxt_client.lock().await;
        *client_guard = None;
        drop(client_guard);
        
        // Re-establish connection
        let _ = self.ensure_subxt_client().await?;
        info!("Reconnected to chain");
        Ok(())
    }

    /// Reconnect with exponential backoff (Issue 10 fix)
    /// 
    /// Attempts to reconnect up to `max_retries` times with exponential backoff.
    /// Delay doubles each attempt: 1s, 2s, 4s, 8s, ... up to `max_delay_secs`.
    pub async fn reconnect_with_backoff(&self) -> Result<()> {
        let config = &self.retry_config;
        let mut delay = Duration::from_secs(config.initial_delay_secs);
        let max_delay = Duration::from_secs(config.max_delay_secs);
        
        for attempt in 1..=config.max_retries {
            // Invalidate current client
            {
                let mut client_guard = self.subxt_client.lock().await;
                *client_guard = None;
            }
            
            match self.ensure_subxt_client().await {
                Ok(_) => {
                    info!(attempt = attempt, "Reconnected to chain after {} attempts", attempt);
                    return Ok(());
                }
                Err(e) => {
                    if attempt == config.max_retries {
                        warn!(
                            attempt = attempt,
                            error = %e,
                            "Failed to reconnect after {} attempts, giving up",
                            config.max_retries
                        );
                        return Err(e);
                    }
                    
                    warn!(
                        attempt = attempt,
                        delay_secs = delay.as_secs(),
                        error = %e,
                        "Reconnection attempt failed, retrying in {} seconds",
                        delay.as_secs()
                    );
                    tokio::time::sleep(delay).await;
                    
                    // Exponential backoff with cap
                    delay = std::cmp::min(delay * 2, max_delay);
                }
            }
        }
        
        bail!("Reconnection failed after {} attempts", config.max_retries)
    }

    /// Get the current active endpoint URL (from failover manager)
    pub async fn active_endpoint(&self) -> Option<String> {
        self.failover_manager.get_primary_url().await
    }

    /// Execute an RPC operation with automatic reconnection on failure (Issue 10 fix)
    /// 
    /// If the operation fails with a connection error, attempts to reconnect
    /// with exponential backoff before retrying the operation.
    pub async fn with_reconnect<T, F, Fut>(&self, operation: F) -> Result<T>
    where
        F: Fn(OnlineClient<SubstrateConfig>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // First attempt
        match self.ensure_subxt_client().await {
            Ok(client) => {
                match operation(client).await {
                    Ok(result) => {
                        self.report_success().await;
                        return Ok(result);
                    }
                    Err(e) => {
                        // Check if this is a connection error
                        let error_str = e.to_string().to_lowercase();
                        if error_str.contains("connection")
                            || error_str.contains("disconnected")
                            || error_str.contains("timeout")
                            || error_str.contains("websocket")
                        {
                            warn!(error = %e, "RPC call failed with connection error, attempting reconnect");
                            self.report_failure().await;
                        } else {
                            // Not a connection error, propagate immediately
                            return Err(e);
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "No active connection, attempting reconnect");
            }
        }

        // Reconnect with backoff
        self.reconnect_with_backoff().await?;

        // Retry operation
        let client = self.ensure_subxt_client().await?;
        let result = operation(client).await;
        if result.is_ok() {
            self.report_success().await;
        }
        result
    }

    /// Get the signer's account ID (for reward claims)
    pub fn signer_account_id(&self) -> [u8; 32] {
        self.signer.account_id()
    }

    /// Get the signer's SS58 address (for logging)
    pub fn signer_ss58(&self) -> String {
        self.signer.ss58_address()
    }

    /// Report a successful RPC call
    pub async fn report_success(&self) {
        self.failover_manager.record_primary_success().await;
    }

    /// Report a failed RPC call and potentially trigger failover
    /// 
    /// Returns the new primary URL if failover occurred
    pub async fn report_failure(&self) -> Option<String> {
        let new_primary = self.failover_manager.record_primary_failure().await;
        if new_primary.is_some() {
            // Try to add standbys from endpoint cache
            self.refresh_standbys_from_cache().await;
        }
        new_primary
    }

    /// Refresh standby connections from endpoint cache
    ///
    /// gossip 由来の candidate は chain_id を自己申告するだけなので、
    /// standby に昇格させる前に実際に接続して genesis hash を照合する。
    /// 偽チェーンを standby に入れると、failover 後に `fragment_exists`
    /// が攻撃者の自由になり Put 認証が無効化されるため。
    /// 一致したものだけ追加し、不一致はキャッシュからも落とす。
    async fn refresh_standbys_from_cache(&self) {
        let expected_genesis = *self.expected_genesis.lock()
            .expect("expected_genesis mutex poisoned");
        let Some(expected_genesis) = expected_genesis else {
            // 一度も正規チェーンに接続できていない場合は検証基準がないため
            // fail-closed: gossip 由来の candidate は採用しない。
            warn!("Skipping standby refresh: chain genesis unknown (never connected); cannot verify gossiped endpoints");
            return;
        };

        let endpoints = self.endpoint_cache.get_all_by_latency().await;
        let current_primary = self.failover_manager.get_primary_url().await;

        for endpoint in endpoints.into_iter().take(5) {
            // Don't add current primary as standby
            if Some(&endpoint.url) == current_primary.as_ref() {
                continue;
            }

            // 昇格前に genesis hash を実照合する
            let genesis = match tokio::time::timeout(
                Duration::from_secs(10),
                OnlineClient::<SubstrateConfig>::from_url(&endpoint.url),
            )
            .await
            {
                Ok(Ok(client)) => client.genesis_hash(),
                Ok(Err(e)) => {
                    warn!(url = %endpoint.url, error = %e, "Skipping standby candidate: connection failed");
                    continue;
                }
                Err(_) => {
                    warn!(url = %endpoint.url, "Skipping standby candidate: connection timed out");
                    continue;
                }
            };

            if genesis != expected_genesis {
                warn!(
                    url = %endpoint.url,
                    expected = %hex::encode(expected_genesis.0),
                    got = %hex::encode(genesis.0),
                    "Dropping standby candidate: genesis hash mismatch (possible failover poisoning)"
                );
                self.endpoint_cache.remove(&endpoint.url).await;
                continue;
            }

            self.failover_manager.add_standby(endpoint).await;
        }
    }

    /// Check if a fragment is registered on-chain (FR-107).
    ///
    /// (#30-C-2) Used by the libp2p Put handler to authenticate incoming
    /// fragment writes: only fragment ids that the chain has registered
    /// (i.e. someone paid for them) may be stored. Without this check,
    /// any peer could fill our disk with arbitrary data.
    ///
    /// Queries `Storage::Fragments(fragment_id)` via subxt dynamic storage.
    /// `Ok(true)` means the fragment record exists on chain.
    pub async fn fragment_exists(&self, fragment_id: &FragmentId) -> Result<bool> {
        debug!(fragment_id = %hex::encode(fragment_id), "Checking fragment existence on-chain");

        let client = self.ensure_subxt_client().await?;
        let query = subxt::dynamic::storage(
            "Storage",
            "Fragments",
            vec![Value::from_bytes(fragment_id.to_vec())],
        );
        let result = client
            .storage()
            .at_latest()
            .await
            .context("Failed to acquire storage handle")?
            .fetch(&query)
            .await
            .context("Failed to fetch Fragments storage")?;
        Ok(result.is_some())
    }

    /// `pallet_storage` の extrinsic を署名提出し、ブロック取り込みと
    /// dispatch 成功まで待つ共通処理。
    ///
    /// `EXTRINSIC_TIMEOUT_SECS` でタイムアウトする（チェーン停止時に
    /// 呼び出し元が固まらないようにするため）。成功時は取り込まれた
    /// ブロックハッシュを返す。
    async fn submit_storage_extrinsic(
        &self,
        call_name: &'static str,
        fields: Vec<Value>,
    ) -> Result<subxt::utils::H256> {
        let client = self.ensure_subxt_client().await?;
        let tx = subxt::dynamic::tx("Storage", call_name, fields);

        let submit_and_wait = async {
            let mut progress = client
                .tx()
                .sign_and_submit_then_watch_default(&tx, &self.subxt_keypair)
                .await
                .with_context(|| format!("Failed to submit {} extrinsic", call_name))?;

            use subxt::tx::TxStatus;
            loop {
                match progress.next().await {
                    Some(Ok(TxStatus::InBestBlock(details)))
                    | Some(Ok(TxStatus::InFinalizedBlock(details))) => {
                        // dispatch エラー (NodeNotRegistered 等) を検出する
                        details
                            .wait_for_success()
                            .await
                            .with_context(|| format!("{} dispatch failed", call_name))?;
                        return Ok(details.block_hash());
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => return Err(anyhow::anyhow!("Transaction error: {}", e)),
                    None => return Err(anyhow::anyhow!("Transaction stream ended unexpectedly")),
                }
            }
        };

        let block_hash = match tokio::time::timeout(
            Duration::from_secs(EXTRINSIC_TIMEOUT_SECS),
            submit_and_wait,
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => {
                warn!(
                    call = call_name,
                    timeout_secs = EXTRINSIC_TIMEOUT_SECS,
                    "Extrinsic submission timed out waiting for inclusion"
                );
                bail!("{} timed out after {}s", call_name, EXTRINSIC_TIMEOUT_SECS);
            }
        };

        self.report_success().await;
        Ok(block_hash)
    }

    /// Declare holding of a fragment (submits `Storage::declare_holding` extrinsic)
    pub async fn declare_holding(&self, fragment_id: FragmentId) -> Result<()> {
        // Check rate limit (FR-108)
        if !self.rate_limiter.try_acquire().await {
            bail!("Rate limit exceeded for declare_holding");
        }

        debug!(fragment_id = %hex::encode(fragment_id), "Declaring holding");

        let block_hash = self
            .submit_storage_extrinsic(
                "declare_holding",
                vec![Value::from_bytes(fragment_id)],
            )
            .await?;

        info!(
            fragment_id = %hex::encode(fragment_id),
            block_hash = %hex::encode(block_hash.0),
            "declare_holding extrinsic included in block"
        );
        Ok(())
    }

    /// Declare holding for a post fragment (T060)
    ///
    /// This tracks the post_id + index for the given fragment hash,
    /// then submits the declare_holding extrinsic.
    pub async fn declare_holding_for_post(
        &self,
        post_id: u64,
        index: u32,
        fragment_hash: FragmentId,
    ) -> Result<()> {
        // Check rate limit (FR-108)
        if !self.rate_limiter.try_acquire().await {
            bail!("Rate limit exceeded for declare_holding");
        }

        // Track the mapping: hash → (post_id, index)
        {
            let mut map = self.holding_map.lock().await;
            map.insert(fragment_hash, (post_id, index));
        }

        debug!(
            post_id = post_id,
            index = index,
            hash = %hex::encode(fragment_hash),
            "Declaring holding for post fragment"
        );

        let block_hash = self
            .submit_storage_extrinsic(
                "declare_holding",
                vec![Value::from_bytes(fragment_hash)],
            )
            .await?;

        info!(
            post_id = post_id,
            index = index,
            hash = %hex::encode(fragment_hash),
            block_hash = %hex::encode(block_hash.0),
            "declare_holding (post fragment) extrinsic included in block"
        );
        Ok(())
    }

    /// Get holding info for a fragment hash
    /// Returns (post_id, index) if tracked
    pub async fn get_holding_info(&self, fragment_hash: &FragmentId) -> Option<(u64, u32)> {
        let map = self.holding_map.lock().await;
        map.get(fragment_hash).copied()
    }

    /// Revoke holding of a fragment (submits `Storage::revoke_holding` extrinsic)
    pub async fn revoke_holding(&self, fragment_id: FragmentId) -> Result<()> {
        // Check rate limit
        if !self.rate_limiter.try_acquire().await {
            bail!("Rate limit exceeded for revoke_holding");
        }

        debug!(fragment_id = %hex::encode(fragment_id), "Revoking holding");

        let block_hash = self
            .submit_storage_extrinsic(
                "revoke_holding",
                vec![Value::from_bytes(fragment_id)],
            )
            .await?;

        info!(
            fragment_id = %hex::encode(fragment_id),
            block_hash = %hex::encode(block_hash.0),
            "revoke_holding extrinsic included in block"
        );
        Ok(())
    }

    /// Get fragment metadata from chain
    ///
    /// 未実装: 以前は `Ok(None)` を返して「メタデータなし」を装っていた。
    /// 呼び出し元が「存在しない」と「未実装」を区別できるよう明示的にエラーを返す。
    pub async fn get_fragment_metadata(&self, fragment_id: &FragmentId) -> Result<Option<FragmentMetadata>> {
        debug!(fragment_id = %hex::encode(fragment_id), "Fetching fragment metadata");
        bail!("get_fragment_metadata is not implemented: on-chain metadata query is missing")
    }

    /// Submit KZG holding proof to chain (T083)
    /// 
    /// Submits `prove_holding_kzg` extrinsic with the proof data via subxt.
    /// Called by ChallengeMonitor after generating a valid proof.
    pub async fn submit_holding_proof(
        &self,
        content_hash: [u8; 32],
        share_index: u8,
        share_value: [u8; 32],
        proof: [u8; 48],
    ) -> Result<()> {
        info!(
            content_hash = %hex::encode(content_hash),
            share_index = share_index,
            account = %self.signer_ss58(),
            "Submitting KZG holding proof to chain via subxt"
        );

        // pallet_storage::Call::prove_holding_kzg { content_hash, share_index, share_value, proof }
        // share_value and proof are BoundedVec<u8, ConstU32<N>>
        let block_hash = self
            .submit_storage_extrinsic(
                "prove_holding_kzg",
                vec![
                    // content_hash: [u8; 32]
                    Value::from_bytes(content_hash),
                    // share_index: u8
                    Value::u128(share_index as u128),
                    // share_value: BoundedVec<u8, ConstU32<32>>
                    Value::from_bytes(share_value),
                    // proof: BoundedVec<u8, ConstU32<48>>
                    Value::from_bytes(proof),
                ],
            )
            .await?;

        info!(
            content_hash = %hex::encode(content_hash),
            block_hash = %hex::encode(block_hash.0),
            "Successfully submitted KZG holding proof - reward pending"
        );
        Ok(())
    }

    /// Claim accumulated rewards from chain
    /// 
    /// Submits `claim_reward` extrinsic via subxt.
    /// Returns Ok(()) on success. Check balance to see claimed amount.
    pub async fn claim_reward(&self) -> Result<()> {
        info!(
            account = %self.signer_ss58(),
            "Claiming accumulated rewards from chain"
        );

        // pallet_storage::Call::claim_reward (no parameters)
        let block_hash = self
            .submit_storage_extrinsic("claim_reward", Vec::<Value>::new())
            .await?;

        info!(
            account = %self.signer_ss58(),
            block_hash = %hex::encode(block_hash.0),
            "Successfully claimed reward"
        );
        Ok(())
    }

    /// Get list of fragment holders from chain
    ///
    /// 未実装: 以前は `Ok(vec![])` を返して「ホルダーなし」を装っていた。
    /// 呼び出し元が「ホルダーゼロ」と「未実装」を区別できるよう明示的にエラーを返す。
    pub async fn get_fragment_holders(&self, fragment_id: &FragmentId) -> Result<Vec<Vec<u8>>> {
        debug!(fragment_id = %hex::encode(fragment_id), "Fetching fragment holders");
        bail!("get_fragment_holders is not implemented: FragmentHolders storage query is missing")
    }

    /// Check connection status — reflects the actual subxt client state.
    /// (#31-H-3): the previous `connected: bool` field was set once at construction
    /// and never updated, so it lied after reconnects/disconnects. Now reads the
    /// real client state under the mutex.
    pub async fn is_connected(&self) -> bool {
        self.subxt_client.lock().await.is_some()
    }

    /// Get remaining rate limit quota
    pub async fn rate_limit_remaining(&self) -> u32 {
        self.rate_limiter.remaining().await
    }

    /// Register this Storage Node with the blockchain node
    /// 
    /// Calls the `storage_registerEndpoint` RPC to register our HTTP endpoint.
    /// This allows the blockchain node to forward fragment requests to us.
    /// Uses failover manager to handle primary node failures (FR-510, FR-511).
    /// Sends signed registration request (PR #22 CRITICAL-3 compatibility).
    pub async fn register_with_blockchain(&self, our_rpc_url: &str) -> Result<()> {
        /// 署名付きエンドポイント登録リクエスト (PR #22 CRITICAL-3 format)
        #[derive(serde::Serialize)]
        struct SignedEndpointRegistration {
            url: String,
            operator: String,
            timestamp: u64,
            signature: String,
        }
        
        #[derive(serde::Serialize)]
        struct RpcRequest {
            jsonrpc: &'static str,
            id: u32,
            method: &'static str,
            params: [SignedEndpointRegistration; 1],
        }
        
        #[derive(serde::Deserialize)]
        struct RpcResponse {
            result: Option<bool>,
            error: Option<RpcError>,
        }
        
        #[derive(serde::Deserialize)]
        struct RpcError {
            message: String,
        }
        
        // Build signed registration request
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        
        let operator = hex::encode(self.signer.account_id());
        let message = format!("register_endpoint:{}:{}", our_rpc_url, timestamp);
        let signature = hex::encode(self.signer.sign(message.as_bytes()));
        
        let signed_request = SignedEndpointRegistration {
            url: our_rpc_url.to_string(),
            operator,
            timestamp,
            signature,
        };
        
        let client = &self.http_client;
        let request = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "storage_registerEndpoint",
            params: [signed_request],
        };
        
        // Retry loop for failover (max 3 attempts)
        const MAX_RETRIES: u32 = 3;
        let mut last_error = None;
        
        for attempt in 0..MAX_RETRIES {
            // Get current primary endpoint from failover manager
            let endpoint_url = match self.active_endpoint().await {
                Some(url) => url,
                None => {
                    return Err(anyhow::anyhow!("No primary endpoint available"));
                }
            };
            
            // Convert ws:// to http:// for JSON-RPC
            let http_endpoint = endpoint_url
                .replace("ws://", "http://")
                .replace("wss://", "https://");
            
            if attempt > 0 {
                info!(
                    blockchain = %http_endpoint,
                    attempt = attempt + 1,
                    "Retrying registration after failover"
                );
            } else {
                info!(
                    blockchain = %http_endpoint,
                    storage_node = %our_rpc_url,
                    "Registering Storage Node with blockchain"
                );
            }
            
            match client
                .post(&http_endpoint)
                .json(&request)
                .send()
                .await
            {
                Ok(resp) => {
                    self.report_success().await;
                    
                    let rpc_response: RpcResponse = resp
                        .json()
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;
                    
                    if let Some(error) = rpc_response.error {
                        bail!("Blockchain node rejected registration: {}", error.message);
                    }
                    
                    if rpc_response.result == Some(true) {
                        info!("Successfully registered with blockchain node");
                    }
                    
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    // Report failure to failover manager
                    if let Some(new_primary) = self.report_failure().await {
                        info!(new_primary = %new_primary, "Failover triggered, will retry with new primary");
                        continue;
                    }
                    // No failover possible, break and return error
                    break;
                }
            }
        }
        
        Err(anyhow::anyhow!(
            "Failed to connect to blockchain node after {} attempts: {}",
            MAX_RETRIES,
            last_error.map(|e| e.to_string()).unwrap_or_default()
        ))
    }

    /// Get the current reward pool balance from blockchain
    ///
    /// Used for GC decisions: when pool is below threshold, nodes can delete data.
    pub async fn get_reward_pool_balance(&self) -> Result<u128> {
        #[derive(serde::Serialize)]
        struct RpcRequest {
            jsonrpc: &'static str,
            id: u32,
            method: &'static str,
            params: [(); 0],
        }
        
        #[derive(serde::Deserialize)]
        struct RpcResponse {
            result: Option<u128>,
            error: Option<RpcError>,
        }
        
        #[derive(serde::Deserialize)]
        struct RpcError {
            message: String,
        }
        
        let client = &self.http_client;
        let request = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "storage_getRewardPoolBalance",
            params: [],
        };
        
        let endpoint_url = self.active_endpoint().await
            .ok_or_else(|| anyhow::anyhow!("No primary endpoint available"))?;
        
        let http_endpoint = endpoint_url
            .replace("ws://", "http://")
            .replace("wss://", "https://");
        
        let resp = client
            .post(&http_endpoint)
            .json(&request)
            .send()
            .await
            .context("Failed to send RPC request")?;
        
        let rpc_response: RpcResponse = resp
            .json()
            .await
            .context("Failed to parse RPC response")?;
        
        if let Some(error) = rpc_response.error {
            bail!("RPC error: {}", error.message);
        }
        
        rpc_response.result.ok_or_else(|| anyhow::anyhow!("No result in RPC response"))
    }
    
    /// Check which content hashes are forgetting candidates
    ///
    /// Used for score-based GC: fragments marked as forgetting candidates have score < threshold.
    pub async fn check_forgetting_candidates(&self, content_hashes: Vec<[u8; 32]>) -> Result<Vec<([u8; 32], bool)>> {
        #[derive(serde::Serialize)]
        struct RpcRequest {
            jsonrpc: &'static str,
            id: u32,
            method: &'static str,
            params: [Vec<[u8; 32]>; 1],
        }
        
        #[derive(serde::Deserialize)]
        struct RpcResponse {
            result: Option<Vec<([u8; 32], bool)>>,
            error: Option<RpcError>,
        }
        
        #[derive(serde::Deserialize)]
        struct RpcError {
            message: String,
        }
        
        let client = &self.http_client;
        let request = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "storage_checkForgettingCandidates",
            params: [content_hashes],
        };
        
        let endpoint_url = self.active_endpoint().await
            .ok_or_else(|| anyhow::anyhow!("No primary endpoint available"))?;
        
        let http_endpoint = endpoint_url
            .replace("ws://", "http://")
            .replace("wss://", "https://");
        
        let resp = client
            .post(&http_endpoint)
            .json(&request)
            .send()
            .await
            .context("Failed to send RPC request")?;
        
        let rpc_response: RpcResponse = resp
            .json()
            .await
            .context("Failed to parse RPC response")?;
        
        if let Some(error) = rpc_response.error {
            bail!("RPC error: {}", error.message);
        }
        
        rpc_response.result.ok_or_else(|| anyhow::anyhow!("No result in RPC response"))
    }

    /// 最新ブロック番号を取得する（challenge cleanup の期限判定用）
    pub async fn latest_block_number(&self) -> Result<u64> {
        let client = self.ensure_subxt_client().await?;
        let block = client
            .blocks()
            .at_latest()
            .await
            .context("Failed to fetch latest block")?;
        Ok(block.number().into())
    }

    /// Poll for new challenges targeting our account (T047, Issue 11 fix)
    ///
    /// 未実装: pallet_storage::Challenges の照会・自アカウント宛フィルタは
    /// まだ実装されていない。以前は `Ok(vec![])` を返して「チャレンジなし」を
    /// 装っていたため、運用者は自動証明提出が動いていると誤認していた。
    /// 呼び出し元が未実装を検知して警告できるよう明示的にエラーを返す。
    pub async fn poll_challenges(&self) -> Result<Vec<crate::challenge::Challenge>> {
        bail!("poll_challenges is not implemented: on-chain challenge polling is missing")
    }
}

/// Fragment metadata from chain (matches pallet types)
#[derive(Debug, Clone)]
pub struct FragmentMetadata {
    pub size: u64,
    pub content_type: String,
    pub created_at: u64,
    pub owner: Vec<u8>,
}

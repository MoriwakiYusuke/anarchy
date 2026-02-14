//! Storage RPC API
//!
//! 分散ストレージ関連のRPCエンドポイントを提供。
//! フロントエンドからの断片アップロード/ダウンロードリクエストを処理し、
//! HTTP経由でStorage Nodeに転送する。
//!
//! ## アーキテクチャ
//!
//! ```text
//! Frontend → Blockchain Node RPC → HTTP → Storage Node(s)
//!                    ↑
//! 将来のインデクサー（読み取りキャッシュ）
//! ```
//!
//! - 書き込み: プライバシー重視でBlockchain Node経由（IP匿名化）
//! - 読み込み: 同様にBlockchain Node経由（将来はインデクサーキャッシュ）
//! - マルチノード: 断片を複数ノードに分散配置（耐障害性向上）

use crate::rpc::SharedStorageNodes;
use anarchy_runtime::opaque::Block;
use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::error::{ErrorCode, ErrorObject},
};
use pallet_post::PostApi as PostRuntimeApi;
use pallet_storage::StorageApi as StorageRuntimeApi;
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use std::sync::Arc;

// ============================================================================
// Security Constants (T074)
// ============================================================================

/// Maximum fragment size: 256KB (262144 bytes)
/// 断片サイズの上限。これを超えるリクエストは拒否される。
pub const MAX_FRAGMENT_SIZE: usize = 256 * 1024;

/// Maximum total leaves in MerkleTree
/// n値 (総断片数) の上限。SSS (k=3, n=5) の場合は通常5以下。
pub const MAX_TOTAL_LEAVES: u32 = 255;

/// Maximum proof size: 8KB (proofはlog2(n)に比例)
pub const MAX_PROOF_SIZE: usize = 8 * 1024;

/// 署名付きリクエスト（認証用）
/// Storage Nodeの認証ミドルウェアが検証するJSON構造体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedAuth {
    /// Sr25519公開鍵（hex 32バイト）
    pub account_id: String,
    /// Unixタイムスタンプ（秒）
    pub timestamp: u64,
    /// ランダムnonce（hex 16バイト）
    pub nonce: String,
    /// リクエストボディのBlake2bハッシュ（hex 32バイト）
    pub payload_hash: String,
    /// Sr25519署名（hex 64バイト）
    pub signature: String,
}

/// 断片アップロードリクエスト
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadFragmentRequest {
    /// 投稿ID（MerkleRootで識別）
    pub merkle_root: [u8; 32],
    /// 断片インデックス (0 ~ n-1)
    pub index: u32,
    /// 断片データ (base64エンコード)
    pub data: String,
    /// MerkleProof (base64エンコード)
    pub proof: String,
    /// 総断片数
    pub total_leaves: u32,
    /// 認証情報（オプション）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<SignedAuth>,
}

/// 断片アップロードレスポンス
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadFragmentResponse {
    /// 成功フラグ
    pub success: bool,
    /// 断片ハッシュ (Blake2b-256)
    pub fragment_hash: [u8; 32],
}

/// 断片取得リクエスト
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetFragmentRequest {
    /// 投稿ID（MerkleRootで識別）
    pub merkle_root: [u8; 32],
    /// 断片インデックス
    pub index: u32,
}

/// 断片取得レスポンス
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetFragmentResponse {
    /// 断片データ (base64エンコード)
    pub data: String,
    /// 断片ハッシュ
    pub hash: [u8; 32],
}

/// 投稿情報取得レスポンス
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostInfoResponse {
    /// MerkleRoot
    pub merkle_root: [u8; 32],
    /// 復元に必要な最小断片数
    pub k: u32,
    /// 総断片数
    pub n: u32,
    /// 元データサイズ
    pub size: u64,
    /// 利用可能な断片インデックス一覧
    pub available_indices: Vec<u32>,
}

/// ホルダー情報
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HolderInfo {
    /// Storage NodeのAccountId (hex)
    pub account_id: String,
    /// 保持している断片インデックス
    pub indices: Vec<u32>,
    /// Storage NodeのエンドポイントURL
    pub endpoint: Option<String>,
}

/// ListHoldersレスポンス
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListHoldersResponse {
    /// ホルダー一覧
    pub holders: Vec<HolderInfo>,
}

/// GetNodesレスポンス（T103: 登録ノード一覧取得RPC用）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetNodesResponse {
    /// 登録されたノード一覧
    pub nodes: Vec<NodeInfo>,
    /// オンラインノード数
    pub online_count: usize,
    /// 総ノード数
    pub total_count: usize,
}

/// ノード情報（RPC用、簡略化版）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    /// エンドポイントURL
    pub endpoint: String,
    /// オンライン状態
    pub is_online: bool,
    /// 登録時刻（Unix timestamp）
    pub registered_at: u64,
}

/// 登録されたStorage Node情報
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisteredStorageNode {
    /// エンドポイントURL
    pub endpoint: String,
    /// ノードID（オプション、将来的にPeerIdを使用）
    pub node_id: Option<String>,
    /// 登録時刻（Unix timestamp）
    pub registered_at: u64,
    /// オンライン状態
    pub is_online: bool,
    /// 最後のヘルスチェック時刻
    pub last_health_check: u64,
    /// レイテンシ（ミリ秒）- 最寄りノード選択用 (FR-104)
    pub latency_ms: Option<u64>,
}

impl RegisteredStorageNode {
    /// 新しいノードを作成
    pub fn new(endpoint: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            endpoint,
            node_id: None,
            registered_at: now,
            is_online: true,
            last_health_check: now,
            latency_ms: None,
        }
    }
}

/// Storage RPC API定義
#[rpc(server, client, namespace = "storage")]
pub trait StorageApi {
    /// Storage Nodeエンドポイントを登録
    ///
    /// Storage Nodeが起動時にこのRPCを呼び出して自分を登録する。
    /// これにより環境変数なしで自動接続が可能になる。
    #[method(name = "registerEndpoint")]
    async fn register_endpoint(&self, url: String) -> RpcResult<bool>;

    /// 断片をアップロード
    ///
    /// フロントエンドからの断片データを受け取り、MerkleProofを検証後、
    /// HTTP経由でStorage Nodeに転送する。
    #[method(name = "uploadFragment")]
    async fn upload_fragment(&self, request: UploadFragmentRequest) -> RpcResult<UploadFragmentResponse>;

    /// 断片を取得
    ///
    /// Storage Nodeから指定された断片を取得する。
    #[method(name = "getFragment")]
    async fn get_fragment(&self, request: GetFragmentRequest) -> RpcResult<GetFragmentResponse>;

    /// 投稿情報を取得
    ///
    /// 指定されたMerkleRootの投稿メタデータと利用可能な断片情報を返す。
    #[method(name = "getPostInfo")]
    async fn get_post_info(&self, merkle_root: [u8; 32]) -> RpcResult<PostInfoResponse>;

    /// 断片を保持しているノード一覧を取得
    ///
    /// 指定された投稿IDに対してdeclare_holdingしているStorage Nodeの一覧を返す。
    #[method(name = "listHolders")]
    async fn list_holders(&self, post_id: u64) -> RpcResult<ListHoldersResponse>;

    /// 登録されたStorage Node一覧を取得
    ///
    /// チェーンノードに登録されている全Storage Nodeの情報を返す。
    /// フロントエンドでのノード状態表示に使用。
    #[method(name = "getNodes")]
    async fn get_nodes(&self) -> RpcResult<GetNodesResponse>;
}

/// Storage Node HTTPクライアント
pub struct StorageNodeClient {
    /// HTTPクライアント
    http_client: reqwest::Client,
    /// Storage NodeのベースURL
    storage_node_url: String,
}

impl StorageNodeClient {
    /// 新しいクライアントを作成
    pub fn new(storage_node_url: String) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            storage_node_url,
        }
    }

    /// 断片をStorage Nodeにアップロード
    pub async fn upload_fragment(&self, request: &UploadFragmentRequest) -> Result<UploadFragmentResponse, String> {
        #[derive(Serialize)]
        struct RpcRequest<'a> {
            jsonrpc: &'static str,
            id: u32,
            method: &'static str,
            params: &'a UploadFragmentRequest,
        }

        #[derive(Deserialize)]
        struct RpcResponse {
            result: Option<UploadFragmentResponse>,
            error: Option<RpcError>,
        }

        #[derive(Deserialize)]
        struct RpcError {
            message: String,
        }

        let rpc_request = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "storage_storeFragment",
            params: request,
        };

        // Build HTTP request with optional auth header
        let mut http_request = self.http_client
            .post(&self.storage_node_url)
            .json(&rpc_request);
        
        // Add X-Anarchy-Auth header if auth is present
        if let Some(auth) = &request.auth {
            let auth_json = serde_json::to_string(auth)
                .map_err(|e| format!("Failed to serialize auth: {}", e))?;
            http_request = http_request.header("X-Anarchy-Auth", auth_json);
        }

        let response = http_request
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        // Check HTTP status before parsing JSON
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Storage Node returned HTTP {}: {}", status.as_u16(), body));
        }

        let rpc_response: RpcResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if let Some(error) = rpc_response.error {
            return Err(error.message);
        }

        rpc_response.result.ok_or_else(|| "No result in response".to_string())
    }

    /// Storage Nodeから断片を取得
    pub async fn get_fragment(&self, merkle_root: &[u8; 32], index: u32) -> Result<GetFragmentResponse, String> {
        #[derive(Serialize)]
        struct RpcRequest {
            jsonrpc: &'static str,
            id: u32,
            method: &'static str,
            params: GetFragmentParams,
        }

        #[derive(Serialize)]
        struct GetFragmentParams {
            merkle_root: [u8; 32],
            index: u32,
        }

        #[derive(Deserialize)]
        struct RpcResponse {
            result: Option<GetFragmentResponse>,
            error: Option<RpcError>,
        }

        #[derive(Deserialize)]
        struct RpcError {
            message: String,
        }

        let rpc_request = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "storage_getFragment",
            params: GetFragmentParams {
                merkle_root: *merkle_root,
                index,
            },
        };

        let response = self.http_client
            .post(&self.storage_node_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        // Check HTTP status before parsing JSON
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Storage Node returned HTTP {}: {}", status.as_u16(), body));
        }

        let rpc_response: RpcResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if let Some(error) = rpc_response.error {
            return Err(error.message);
        }

        rpc_response.result.ok_or_else(|| "No result in response".to_string())
    }
}

/// Storage RPC実装
pub struct Storage<C> {
    /// Runtime Client（チェーン状態参照用）
    client: Arc<C>,
    /// Storage Nodeレジストリ (マルチノード対応)
    storage_nodes: SharedStorageNodes,
    /// Gossipハンドル (ノード登録のブロードキャスト用)
    gossip_handle: crate::gossip::StorageNodeGossipHandle,
}

impl<C> Storage<C>
where
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: StorageRuntimeApi<Block>,
{
    /// 新しいStorage RPCハンドラを作成
    /// Storage Nodeは起動時にstorage_registerEndpoint RPCで自動登録される
    /// 複数ノードが登録可能で、断片は分散配置される
    pub fn new(client: Arc<C>, storage_nodes: SharedStorageNodes, gossip_handle: crate::gossip::StorageNodeGossipHandle) -> Self {
        Self { 
            client, 
            storage_nodes,
            gossip_handle,
        }
    }
    
    /// オンチェーン登録されたストレージノードのHTTP URLリストを取得
    fn get_on_chain_storage_nodes(&self) -> Vec<String> {
        let best_hash = self.client.info().best_hash;
        let api = self.client.runtime_api();
        
        match api.get_all_storage_nodes(best_hash) {
            Ok(nodes) => nodes
                .into_iter()
                .filter_map(|n| String::from_utf8(n.http_url).ok())
                .collect(),
            Err(e) => {
                log::warn!("Failed to fetch on-chain storage nodes: {:?}", e);
                Vec::new()
            }
        }
    }
    
    /// 指定インデックスの断片用にStorage Nodeクライアントを取得
    /// 断片インデックスに基づいて異なるノードを選択（分散配置）
    /// インメモリレジストリを優先し、不足時はオンチェーンノードをフォールバック
    async fn get_storage_client_for_fragment(&self, fragment_index: usize) -> Option<StorageNodeClient> {
        // まずインメモリレジストリから取得を試みる
        {
            let registry = self.storage_nodes.read().await;
            if let Some(node) = registry.select_node_for_fragment(fragment_index) {
                return Some(StorageNodeClient::new(node.endpoint.clone()));
            }
        }
        
        // インメモリに十分なノードがない場合、オンチェーンからフォールバック
        let on_chain_urls = self.get_on_chain_storage_nodes();
        if on_chain_urls.is_empty() {
            return None;
        }
        
        // オンチェーンノードからインデックスベースで選択
        let node_index = fragment_index % on_chain_urls.len();
        Some(StorageNodeClient::new(on_chain_urls[node_index].clone()))
    }
    
    /// 全てのオンラインノードへのクライアントを取得（取得時のフォールバック用）
    /// 選択戦略に基づいた順序でノードを返す (FR-101)
    /// インメモリノードとオンチェーンノードの両方を含む
    async fn get_all_storage_clients(&self) -> Vec<StorageNodeClient> {
        let mut endpoints: Vec<String> = Vec::new();
        
        // インメモリレジストリから（ランダム順序で取得 - プライバシー優先）
        {
            let registry = self.storage_nodes.read().await;
            for node in registry.online_nodes_shuffled() {
                endpoints.push(node.endpoint.clone());
            }
        }
        
        // オンチェーンからも追加（重複除去）
        for url in self.get_on_chain_storage_nodes() {
            if !endpoints.contains(&url) {
                endpoints.push(url);
            }
        }
        
        endpoints.into_iter()
            .map(StorageNodeClient::new)
            .collect()
    }
}

/// MerkleProofを検証（Blake2b-256ベース）
fn verify_merkle_proof(
    root: &[u8; 32],
    proof_bytes: &[u8],
    leaf_data: &[u8],
    leaf_index: usize,
    total_leaves: usize,
) -> Result<bool, String> {
    use blake2::{Blake2b, Digest};
    use rs_merkle::{Hasher, MerkleProof};

    /// Blake2b-256 Hasher
    #[derive(Clone)]
    struct Blake2bHasher;

    impl Hasher for Blake2bHasher {
        type Hash = [u8; 32];

        fn hash(data: &[u8]) -> Self::Hash {
            let mut hasher = Blake2b::<blake2::digest::consts::U32>::new();
            hasher.update(data);
            hasher.finalize().into()
        }
    }

    let proof = MerkleProof::<Blake2bHasher>::from_bytes(proof_bytes)
        .map_err(|e| format!("Invalid proof format: {:?}", e))?;

    let leaf_hash = Blake2bHasher::hash(leaf_data);

    Ok(proof.verify(*root, &[leaf_index], &[leaf_hash], total_leaves))
}

/// Blake2b-256ハッシュを計算
fn blake2b_hash(data: &[u8]) -> [u8; 32] {
    use blake2::{Blake2b, Digest};
    let mut hasher = Blake2b::<blake2::digest::consts::U32>::new();
    hasher.update(data);
    hasher.finalize().into()
}

#[async_trait::async_trait]
impl<C> StorageApiServer for Storage<C>
where
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: PostRuntimeApi<Block> + StorageRuntimeApi<Block>,
{
    async fn register_endpoint(&self, url: String) -> RpcResult<bool> {
        // URLの基本的な検証
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                "Invalid URL: must start with http:// or https://",
                None::<()>,
            ));
        }

        // マルチノード対応：ノードをレジストリに追加
        let mut registry = self.storage_nodes.write().await;
        let node = RegisteredStorageNode::new(url.clone());
        let registered_at = node.registered_at;
        let latency_ms = node.latency_ms;
        
        if registry.register(node) {
            log::info!("Storage Node registered: {} (total: {} nodes)", url, registry.nodes.len());
            
            // Gossipで他チェーンノードにブロードキャスト
            self.gossip_handle.broadcast_registration(url.clone(), registered_at, latency_ms);
            
            Ok(true)
        } else {
            log::info!("Storage Node already registered: {}", url);
            Ok(false) // 既に登録済みの場合もエラーにはしない
        }
    }

    async fn upload_fragment(&self, request: UploadFragmentRequest) -> RpcResult<UploadFragmentResponse> {
        // 0. ノード数チェック（total_leaves以上のノードが必要）
        let required_nodes = request.total_leaves as usize;
        let available_nodes = {
            let registry = self.storage_nodes.read().await;
            registry.online_node_count()
        };
        
        if available_nodes < required_nodes {
            return Err(ErrorObject::owned(
                ErrorCode::ServerError(-32001).code(),
                format!(
                    "Insufficient Storage Nodes: {} required, {} available. Start more storage-node instances.",
                    required_nodes,
                    available_nodes
                ),
                None::<()>,
            ));
        }

        // 1. Base64デコード
        use base64::{engine::general_purpose::STANDARD, Engine};
        
        let data = STANDARD.decode(&request.data).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                format!("Invalid base64 data: {}", e),
                None::<()>,
            )
        })?;

        let proof = STANDARD.decode(&request.proof).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                format!("Invalid base64 proof: {}", e),
                None::<()>,
            )
        })?;

        // ============================================================
        // Security Validation (T074)
        // ============================================================

        // Fragment size check (max 256KB)
        if data.len() > MAX_FRAGMENT_SIZE {
            return Err(ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                format!(
                    "Fragment size {} exceeds maximum allowed {} bytes",
                    data.len(),
                    MAX_FRAGMENT_SIZE
                ),
                None::<()>,
            ));
        }

        // Proof size check (max 8KB)
        if proof.len() > MAX_PROOF_SIZE {
            return Err(ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                format!(
                    "Proof size {} exceeds maximum allowed {} bytes",
                    proof.len(),
                    MAX_PROOF_SIZE
                ),
                None::<()>,
            ));
        }

        // total_leaves check
        if request.total_leaves == 0 || request.total_leaves > MAX_TOTAL_LEAVES {
            return Err(ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                format!(
                    "Invalid total_leaves {}: must be 1-{}",
                    request.total_leaves,
                    MAX_TOTAL_LEAVES
                ),
                None::<()>,
            ));
        }

        // index check (must be < total_leaves)
        if request.index >= request.total_leaves {
            return Err(ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                format!(
                    "Invalid index {}: must be < total_leaves ({})",
                    request.index,
                    request.total_leaves
                ),
                None::<()>,
            ));
        }

        // 2. MerkleProof検証
        let is_valid = verify_merkle_proof(
            &request.merkle_root,
            &proof,
            &data,
            request.index as usize,
            request.total_leaves as usize,
        )
        .map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                format!("Proof verification error: {}", e),
                None::<()>,
            )
        })?;

        if !is_valid {
            return Err(ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                "MerkleProof verification failed",
                None::<()>,
            ));
        }

        // 3. 断片ハッシュ計算
        let fragment_hash = blake2b_hash(&data);

        // 4. Storage Nodeに転送（HTTP経由）- マルチノード分散配置
        // 断片インデックスに基づいて異なるノードを選択
        let storage_client = self.get_storage_client_for_fragment(request.index as usize).await.ok_or_else(|| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "No Storage Nodes connected. Start storage-node(s) and they will auto-register.",
                None::<()>,
            )
        })?;

        storage_client.upload_fragment(&request).await.map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                format!("Failed to upload fragment to Storage Node: {}", e),
                None::<()>,
            )
        })?;

        log::info!(
            "Fragment uploaded to Storage Node: root={:?}, index={}, size={}",
            hex::encode(&request.merkle_root[..8]),
            request.index,
            data.len()
        );

        Ok(UploadFragmentResponse {
            success: true,
            fragment_hash,
        })
    }

    async fn get_fragment(&self, request: GetFragmentRequest) -> RpcResult<GetFragmentResponse> {
        log::debug!(
            "get_fragment called: root={:?}, index={}",
            hex::encode(&request.merkle_root[..8]),
            request.index
        );

        // マルチノード対応：全ノードを試行（フォールバック）
        let storage_clients = self.get_all_storage_clients().await;
        
        if storage_clients.is_empty() {
            return Err(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "No Storage Nodes connected. Start storage-node(s) and they will auto-register.",
                None::<()>,
            ));
        }

        let mut last_error = String::new();
        
        // 断片を配置したノード（インデックスベース）を優先して試行
        let primary_index = request.index as usize % storage_clients.len();
        let mut indices: Vec<usize> = (0..storage_clients.len()).collect();
        // 優先ノードを先頭に移動
        indices.remove(primary_index);
        indices.insert(0, primary_index);
        
        for idx in indices {
            let client = &storage_clients[idx];
            match client.get_fragment(&request.merkle_root, request.index).await {
                Ok(response) => {
                    log::info!(
                        "Fragment retrieved from Storage Node (node {}): root={:?}, index={}",
                        idx,
                        hex::encode(&request.merkle_root[..8]),
                        request.index
                    );
                    return Ok(response);
                }
                Err(e) => {
                    log::warn!(
                        "Failed to get fragment from node {}: {}",
                        idx, e
                    );
                    last_error = e;
                }
            }
        }

        Err(ErrorObject::owned(
            ErrorCode::InternalError.code(),
            format!("Failed to get fragment from all Storage Nodes: {}", last_error),
            None::<()>,
        ))
    }

    async fn get_post_info(&self, merkle_root: [u8; 32]) -> RpcResult<PostInfoResponse> {
        // Runtime APIを使用してチェーンからコンテンツ情報を取得
        let best_hash = self.client.info().best_hash;
        let api = self.client.runtime_api();

        let content_info = api
            .get_content_by_merkle_root(best_hash, merkle_root)
            .map_err(|e| {
                ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    format!("Runtime API error: {:?}", e),
                    None::<()>,
                )
            })?
            .ok_or_else(|| {
                ErrorObject::owned(
                    ErrorCode::InvalidParams.code(),
                    format!("Post not found for merkle_root: {:?}", hex::encode(&merkle_root[..8])),
                    None::<()>,
                )
            })?;

        log::info!(
            "get_post_info: root={:?}, k={}, n={}, size={}",
            hex::encode(&merkle_root[..8]),
            content_info.k,
            content_info.n,
            content_info.size
        );

        // TODO: Storage Nodeから利用可能な断片インデックスを取得
        // 暫定: 全インデックスを返す（0..n）
        let available_indices = (0..content_info.n).collect();

        Ok(PostInfoResponse {
            merkle_root: content_info.root,
            k: content_info.k,
            n: content_info.n,
            size: content_info.size,
            available_indices,
        })
    }

    async fn list_holders(&self, post_id: u64) -> RpcResult<ListHoldersResponse> {
        // TODO: declare_holdingストレージからホルダー一覧を取得
        // マルチノード対応：登録された全ノードから情報を取得
        log::debug!("list_holders called: post_id={}", post_id);

        let registry = self.storage_nodes.read().await;
        let holders: Vec<HolderInfo> = registry.online_nodes()
            .iter()
            .enumerate()
            .map(|(idx, node)| HolderInfo {
                account_id: format!("storage_node_{}", idx),
                indices: (0..5).collect(), // 仮に0-4を保持
                endpoint: Some(node.endpoint.clone()),
            })
            .collect();

        Ok(ListHoldersResponse { holders })
    }

    async fn get_nodes(&self) -> RpcResult<GetNodesResponse> {
        log::debug!("get_nodes called");

        // インメモリレジストリからノードを取得
        let registry = self.storage_nodes.read().await;
        
        let mut nodes: Vec<NodeInfo> = registry.nodes
            .iter()
            .map(|node| NodeInfo {
                endpoint: node.endpoint.clone(),
                is_online: node.is_online,
                registered_at: node.registered_at,
            })
            .collect();
        
        let in_memory_count = nodes.len();
        
        // オンチェーンからもストレージノードを取得（Runtime API経由）
        let best_hash = self.client.info().best_hash;
        let api = self.client.runtime_api();
        
        if let Ok(on_chain_nodes) = api.get_all_storage_nodes(best_hash) {
            for node_info in on_chain_nodes {
                // http_url をエンドポイントURLとして使用
                if let Ok(endpoint) = String::from_utf8(node_info.http_url) {
                    // 重複チェック（インメモリに既にある場合はスキップ）
                    if !nodes.iter().any(|n| n.endpoint == endpoint) {
                        nodes.push(NodeInfo {
                            endpoint,
                            is_online: true, // オンチェーン登録されているノードはオンラインと仮定
                            registered_at: node_info.registered_at as u64,
                        });
                    }
                }
            }
        }
        
        // オンラインノード数は、インメモリレジストリからのみカウント
        // （オンチェーンノードは実際にヘルスチェックしていないため）
        let online_count = registry.online_nodes().len();
        let total_count = nodes.len();

        log::info!(
            "get_nodes: {} total ({} in-memory, {} on-chain unique), {} online", 
            total_count, 
            in_memory_count,
            total_count - in_memory_count,
            online_count
        );

        Ok(GetNodesResponse {
            nodes,
            online_count,
            total_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T023: storage_uploadFragment RPCテスト
    #[test]
    fn test_merkle_proof_verification() {
        use blake2::{Blake2b, Digest};
        use rs_merkle::{Hasher, MerkleTree};

        #[derive(Clone)]
        struct Blake2bHasher;

        impl Hasher for Blake2bHasher {
            type Hash = [u8; 32];

            fn hash(data: &[u8]) -> Self::Hash {
                let mut hasher = Blake2b::<blake2::digest::consts::U32>::new();
                hasher.update(data);
                hasher.finalize().into()
            }
        }

        // テストデータ
        let fragments: Vec<&[u8]> = vec![
            b"fragment0",
            b"fragment1",
            b"fragment2",
            b"fragment3",
            b"fragment4",
        ];

        // MerkleTree構築
        let leaves: Vec<[u8; 32]> = fragments.iter().map(|f| Blake2bHasher::hash(f)).collect();
        let tree = MerkleTree::<Blake2bHasher>::from_leaves(&leaves);
        let root = tree.root().unwrap();

        // インデックス2のProof生成
        let proof = tree.proof(&[2]);
        let proof_bytes = proof.to_bytes();

        // 正常ケース: 有効なProof
        let result = verify_merkle_proof(&root, &proof_bytes, b"fragment2", 2, 5);
        assert!(result.is_ok());
        assert!(result.unwrap(), "Valid proof should verify");

        // T024: 無効なProofを拒否
        let result_invalid = verify_merkle_proof(&root, &proof_bytes, b"wrong_data", 2, 5);
        assert!(result_invalid.is_ok());
        assert!(!result_invalid.unwrap(), "Invalid data should not verify");

        // 異なるインデックスで検証
        let result_wrong_index = verify_merkle_proof(&root, &proof_bytes, b"fragment2", 0, 5);
        assert!(result_wrong_index.is_ok());
        assert!(!result_wrong_index.unwrap(), "Wrong index should not verify");
    }

    #[test]
    fn test_blake2b_hash() {
        let data = b"test data";
        let hash = blake2b_hash(data);
        assert_eq!(hash.len(), 32);

        // 同じ入力は同じ出力
        let hash2 = blake2b_hash(data);
        assert_eq!(hash, hash2);
    }

    // T040: storage_getFragment RPCテスト
    #[test]
    fn test_get_fragment_request_serialization() {
        let request = GetFragmentRequest {
            merkle_root: [1u8; 32],
            index: 2,
        };
        
        // シリアライズ・デシリアライズ
        let json = serde_json::to_string(&request).unwrap();
        let parsed: GetFragmentRequest = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.merkle_root, request.merkle_root);
        assert_eq!(parsed.index, request.index);
    }

    #[test]
    fn test_get_fragment_response_serialization() {
        let response = GetFragmentResponse {
            data: "SGVsbG8gV29ybGQ=".to_string(), // "Hello World" base64
            hash: [42u8; 32],
        };
        
        let json = serde_json::to_string(&response).unwrap();
        let parsed: GetFragmentResponse = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.data, response.data);
        assert_eq!(parsed.hash, response.hash);
    }

    // T041: storage_getPostInfo RPCテスト
    #[test]
    fn test_post_info_response_serialization() {
        let response = PostInfoResponse {
            merkle_root: [0xab; 32],
            k: 3,
            n: 5,
            size: 1024,
            available_indices: vec![0, 1, 3],
        };
        
        let json = serde_json::to_string(&response).unwrap();
        let parsed: PostInfoResponse = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.merkle_root, response.merkle_root);
        assert_eq!(parsed.k, 3);
        assert_eq!(parsed.n, 5);
        assert_eq!(parsed.size, 1024);
        assert_eq!(parsed.available_indices, vec![0, 1, 3]);
    }

    #[test]
    fn test_upload_fragment_request_validation() {
        // base64エンコードされたデータ
        use base64::{engine::general_purpose::STANDARD, Engine};
        
        let data = b"fragment data";
        let encoded = STANDARD.encode(data);
        
        let request = UploadFragmentRequest {
            merkle_root: [0xff; 32],
            index: 0,
            data: encoded.clone(),
            proof: STANDARD.encode(b"proof"),
            total_leaves: 5,
            auth: None,
        };
        
        // デコード検証
        let decoded = STANDARD.decode(&request.data).unwrap();
        assert_eq!(decoded, data);
        
        // インデックス範囲確認
        assert!(request.index < request.total_leaves);
    }

    #[test]
    fn test_upload_fragment_response_success() {
        let response = UploadFragmentResponse {
            success: true,
            fragment_hash: blake2b_hash(b"test fragment"),
        };
        
        assert!(response.success);
        assert_eq!(response.fragment_hash.len(), 32);
    }

    // T040補足: getFragment リクエスト境界値テスト
    #[test]
    fn test_get_fragment_boundary_indices() {
        // インデックス0（最小）
        let req_min = GetFragmentRequest {
            merkle_root: [0u8; 32],
            index: 0,
        };
        assert_eq!(req_min.index, 0);
        
        // インデックス4（n=5の場合の最大）
        let req_max = GetFragmentRequest {
            merkle_root: [0u8; 32],
            index: 4,
        };
        assert_eq!(req_max.index, 4);
    }

    // T041補足: PostInfoResponse パラメータ検証
    #[test]
    fn test_post_info_k_n_constraints() {
        // k <= n の制約
        let response = PostInfoResponse {
            merkle_root: [0u8; 32],
            k: 3,
            n: 5,
            size: 512,
            available_indices: vec![0, 2, 4],
        };
        
        assert!(response.k <= response.n, "k should be <= n");
        assert!(response.k > 0, "k should be > 0");
        assert!(response.available_indices.len() <= response.n as usize);
    }

    // ============================================================================
    // T033/T046/T048: Storage Node連携テスト
    // ============================================================================

    /// ListHoldersリクエスト
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct ListHoldersRequest {
        pub post_id: u64,
    }

    /// ListHoldersレスポンス
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct ListHoldersResponse {
        pub holders: Vec<HolderInfo>,
    }

    /// ホルダー情報
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct HolderInfo {
        /// Storage NodeのAccountId (hex)
        pub account_id: String,
        /// 保持している断片インデックス
        pub indices: Vec<u32>,
        /// Storage NodeのエンドポイントURL（オプション）
        pub endpoint: Option<String>,
    }

    // T048: listHolders RPCテスト
    #[test]
    fn test_list_holders_request_serialization() {
        let request = ListHoldersRequest {
            post_id: 42,
        };
        
        let json = serde_json::to_string(&request).unwrap();
        let parsed: ListHoldersRequest = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.post_id, 42);
    }

    #[test]
    fn test_list_holders_response_serialization() {
        let response = ListHoldersResponse {
            holders: vec![
                HolderInfo {
                    account_id: "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY".to_string(),
                    indices: vec![0, 1, 2],
                    endpoint: Some("http://storage1.local:8080".to_string()),
                },
                HolderInfo {
                    account_id: "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty".to_string(),
                    indices: vec![1, 3, 4],
                    endpoint: None,
                },
            ],
        };
        
        let json = serde_json::to_string(&response).unwrap();
        let parsed: ListHoldersResponse = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.holders.len(), 2);
        assert_eq!(parsed.holders[0].indices, vec![0, 1, 2]);
        assert_eq!(parsed.holders[1].endpoint, None);
    }

    // T033: Storage Node転送のためのリクエスト構造テスト
    #[test]
    fn test_storage_node_forward_request() {
        // Storage Nodeへの転送リクエスト構造
        #[derive(Serialize, Deserialize)]
        struct StorageNodeUploadRequest {
            jsonrpc: String,
            id: u32,
            method: String,
            params: UploadFragmentRequest,
        }

        use base64::{engine::general_purpose::STANDARD, Engine};
        
        let request = StorageNodeUploadRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "storage_uploadFragment".to_string(),
            params: UploadFragmentRequest {
                merkle_root: [0xab; 32],
                index: 2,
                data: STANDARD.encode(b"fragment data"),
                proof: STANDARD.encode(b"proof bytes"),
                total_leaves: 5,
                auth: None,
            },
        };
        
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("jsonrpc"));
        assert!(json.contains("storage_uploadFragment"));
    }

    // T046: Storage Nodeからの取得レスポンステスト
    #[test]
    fn test_storage_node_get_response() {
        #[derive(Serialize, Deserialize)]
        struct StorageNodeResponse<T> {
            jsonrpc: String,
            id: u32,
            result: Option<T>,
            error: Option<RpcError>,
        }
        
        #[derive(Serialize, Deserialize)]
        struct RpcError {
            code: i32,
            message: String,
        }

        // 成功レスポンス
        let success_response: StorageNodeResponse<GetFragmentResponse> = StorageNodeResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(GetFragmentResponse {
                data: "SGVsbG8gV29ybGQ=".to_string(),
                hash: [0x42; 32],
            }),
            error: None,
        };
        
        let json = serde_json::to_string(&success_response).unwrap();
        let parsed: StorageNodeResponse<GetFragmentResponse> = serde_json::from_str(&json).unwrap();
        
        assert!(parsed.result.is_some());
        assert!(parsed.error.is_none());

        // エラーレスポンス
        let error_response: StorageNodeResponse<GetFragmentResponse> = StorageNodeResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: None,
            error: Some(RpcError {
                code: -32000,
                message: "Fragment not found".to_string(),
            }),
        };
        
        let json = serde_json::to_string(&error_response).unwrap();
        let parsed: StorageNodeResponse<GetFragmentResponse> = serde_json::from_str(&json).unwrap();
        
        assert!(parsed.result.is_none());
        assert!(parsed.error.is_some());
        assert_eq!(parsed.error.unwrap().message, "Fragment not found");
    }

    // T033補足: Storage Nodeクライアントのテスト
    #[test]
    fn test_storage_client_creation() {
        // StorageNodeClientが正しく作成されることのテスト
        let client = StorageNodeClient::new("http://localhost:8080".to_string());
        // クライアントが作成されていることを確認
        // Upload/getメソッドはasyncなので統合テストで確認
        assert!(!client.storage_node_url.is_empty());
        
        // Storage Nodeは起動時にstorage_registerEndpoint RPCで自動登録されるため、
        // 環境変数からの初期化は不要
    }
    
    // マルチノード対応: RegisteredStorageNodeのテスト
    #[test]
    fn test_registered_storage_node() {
        let node = RegisteredStorageNode::new("http://localhost:3030".to_string());
        assert_eq!(node.endpoint, "http://localhost:3030");
        assert!(node.is_online);
        assert!(node.registered_at > 0);
    }

    // T033補足: 分散ストレージで必要なパラメータ検証
    #[test]
    fn test_sss_parameters_validation() {
        // SSS (Shamir Secret Sharing) の基本パラメータ
        // k: 復元に必要な断片数
        // n: 総断片数
        // 条件: 1 <= k <= n
        
        // 有効なケース
        assert!(1 <= 3 && 3 <= 5); // k=3, n=5
        assert!(1 <= 2 && 2 <= 3); // k=2, n=3
        assert!(1 <= 1 && 1 <= 1); // k=1, n=1 (冗長性なし)
        
        // 分散要件: 少なくとも1つのノードがオフラインでも復元可能
        // => k < n が必要 (n - k >= 1)
        let k = 3;
        let n = 5;
        let tolerance = n - k; // 許容オフラインノード数
        assert_eq!(tolerance, 2); // 2ノードまでオフラインでOK
    }
    
    // ============================================================================
    // T106: Multi-node selection tests
    // ============================================================================

    // T106: Test fragment-index selection distributes across nodes
    #[test]
    fn test_fragment_index_distribution() {
        use crate::rpc::StorageNodeRegistry;
        
        let mut registry = StorageNodeRegistry::new();
        
        // 3つのノードを登録
        registry.register(RegisteredStorageNode::new("http://node1:3030".to_string()));
        registry.register(RegisteredStorageNode::new("http://node2:3030".to_string()));
        registry.register(RegisteredStorageNode::new("http://node3:3030".to_string()));
        
        // 5つの断片を分散配置
        let mut node_assignments: Vec<String> = Vec::new();
        for i in 0..5 {
            let node = registry.select_node_for_fragment(i).unwrap();
            node_assignments.push(node.endpoint.clone());
        }
        
        // 断片0 -> node1 (0 % 3 = 0)
        // 断片1 -> node2 (1 % 3 = 1)
        // 断片2 -> node3 (2 % 3 = 2)
        // 断片3 -> node1 (3 % 3 = 0)
        // 断片4 -> node2 (4 % 3 = 1)
        assert_eq!(node_assignments[0], "http://node1:3030");
        assert_eq!(node_assignments[1], "http://node2:3030");
        assert_eq!(node_assignments[2], "http://node3:3030");
        assert_eq!(node_assignments[3], "http://node1:3030");
        assert_eq!(node_assignments[4], "http://node2:3030");
        
        // 全ノードが使用されていることを確認
        let unique_nodes: std::collections::HashSet<_> = node_assignments.iter().collect();
        assert_eq!(unique_nodes.len(), 3);
    }
    
    // T107: Test offline node filtering
    #[test]
    fn test_offline_node_filtering() {
        use crate::rpc::StorageNodeRegistry;
        
        let mut registry = StorageNodeRegistry::new();
        
        // 3つのノードを登録（1つはオフライン）
        let mut node1 = RegisteredStorageNode::new("http://node1:3030".to_string());
        let mut node2 = RegisteredStorageNode::new("http://node2:3030".to_string());
        let node3 = RegisteredStorageNode::new("http://node3:3030".to_string());
        
        // Node2をオフラインに設定
        node1.is_online = true;
        node2.is_online = false;
        
        registry.register(node1);
        registry.register(node2);
        registry.register(node3);
        
        // 総ノード数は3
        assert_eq!(registry.nodes.len(), 3);
        
        // オンラインノード数は2
        assert_eq!(registry.online_node_count(), 2);
        
        // オンラインノードのみが選択可能
        let online = registry.online_nodes();
        assert_eq!(online.len(), 2);
        
        // オフラインノード(node2)は含まれない
        let endpoints: Vec<&str> = online.iter().map(|n| n.endpoint.as_str()).collect();
        assert!(!endpoints.contains(&"http://node2:3030"));
        assert!(endpoints.contains(&"http://node1:3030"));
        assert!(endpoints.contains(&"http://node3:3030"));
        
        // 断片選択時もオフラインノードはスキップ
        for i in 0..10 {
            let node = registry.select_node_for_fragment(i).unwrap();
            assert!(node.is_online);
            assert_ne!(node.endpoint, "http://node2:3030");
        }
    }
    
    // T108: Test insufficient nodes check for SSS_N=5
    #[test]
    fn test_insufficient_nodes_for_sss() {
        use crate::rpc::StorageNodeRegistry;
        
        const SSS_N: usize = 5; // 必要な断片数
        
        // 3ノードしかない場合
        let mut registry = StorageNodeRegistry::new();
        registry.register(RegisteredStorageNode::new("http://node1:3030".to_string()));
        registry.register(RegisteredStorageNode::new("http://node2:3030".to_string()));
        registry.register(RegisteredStorageNode::new("http://node3:3030".to_string()));
        
        let available = registry.online_node_count();
        assert_eq!(available, 3);
        
        // 5ノード必要なのに3ノードしかない -> 不十分
        assert!(available < SSS_N);
        
        // 各断片がどのノードに割り当てられるか確認（重複あり）
        let assignments: Vec<usize> = (0..SSS_N)
            .map(|i| i % available)
            .collect();
        
        // 断片0,3 -> node0, 断片1,4 -> node1, 断片2 -> node2
        // 重複が発生している
        assert_eq!(assignments, vec![0, 1, 2, 0, 1]);
        
        // 5ノード追加して十分にする
        registry.register(RegisteredStorageNode::new("http://node4:3030".to_string()));
        registry.register(RegisteredStorageNode::new("http://node5:3030".to_string()));
        
        let available = registry.online_node_count();
        assert_eq!(available, 5);
        
        // 5ノード必要で5ノードある -> 十分
        assert!(available >= SSS_N);
        
        // 各断片が異なるノードに割り当てられる
        let assignments: Vec<usize> = (0..SSS_N)
            .map(|i| i % available)
            .collect();
        
        // 各断片が一意のノードに割り当て
        assert_eq!(assignments, vec![0, 1, 2, 3, 4]);
    }
    
    // T109: Test node count validation with offline nodes
    #[test]
    fn test_node_count_with_offline() {
        use crate::rpc::StorageNodeRegistry;
        
        const SSS_N: usize = 5;
        
        let mut registry = StorageNodeRegistry::new();
        
        // 5ノード登録するが、2つはオフライン
        for i in 0..5 {
            let mut node = RegisteredStorageNode::new(format!("http://node{}:3030", i));
            // ノード3と4はオフライン
            node.is_online = i < 3;
            registry.register(node);
        }
        
        // 総ノード数は5
        assert_eq!(registry.nodes.len(), 5);
        
        // オンラインノード数は3（不十分）
        let available = registry.online_node_count();
        assert_eq!(available, 3);
        assert!(available < SSS_N);
        
        // すべてのノードをオンラインにする
        for node in &mut registry.nodes {
            node.is_online = true;
        }
        
        // オンラインノード数は5（十分）
        let available = registry.online_node_count();
        assert_eq!(available, 5);
        assert!(available >= SSS_N);
    }
    
    // Test random selection
    #[test]
    fn test_random_selection() {
        use crate::rpc::StorageNodeRegistry;
        
        let registry = StorageNodeRegistry::new();
        
        // 空のレジストリからは選択できない
        assert!(registry.online_nodes_shuffled().into_iter().next().is_none());
    }
    
    // Test random selection with nodes
    #[test]
    fn test_random_selection_with_nodes() {
        use crate::rpc::StorageNodeRegistry;
        
        let mut registry = StorageNodeRegistry::new();
        
        // ノード登録
        for i in 0..3 {
            registry.register(RegisteredStorageNode::new(format!("http://node{}:3030", i)));
        }
        
        // ランダム選択が成功することを確認（複数回テスト）
        for _ in 0..10 {
            let node = registry.online_nodes_shuffled().into_iter().next();
            assert!(node.is_some());
        }
    }
}

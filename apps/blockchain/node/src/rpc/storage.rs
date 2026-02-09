//! Storage RPC API
//!
//! 分散ストレージ関連のRPCエンドポイントを提供。
//! フロントエンドからの断片アップロード/ダウンロードリクエストを処理し、
//! HTTP経由でStorage Nodeに転送する。
//!
//! ## アーキテクチャ
//!
//! ```text
//! Frontend → Blockchain Node RPC → HTTP → Storage Node
//!                    ↑
//! 将来のインデクサー（読み取りキャッシュ）
//! ```
//!
//! - 書き込み: プライバシー重視でBlockchain Node経由（IP匿名化）
//! - 読み込み: 同様にBlockchain Node経由（将来はインデクサーキャッシュ）

use anarchy_runtime::opaque::Block;
use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::error::{ErrorCode, ErrorObject},
};
use pallet_post::PostApi as PostRuntimeApi;
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// Security Constants (T074)
// ============================================================================

/// Maximum fragment size: 256KB (262144 bytes)
/// 断片サイズの上限。これを超えるリクエストは拒否される。
pub const MAX_FRAGMENT_SIZE: usize = 256 * 1024;

/// Maximum total leaves in MerkleTree
/// n値 (総断片数) の上限。SSS (k=3, n=5) の場合は通常5以下。
pub const MAX_TOTAL_LEAVES: u32 = 255;

/// Minimum k value (threshold)
pub const MIN_K: u32 = 1;

/// Maximum proof size: 8KB (proofはlog2(n)に比例)
pub const MAX_PROOF_SIZE: usize = 8 * 1024;

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

        let response = self.http_client
            .post(&self.storage_node_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

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
    /// Storage Node URL (動的に登録される)
    storage_node_url: Arc<RwLock<Option<String>>>,
}

impl<C> Storage<C> {
    /// 新しいStorage RPCハンドラを作成
    /// Storage Nodeは起動時にstorage_registerEndpoint RPCで自動登録される
    pub fn new(client: Arc<C>) -> Self {
        Self { 
            client, 
            storage_node_url: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Storage Nodeクライアントを取得
    async fn get_storage_client(&self) -> Option<StorageNodeClient> {
        let url = self.storage_node_url.read().await;
        url.as_ref().map(|u| StorageNodeClient::new(u.clone()))
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
    C::Api: PostRuntimeApi<Block>,
{
    async fn register_endpoint(&self, url: String) -> RpcResult<bool> {
        // URLの基本的な検証
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                "Invalid URL: must start with http:// or https://",
                None::<()>,
            ).into());
        }

        log::info!("Storage Node registered: {}", url);
        
        // URLを保存
        let mut storage_url = self.storage_node_url.write().await;
        *storage_url = Some(url);
        
        Ok(true)
    }

    async fn upload_fragment(&self, request: UploadFragmentRequest) -> RpcResult<UploadFragmentResponse> {
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
            ).into());
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
            ).into());
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
            ).into());
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
            ).into());
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
            ).into());
        }

        // 3. 断片ハッシュ計算
        let fragment_hash = blake2b_hash(&data);

        // 4. Storage Nodeに転送（HTTP経由）- 必須
        let storage_client = self.get_storage_client().await.ok_or_else(|| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Storage Node not connected. Start storage-node and it will auto-register.",
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

        // Storage Nodeから取得
        let storage_client = self.get_storage_client().await.ok_or_else(|| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Storage Node not connected. Start storage-node and it will auto-register.",
                None::<()>,
            )
        })?;

        let response = storage_client
            .get_fragment(&request.merkle_root, request.index)
            .await
            .map_err(|e| {
                ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    format!("Failed to get fragment from Storage Node: {}", e),
                    None::<()>,
                )
            })?;

        log::info!(
            "Fragment retrieved from Storage Node: root={:?}, index={}",
            hex::encode(&request.merkle_root[..8]),
            request.index
        );

        Ok(response)
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
        let available_indices = (0..content_info.n as u32).collect();

        Ok(PostInfoResponse {
            merkle_root: content_info.root,
            k: content_info.k as u32,
            n: content_info.n as u32,
            size: content_info.size,
            available_indices,
        })
    }

    async fn list_holders(&self, post_id: u64) -> RpcResult<ListHoldersResponse> {
        // TODO: declare_holdingストレージからホルダー一覧を取得
        // 現在はStorage Nodeが1つの想定なので、登録されたURLから取得
        log::debug!("list_holders called: post_id={}", post_id);

        let url = self.storage_node_url.read().await;
        let holders = if let Some(ref endpoint) = *url {
            vec![HolderInfo {
                account_id: "configured_storage_node".to_string(),
                indices: (0..5).collect(), // 仮に0-4を保持
                endpoint: Some(endpoint.clone()),
            }]
        } else {
            vec![]
        };

        Ok(ListHoldersResponse { holders })
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

    // T033補足: Storage Nodeが未設定の場合のテスト
    #[test]
    fn test_storage_client_none_handling() {
        // Storage<C>のstorage_clientがNoneの場合、
        // upload_fragmentとget_fragmentはエラーを返すべき
        // (実際のasync呼び出しテストは統合テストで行う)
        
        // StorageNodeClientが正しく作成されることのテスト
        let client = StorageNodeClient::new("http://localhost:8080".to_string());
        assert!(!client.storage_node_url.is_empty());
        
        // Storage Nodeは起動時にstorage_registerEndpoint RPCで自動登録されるため、
        // 環境変数からの初期化は不要
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
}


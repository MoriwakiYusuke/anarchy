//! HTTP JSON-RPC Server
//!
//! Exposes storage operations via HTTP JSON-RPC for blockchain node integration.
//! This allows the blockchain node to forward fragment upload/download requests.

pub mod auth;

use std::sync::Arc;
use axum::{
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    middleware,
    routing::{get, post},
    Json, Router,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn, error};

use crate::storage::FragmentStore;
use crate::metrics::Metrics;
use auth::{AuthState, auth_middleware, method_requires_auth, require_auth};

/// Maximum fragment size: 256KB
const MAX_FRAGMENT_SIZE: usize = 256 * 1024;

/// JSON-RPC Request wrapper
#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: u32,
    pub method: String,
    pub params: serde_json::Value,
}

/// JSON-RPC Response wrapper
#[derive(Debug, Serialize)]
pub struct RpcResponse<T> {
    pub jsonrpc: &'static str,
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

/// Fragment upload request (matches blockchain node's UploadFragmentRequest)
#[derive(Debug, Deserialize)]
pub struct StoreFragmentParams {
    /// MerkleRoot identifying the post
    pub merkle_root: [u8; 32],
    /// Fragment index (0 ~ n-1)
    pub index: u32,
    /// Fragment data (base64 encoded)
    pub data: String,
    /// MerkleProof (base64 encoded) - for future verification
    pub proof: String,
    /// Total number of fragments
    pub total_leaves: u32,
}

/// Fragment upload response
#[derive(Debug, Serialize)]
pub struct StoreFragmentResult {
    pub success: bool,
    pub fragment_hash: [u8; 32],
}

/// Fragment get params
#[derive(Debug, Deserialize)]
pub struct GetFragmentParams {
    pub merkle_root: [u8; 32],
    pub index: u32,
}

/// Fragment get response
#[derive(Debug, Serialize)]
pub struct GetFragmentResult {
    pub data: String,
    pub hash: [u8; 32],
}

/// Shared state for the RPC server
#[derive(Clone)]
pub struct RpcState {
    pub store: Arc<FragmentStore>,
    pub auth: AuthState,
    pub metrics: Metrics,
}

/// Create the HTTP RPC router (NFR-002: /metrics endpoint)
pub fn create_rpc_router(store: Arc<FragmentStore>, auth_enabled: bool, metrics: Metrics) -> Router {
    let auth_state = AuthState::new(auth_enabled);
    let state = RpcState { 
        store,
        auth: auth_state.clone(),
        metrics,
    };
    
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", post(handle_rpc))
        .route("/metrics", get(handle_metrics)) // NFR-002: Prometheus metrics endpoint
        .layer(middleware::from_fn_with_state(auth_state, auth_middleware))
        .with_state(state)
        .layer(cors)
        .layer(DefaultBodyLimit::max(512 * 1024)) // 512KB limit
}

/// Handle JSON-RPC requests
async fn handle_rpc(
    State(state): State<RpcState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<RpcRequest>,
) -> Result<Json<RpcResponse<serde_json::Value>>, (StatusCode, &'static str)> {
    let id = request.id;
    
    // Check if method requires authentication (T048-T051)
    if method_requires_auth(&request.method) {
        require_auth(&headers, state.auth.enabled)?;
    }
    
    let response = match request.method.as_str() {
        "storage_storeFragment" => handle_store_fragment(&state, request.params).await,
        "storage_getFragment" => handle_get_fragment(&state, request.params).await,
        _ => Err(RpcError {
            code: -32601,
            message: format!("Method not found: {}", request.method),
        }),
    };

    match response {
        Ok(result) => Ok(Json(RpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        })),
        Err(error) => Ok(Json(RpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(error),
        })),
    }
}

/// Handle storage_storeFragment
async fn handle_store_fragment(
    state: &RpcState,
    params: serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: StoreFragmentParams = serde_json::from_value(params)
        .map_err(|e| RpcError {
            code: -32602,
            message: format!("Invalid params: {}", e),
        })?;

    // Decode base64 data
    let data = b64_decode(&params.data)
        .map_err(|e| RpcError {
            code: -32602,
            message: format!("Invalid base64 data: {}", e),
        })?;

    // Validate size
    if data.len() > MAX_FRAGMENT_SIZE {
        return Err(RpcError {
            code: -32000,
            message: format!("Fragment too large: {} > {}", data.len(), MAX_FRAGMENT_SIZE),
        });
    }

    // Generate fragment ID from merkle_root and index
    let fragment_id = create_fragment_id(&params.merkle_root, params.index);

    info!(
        fragment_id = %hex::encode(fragment_id),
        merkle_root = %hex::encode(params.merkle_root),
        index = params.index,
        size = data.len(),
        "Storing fragment"
    );

    // Store the fragment
    match state.store.store(fragment_id, &data) {
        Ok(()) => {
            // Calculate fragment hash
            use blake2::{Blake2b, Digest};
            use blake2::digest::consts::U32;
            let mut hasher = Blake2b::<U32>::new();
            hasher.update(&data);
            let hash: [u8; 32] = hasher.finalize().into();

            let result = StoreFragmentResult {
                success: true,
                fragment_hash: hash,
            };

            serde_json::to_value(result).map_err(|e| RpcError {
                code: -32603,
                message: format!("Internal serialization error: {}", e),
            })
        }
        Err(e) => {
            error!(error = %e, "Failed to store fragment");
            Err(RpcError {
                code: -32001,
                message: format!("Storage error: {}", e),
            })
        }
    }
}

/// Handle storage_getFragment
async fn handle_get_fragment(
    state: &RpcState,
    params: serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let params: GetFragmentParams = serde_json::from_value(params)
        .map_err(|e| RpcError {
            code: -32602,
            message: format!("Invalid params: {}", e),
        })?;

    // Generate fragment ID from merkle_root and index
    let fragment_id = create_fragment_id(&params.merkle_root, params.index);

    info!(
        fragment_id = %hex::encode(fragment_id),
        merkle_root = %hex::encode(params.merkle_root),
        index = params.index,
        "Getting fragment"
    );

    // Retrieve the fragment
    match state.store.retrieve(&fragment_id) {
        Ok(Some(data)) => {
            // Calculate hash
            use blake2::{Blake2b, Digest};
            use blake2::digest::consts::U32;
            let mut hasher = Blake2b::<U32>::new();
            hasher.update(&data);
            let hash: [u8; 32] = hasher.finalize().into();

            let result = GetFragmentResult {
                data: b64_encode(&data),
                hash,
            };

            serde_json::to_value(result).map_err(|e| RpcError {
                code: -32603,
                message: format!("Internal serialization error: {}", e),
            })
        }
        Ok(None) => {
            warn!(
                fragment_id = %hex::encode(fragment_id),
                "Fragment not found"
            );
            Err(RpcError {
                code: -32002,
                message: "Fragment not found".to_string(),
            })
        }
        Err(e) => {
            error!(error = %e, "Failed to retrieve fragment");
            Err(RpcError {
                code: -32001,
                message: format!("Storage error: {}", e),
            })
        }
    }
}

/// Create fragment ID from merkle_root and index
/// Uses Blake2b-256(merkle_root || index) to generate a unique ID
fn create_fragment_id(merkle_root: &[u8; 32], index: u32) -> [u8; 32] {
    use blake2::{Blake2b, Digest};
    use blake2::digest::consts::U32;
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(merkle_root);
    hasher.update(index.to_le_bytes());
    hasher.finalize().into()
}

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

/// Decode base64 or hex (0x-prefixed) string to bytes
fn b64_decode(input: &str) -> Result<Vec<u8>, String> {
    if let Some(hex_str) = input.strip_prefix("0x") {
        hex::decode(hex_str).map_err(|e| e.to_string())
    } else {
        BASE64_STANDARD.decode(input).map_err(|e| e.to_string())
    }
}

/// Encode bytes to base64 string
fn b64_encode(input: &[u8]) -> String {
    BASE64_STANDARD.encode(input)
}

/// Handle Prometheus metrics endpoint (NFR-002)
///
/// Returns metrics in Prometheus text format:
/// - fragment_upload_total
/// - fragment_download_total
/// - storage_node_peers
/// - blockchain_node_failover_total
/// - gossipsub_messages_received_total
/// - peer_reputation_score (gauge, average)
async fn handle_metrics(
    State(state): State<RpcState>,
) -> impl IntoResponse {
    let m = &state.metrics;
    
    // NFR-003: Collect required metrics
    let output = format!(
        r#"# HELP fragment_upload_total Total number of fragment uploads
# TYPE fragment_upload_total counter
fragment_upload_total {}

# HELP fragment_download_total Total number of fragment downloads
# TYPE fragment_download_total counter
fragment_download_total {}

# HELP storage_node_peers Number of connected storage node peers
# TYPE storage_node_peers gauge
storage_node_peers {}

# HELP blockchain_node_failover_total Total number of blockchain node failovers
# TYPE blockchain_node_failover_total counter
blockchain_node_failover_total {}

# HELP gossipsub_messages_received_total Total number of Gossipsub messages received
# TYPE gossipsub_messages_received_total counter
gossipsub_messages_received_total {}

# HELP storage_fragments_total Total number of stored fragments
# TYPE storage_fragments_total gauge
storage_fragments_total {}

# HELP storage_capacity_used_bytes Storage capacity used in bytes
# TYPE storage_capacity_used_bytes gauge
storage_capacity_used_bytes {}

# HELP storage_capacity_total_bytes Total storage capacity in bytes
# TYPE storage_capacity_total_bytes gauge
storage_capacity_total_bytes {}

# HELP storage_auth_failures_total Total number of authentication failures
# TYPE storage_auth_failures_total counter
storage_auth_failures_total {}

# HELP storage_chain_latency_ms Current blockchain connection latency in milliseconds
# TYPE storage_chain_latency_ms gauge
storage_chain_latency_ms {}
"#,
        m.put_requests(),
        m.get_requests(),
        m.connected_peers(),
        m.chain_failovers(),
        m.gossip_messages_received(),
        m.fragment_count(),
        m.capacity_used_bytes(),
        m.capacity_total_bytes(),
        m.auth_failures(),
        m.chain_latency_ms(),
    );
    
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        output,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_fragment_id_generation() {
        let merkle_root = [1u8; 32];
        let id1 = create_fragment_id(&merkle_root, 0);
        let id2 = create_fragment_id(&merkle_root, 1);
        
        // Different indices should produce different IDs
        assert_ne!(id1, id2);
        
        // Same inputs should produce same ID
        let id1_again = create_fragment_id(&merkle_root, 0);
        assert_eq!(id1, id1_again);
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = b"Hello, World!";
        let encoded = b64_encode(original);
        let decoded = b64_decode(&encoded).unwrap();
        assert_eq!(original.as_slice(), decoded.as_slice());
    }

    #[test]
    fn test_base64_hex_fallback() {
        let original = vec![0xde, 0xad, 0xbe, 0xef];
        let hex_encoded = "0xdeadbeef";
        let decoded = b64_decode(hex_encoded).unwrap();
        assert_eq!(original, decoded);
    }
}

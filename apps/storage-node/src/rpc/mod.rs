//! HTTP JSON-RPC Server
//!
//! Exposes storage operations via HTTP JSON-RPC for blockchain node integration.
//! This allows the blockchain node to forward fragment upload/download requests.

use std::sync::Arc;
use axum::{
    extract::State,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn, error};

use crate::storage::FragmentStore;

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
}

/// Create the HTTP RPC router
pub fn create_rpc_router(store: Arc<FragmentStore>) -> Router {
    let state = RpcState { store };
    
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", post(handle_rpc))
        .with_state(state)
        .layer(cors)
}

/// Handle JSON-RPC requests
async fn handle_rpc(
    State(state): State<RpcState>,
    Json(request): Json<RpcRequest>,
) -> Json<RpcResponse<serde_json::Value>> {
    let id = request.id;
    
    let response = match request.method.as_str() {
        "storage_storeFragment" => handle_store_fragment(&state, request.params).await,
        "storage_getFragment" => handle_get_fragment(&state, request.params).await,
        _ => Err(RpcError {
            code: -32601,
            message: format!("Method not found: {}", request.method),
        }),
    };

    match response {
        Ok(result) => Json(RpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }),
        Err(error) => Json(RpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(error),
        }),
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
    let data = base64::decode(&params.data)
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
        fragment_id = %hex::encode(&fragment_id),
        merkle_root = %hex::encode(&params.merkle_root),
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

            Ok(serde_json::to_value(result).unwrap())
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
        fragment_id = %hex::encode(&fragment_id),
        merkle_root = %hex::encode(&params.merkle_root),
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
                data: base64::encode(&data),
                hash,
            };

            Ok(serde_json::to_value(result).unwrap())
        }
        Ok(None) => {
            warn!(
                fragment_id = %hex::encode(&fragment_id),
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
    hasher.update(&index.to_le_bytes());
    hasher.finalize().into()
}

// Note: base64 crate is used via serde_json compatibility
// but we need to add it explicitly
mod base64 {

    pub fn decode(input: &str) -> Result<Vec<u8>, String> {
        // Use data URL or hex fallback for compatibility
        if input.starts_with("0x") {
            hex::decode(&input[2..]).map_err(|e| e.to_string())
        } else {
            // Try standard base64
            data_encoding_decode(input)
        }
    }

    fn data_encoding_decode(input: &str) -> Result<Vec<u8>, String> {
        // Simple base64 decoder
        let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut result = Vec::new();
        let input = input.trim_end_matches('=');
        let bytes: Vec<u8> = input.bytes().collect();

        for chunk in bytes.chunks(4) {
            let mut buf = [0u8; 4];
            for (i, &b) in chunk.iter().enumerate() {
                buf[i] = table.iter().position(|&c| c == b)
                    .ok_or_else(|| format!("Invalid base64 char: {}", b as char))? as u8;
            }

            result.push((buf[0] << 2) | (buf[1] >> 4));
            if chunk.len() > 2 {
                result.push((buf[1] << 4) | (buf[2] >> 2));
            }
            if chunk.len() > 3 {
                result.push((buf[2] << 6) | buf[3]);
            }
        }

        Ok(result)
    }

    pub fn encode(input: &[u8]) -> String {
        const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut result = String::new();

        for chunk in input.chunks(3) {
            let b0 = chunk[0] as usize;
            let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
            let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

            result.push(TABLE[b0 >> 2] as char);
            result.push(TABLE[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
            
            if chunk.len() > 1 {
                result.push(TABLE[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
            } else {
                result.push('=');
            }
            
            if chunk.len() > 2 {
                result.push(TABLE[b2 & 0x3f] as char);
            } else {
                result.push('=');
            }
        }

        result
    }
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
        let encoded = base64::encode(original);
        let decoded = base64::decode(&encoded).unwrap();
        assert_eq!(original.as_slice(), decoded.as_slice());
    }

    #[test]
    fn test_base64_hex_fallback() {
        let original = vec![0xde, 0xad, 0xbe, 0xef];
        let hex_encoded = "0xdeadbeef";
        let decoded = base64::decode(hex_encoded).unwrap();
        assert_eq!(original, decoded);
    }
}

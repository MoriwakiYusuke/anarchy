//! Storage Node Discovery Gossip Protocol
//!
//! チェーンノード間でストレージノード情報をP2Pで共有するプロトコル。
//! storage_registerEndpoint RPCで登録されたノード情報を他チェーンノードに伝播。
//!
//! ## プロトコル
//!
//! ```text
//! Chain Node A                          Chain Node B
//!     │                                      │
//!     │ storage_registerEndpoint(url)        │
//!     │◄────────────────────────────────────►│
//!     │         Gossip broadcast             │
//!     │                                      │
//!     ▼                                      ▼
//! Registry(A)                           Registry(B)
//!   - node1                               - node1 (from A)
//! ```

use crate::rpc::storage::{verify_registration_signature, RegistrationProof, MAX_TIMESTAMP_SKEW_SECS};
use crate::rpc::{EndpointPolicy, RegisteredStorageNode, SharedStorageNodes};
use log::{debug, info, warn};
use parity_scale_codec::{Decode, Encode};
use sc_network::{
    config::SetConfig,
    peer_store::PeerStoreProvider,
    service::{traits::{NotificationEvent, NotificationService, ValidationResult}, NotificationMetrics},
    NetworkBackend, ProtocolName,
};
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

/// Maximum number of concurrent connections (Issue 6 fix - DoS prevention)
pub const MAX_CONNECTIONS: usize = 128;

/// Maximum registry size (Issue 7 fix - memory exhaustion prevention)
pub const MAX_REGISTRY_SIZE: usize = 10_000;

/// ストレージノードGossipプロトコル名
pub const STORAGE_NODE_PROTOCOL: &str = "/anarchy/storage-nodes/1";

/// ストレージノードGossipプロトコル設定を作成
///
/// GrandpaのようにNotificationServiceとプロトコル設定のペアを返す
pub fn storage_nodes_peers_set_config<
    B: BlockT,
    N: NetworkBackend<B, <B as BlockT>::Hash>,
>(
    metrics: NotificationMetrics,
    peer_store_handle: Arc<dyn PeerStoreProvider>,
) -> (N::NotificationProtocolConfig, Box<dyn NotificationService>) {
    N::notification_config(
        ProtocolName::from(STORAGE_NODE_PROTOCOL),
        vec![], // No fallback names
        64 * 1024, // 64KB max notification size
        None,   // No handshake
        SetConfig {
            in_peers: 25,
            out_peers: 25,
            reserved_nodes: Vec::new(),
            non_reserved_mode: sc_network::config::NonReservedPeerMode::Accept,
        },
        metrics,
        peer_store_handle,
    )
}

/// SyncResponse 1通あたりの最大処理ノード数 (DoS 対策)
///
/// エントリごとに署名検証 + URL 検証 (DNS 解決の可能性あり) を行うため、
/// 悪意あるピアが大量エントリで gossip ループを長時間占有するのを防ぐ。
pub const MAX_SYNC_NODES: usize = 256;

/// Gossipメッセージタイプ
///
/// (finding #2) NodeRegistered / SyncResponse は RPC 経路
/// (`storage_registerEndpoint`) と同一のオペレーター sr25519 署名を運び、
/// 受信側で検証する。互換性は考慮しない (全ノードが同一バイナリを実行)。
#[derive(Clone, Debug, Encode, Decode)]
pub enum StorageNodeGossipMessage {
    /// ノード登録通知
    NodeRegistered {
        /// エンドポイントURL
        endpoint: String,
        /// 登録時刻 (Unix timestamp)
        registered_at: u64,
        /// レイテンシ (ミリ秒、オプション)
        latency_ms: Option<u64>,
        /// オペレーター sr25519 公開鍵 (32バイト)
        operator: [u8; 32],
        /// 署名時の Unix タイムスタンプ（秒）
        timestamp: u64,
        /// sr25519 署名 (64バイト)。署名対象: "register_endpoint:{endpoint}:{timestamp}"
        signature: [u8; 64],
    },
    /// ノード一覧同期要求
    SyncRequest,
    /// ノード一覧同期応答
    SyncResponse {
        /// 全登録ノード
        nodes: Vec<GossipNodeInfo>,
    },
}

/// Gossip用ノード情報 (軽量版 + 登録証明)
#[derive(Clone, Debug, Encode, Decode)]
pub struct GossipNodeInfo {
    pub endpoint: String,
    pub registered_at: u64,
    pub latency_ms: Option<u64>,
    /// オペレーター sr25519 公開鍵
    pub operator: [u8; 32],
    /// 署名時の Unix タイムスタンプ（秒）
    pub timestamp: u64,
    /// sr25519 署名。署名対象: "register_endpoint:{endpoint}:{timestamp}"
    pub signature: [u8; 64],
}

impl GossipNodeInfo {
    /// 登録証明を持つノードのみ Gossip 中継対象にする。
    /// 証明のないノードは None (中継しない)。
    fn from_node(node: &RegisteredStorageNode) -> Option<Self> {
        let proof = node.registration_proof.as_ref()?;
        let signature: [u8; 64] = proof.signature.as_slice().try_into().ok()?;
        Some(Self {
            endpoint: node.endpoint.clone(),
            registered_at: node.registered_at,
            latency_ms: node.latency_ms,
            operator: proof.operator,
            timestamp: proof.timestamp,
            signature,
        })
    }
}

/// StorageNodeGossipサービス
pub struct StorageNodeGossip {
    /// 共有ストレージノードレジストリ
    storage_nodes: SharedStorageNodes,
    /// Notificationサービス
    notification_service: Box<dyn NotificationService>,
    /// ブロードキャストコマンド受信チャンネル
    broadcast_rx: tokio::sync::mpsc::UnboundedReceiver<BroadcastCommand>,
    /// 接続中のピア一覧
    connected_peers: std::collections::HashSet<sc_network::PeerId>,
    /// エンドポイント URL 検証ポリシー (SSRF 対策, finding #1)
    endpoint_policy: EndpointPolicy,
}

impl StorageNodeGossip {
    /// 新しいGossipサービスとハンドルを作成
    pub fn new_with_handle(
        storage_nodes: SharedStorageNodes,
        notification_service: Box<dyn NotificationService>,
        endpoint_policy: EndpointPolicy,
    ) -> (Self, StorageNodeGossipHandle) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = StorageNodeGossipHandle { tx };
        let service = Self {
            storage_nodes,
            notification_service,
            broadcast_rx: rx,
            connected_peers: std::collections::HashSet::new(),
            endpoint_policy,
        };
        (service, handle)
    }

    /// Gossipイベントループを実行
    pub async fn run(mut self) {
        info!("Storage Node Gossip service started");
        
        loop {
            tokio::select! {
                event = self.notification_service.next_event() => {
                    match event {
                        Some(NotificationEvent::NotificationReceived { peer, notification }) => {
                            self.handle_notification(peer, notification).await;
                        }
                        Some(NotificationEvent::ValidateInboundSubstream { result_tx, .. }) => {
                            // Issue 6 fix: Check connection limit to prevent DoS
                            if self.connected_peers.len() >= MAX_CONNECTIONS {
                                warn!("Connection limit reached ({}/{}), rejecting inbound connection",
                                    self.connected_peers.len(), MAX_CONNECTIONS);
                                let _ = result_tx.send(ValidationResult::Reject);
                            } else {
                                // Accept connection if under limit
                                let _ = result_tx.send(ValidationResult::Accept);
                            }
                        }
                        Some(NotificationEvent::NotificationStreamOpened { peer, .. }) => {
                            debug!("Gossip stream opened with peer: {:?}", peer);
                            self.connected_peers.insert(peer);
                            // 新しいピアに現在のノード一覧を送信
                            self.send_sync_response_to(&peer).await;
                        }
                        Some(NotificationEvent::NotificationStreamClosed { peer }) => {
                            debug!("Gossip stream closed with peer: {:?}", peer);
                            self.connected_peers.remove(&peer);
                        }
                        None => {
                            warn!("Notification service stream ended");
                            break;
                        }
                    }
                }
                cmd = self.broadcast_rx.recv() => {
                    match cmd {
                        Some(BroadcastCommand::BroadcastRegistration { endpoint, registered_at, latency_ms, proof }) => {
                            self.broadcast_node_registered(endpoint, registered_at, latency_ms, proof).await;
                        }
                        None => {
                            // チャンネルがクローズされた（全ハンドルがドロップ）
                            // breakしないとNoneが即座に返り続けてCPUを消費する
                            debug!("Broadcast channel closed");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// ノード登録を全ピアにブロードキャスト (オペレーター署名付き, finding #2)
    async fn broadcast_node_registered(
        &mut self,
        endpoint: String,
        registered_at: u64,
        latency_ms: Option<u64>,
        proof: RegistrationProof,
    ) {
        let signature: [u8; 64] = match proof.signature.as_slice().try_into() {
            Ok(s) => s,
            Err(_) => {
                warn!("Cannot broadcast registration for {}: invalid signature length", endpoint);
                return;
            }
        };
        let message = StorageNodeGossipMessage::NodeRegistered {
            endpoint: endpoint.clone(),
            registered_at,
            latency_ms,
            operator: proof.operator,
            timestamp: proof.timestamp,
            signature,
        };
        let encoded = message.encode();
        
        // 全接続ピアにブロードキャスト
        let peer_count = self.connected_peers.len();
        for peer in self.connected_peers.iter() {
            let _ = self.notification_service.send_async_notification(
                peer,
                encoded.clone(),
            ).await;
        }
        
        info!("Broadcasted storage node registration to {} peers: {}", peer_count, endpoint);
    }

    /// 受信した通知を処理
    async fn handle_notification(&mut self, peer: sc_network::PeerId, notification: Vec<u8>) {
        let message = match StorageNodeGossipMessage::decode(&mut &notification[..]) {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to decode gossip message from {:?}: {:?}", peer, e);
                return;
            }
        };

        match message {
            StorageNodeGossipMessage::NodeRegistered {
                endpoint,
                registered_at,
                latency_ms,
                operator,
                timestamp,
                signature,
            } => {
                debug!("Received node registration from {:?}: {}", peer, endpoint);

                // (finding #2) RPC 経路と同じ検証: タイムスタンプ skew (リプレイ防止)
                // ブロードキャストは登録直後に行われるため、正規メッセージは窓内に収まる
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let skew = now.abs_diff(timestamp);
                if skew > MAX_TIMESTAMP_SKEW_SECS {
                    warn!(
                        "Dropping gossip registration from {:?}: timestamp skew {}s > {}s ({})",
                        peer, skew, MAX_TIMESTAMP_SKEW_SECS, endpoint
                    );
                    return;
                }

                // (finding #2) RPC 経路と同じオペレーター sr25519 署名検証
                if !verify_registration_signature(&endpoint, &operator, timestamp, &signature) {
                    warn!(
                        "Dropping gossip registration from {:?}: invalid operator signature ({})",
                        peer, endpoint
                    );
                    return;
                }

                // (finding #1) URL ポリシー検証 (SSRF 対策, DNS 解決込み)
                if let Err(e) = self.endpoint_policy.validate_url(&endpoint).await {
                    warn!(
                        "Dropping gossip registration from {:?}: URL policy violation ({}): {}",
                        peer, endpoint, e
                    );
                    return;
                }

                // Issue 7 fix: Registry has built-in size limit with LRU eviction
                let mut registry = self.storage_nodes.write().await;

                // Check if registry is at capacity before registering
                let was_at_capacity = registry.nodes.len() >= MAX_REGISTRY_SIZE;

                let node = RegisteredStorageNode {
                    endpoint: endpoint.clone(),
                    node_id: None,
                    registered_at,
                    is_online: true,
                    last_health_check: registered_at,
                    latency_ms,
                    registration_proof: Some(RegistrationProof {
                        operator,
                        timestamp,
                        signature: signature.to_vec(),
                    }),
                };

                if registry.register(node) {
                    if was_at_capacity {
                        info!("Added storage node from gossip (LRU eviction triggered): {} (total: {})",
                            endpoint, registry.nodes.len());
                    } else {
                        info!("Added storage node from gossip: {} (total: {})", endpoint, registry.nodes.len());
                    }
                }
            }
            StorageNodeGossipMessage::SyncRequest => {
                debug!("Received sync request from {:?}", peer);
                self.send_sync_response_to(&peer).await;
            }
            StorageNodeGossipMessage::SyncResponse { nodes } => {
                let node_count = nodes.len();
                debug!("Received sync response from {:?}: {} nodes", peer, node_count);

                if node_count > MAX_SYNC_NODES {
                    warn!(
                        "Dropping sync response from {:?}: too many nodes ({} > {})",
                        peer, node_count, MAX_SYNC_NODES
                    );
                    return;
                }

                let mut added = 0usize;
                for node_info in nodes {
                    // (finding #2) 同期エントリにも署名検証を要求する。
                    // 注: 同期は登録からかなり後に起こりうるため timestamp skew は
                    // 検査しない。署名が URL とオペレーターを束縛しているので、
                    // 改ざんした URL の注入は不可（可能なのは正規登録の再中継のみ）。
                    if !verify_registration_signature(
                        &node_info.endpoint,
                        &node_info.operator,
                        node_info.timestamp,
                        &node_info.signature,
                    ) {
                        warn!(
                            "Skipping sync entry from {:?}: invalid operator signature ({})",
                            peer, node_info.endpoint
                        );
                        continue;
                    }

                    // (finding #1) URL ポリシー検証 (SSRF 対策)
                    if let Err(e) = self.endpoint_policy.validate_url(&node_info.endpoint).await {
                        warn!(
                            "Skipping sync entry from {:?}: URL policy violation ({}): {}",
                            peer, node_info.endpoint, e
                        );
                        continue;
                    }

                    let mut registry = self.storage_nodes.write().await;
                    let node = RegisteredStorageNode {
                        endpoint: node_info.endpoint.clone(),
                        node_id: None,
                        registered_at: node_info.registered_at,
                        is_online: true,
                        last_health_check: node_info.registered_at,
                        latency_ms: node_info.latency_ms,
                        registration_proof: Some(RegistrationProof {
                            operator: node_info.operator,
                            timestamp: node_info.timestamp,
                            signature: node_info.signature.to_vec(),
                        }),
                    };

                    if registry.register(node) {
                        added += 1;
                        debug!("Added storage node from sync: {}", node_info.endpoint);
                    }
                }
                info!("Synced {} storage nodes ({} added) from peer {:?}", node_count, added, peer);
            }
        }
    }

    /// 特定ピアに同期応答を送信
    ///
    /// (finding #2) 登録証明 (オペレーター署名) を持つノードのみ中継する。
    /// 受信側で署名を再検証できないノードは同期対象から除外。
    async fn send_sync_response_to(&mut self, _peer: &sc_network::PeerId) {
        let registry = self.storage_nodes.read().await;
        let nodes: Vec<GossipNodeInfo> = registry
            .nodes
            .iter()
            .filter_map(GossipNodeInfo::from_node)
            .take(MAX_SYNC_NODES)
            .collect();
        
        if !nodes.is_empty() {
            let message = StorageNodeGossipMessage::SyncResponse { nodes };
            let encoded = message.encode();
            
            // NotificationServiceのbroadcast機能を使用
            // 実際には特定ピアに送信したいが、APIの制約でブロードキャストを使用
            let _ = self.notification_service.send_async_notification(
                _peer,
                encoded.clone(),
            ).await;
        }
    }
}

/// ストレージノード登録をブロードキャストするためのハンドル
#[derive(Clone)]
pub struct StorageNodeGossipHandle {
    tx: tokio::sync::mpsc::UnboundedSender<BroadcastCommand>,
}

/// ブロードキャストコマンド
pub enum BroadcastCommand {
    /// ノード登録をブロードキャスト (オペレーター署名付き, finding #2)
    BroadcastRegistration {
        endpoint: String,
        registered_at: u64,
        latency_ms: Option<u64>,
        proof: RegistrationProof,
    },
}

impl StorageNodeGossipHandle {
    /// 新しいノード登録をブロードキャスト (検証済みオペレーター署名を添付)
    pub fn broadcast_registration(
        &self,
        endpoint: String,
        registered_at: u64,
        latency_ms: Option<u64>,
        proof: RegistrationProof,
    ) {
        let _ = self.tx.send(BroadcastCommand::BroadcastRegistration {
            endpoint,
            registered_at,
            latency_ms,
            proof,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::{sr25519, Pair};

    /// 正規署名付きの NodeRegistered メッセージを生成
    fn make_signed_message(endpoint: &str, timestamp: u64) -> (StorageNodeGossipMessage, sr25519::Pair) {
        let (pair, _) = sr25519::Pair::generate();
        let message = format!("register_endpoint:{}:{}", endpoint, timestamp);
        let signature = pair.sign(message.as_bytes());
        (
            StorageNodeGossipMessage::NodeRegistered {
                endpoint: endpoint.to_string(),
                registered_at: timestamp,
                latency_ms: None,
                operator: pair.public().0,
                timestamp,
                signature: signature.0,
            },
            pair,
        )
    }

    /// (finding #2) gossip メッセージの SCALE roundtrip と署名検証
    #[test]
    fn test_gossip_message_signature_roundtrip() {
        let endpoint = "http://127.0.0.1:3030";
        let (msg, _pair) = make_signed_message(endpoint, 1_708_502_400);

        // SCALE encode → decode roundtrip
        let encoded = msg.encode();
        let decoded = StorageNodeGossipMessage::decode(&mut &encoded[..]).unwrap();

        match decoded {
            StorageNodeGossipMessage::NodeRegistered {
                endpoint: ep,
                operator,
                timestamp,
                signature,
                ..
            } => {
                // 正規署名は検証に通る
                assert!(verify_registration_signature(&ep, &operator, timestamp, &signature));
                // endpoint を改ざんすると検証に失敗する (registry poisoning 防止)
                assert!(!verify_registration_signature(
                    "http://evil.example:3030",
                    &operator,
                    timestamp,
                    &signature
                ));
                // timestamp を改ざんしても失敗する
                assert!(!verify_registration_signature(&ep, &operator, timestamp + 1, &signature));
            }
            _ => panic!("decoded to wrong variant"),
        }
    }

    /// (finding #2) 無署名 (ゼロ署名) のメッセージは検証で落ちる
    #[test]
    fn test_gossip_forged_signature_rejected() {
        let endpoint = "http://127.0.0.1:3030";
        let operator = [0x42u8; 32];
        let signature = [0u8; 64];
        assert!(!verify_registration_signature(endpoint, &operator, 1_708_502_400, &signature));
    }

    /// (finding #2) 登録証明のないノードは Gossip 中継対象にならない
    #[test]
    fn test_gossip_node_info_requires_proof() {
        let node = RegisteredStorageNode::new("http://127.0.0.1:3030".to_string());
        assert!(node.registration_proof.is_none());
        assert!(GossipNodeInfo::from_node(&node).is_none(), "no proof → not relayed");

        let mut node_with_proof = node.clone();
        node_with_proof.registration_proof = Some(RegistrationProof {
            operator: [1u8; 32],
            timestamp: 1000,
            signature: vec![0u8; 64],
        });
        let info = GossipNodeInfo::from_node(&node_with_proof).expect("proof → relayed");
        assert_eq!(info.endpoint, "http://127.0.0.1:3030");
        assert_eq!(info.operator, [1u8; 32]);

        // 署名長が不正な証明は中継しない
        let mut bad_sig = node_with_proof.clone();
        bad_sig.registration_proof = Some(RegistrationProof {
            operator: [1u8; 32],
            timestamp: 1000,
            signature: vec![0u8; 10],
        });
        assert!(GossipNodeInfo::from_node(&bad_sig).is_none());
    }
}

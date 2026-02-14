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

use crate::rpc::{RegisteredStorageNode, SharedStorageNodes};
use log::{debug, info, warn};
use parity_scale_codec::{Decode, Encode};
use sc_network::{
    config::SetConfig,
    peer_store::PeerStoreProvider,
    service::{traits::{NotificationEvent, NotificationService}, NotificationMetrics},
    NetworkBackend, ProtocolName,
};
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

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

/// Gossipメッセージタイプ
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
    },
    /// ノード一覧同期要求
    SyncRequest,
    /// ノード一覧同期応答
    SyncResponse {
        /// 全登録ノード
        nodes: Vec<GossipNodeInfo>,
    },
}

/// Gossip用ノード情報 (軽量版)
#[derive(Clone, Debug, Encode, Decode)]
pub struct GossipNodeInfo {
    pub endpoint: String,
    pub registered_at: u64,
    pub latency_ms: Option<u64>,
}

impl From<&RegisteredStorageNode> for GossipNodeInfo {
    fn from(node: &RegisteredStorageNode) -> Self {
        Self {
            endpoint: node.endpoint.clone(),
            registered_at: node.registered_at,
            latency_ms: node.latency_ms,
        }
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
}

impl StorageNodeGossip {
    /// 新しいGossipサービスとハンドルを作成
    pub fn new_with_handle(
        storage_nodes: SharedStorageNodes,
        notification_service: Box<dyn NotificationService>,
    ) -> (Self, StorageNodeGossipHandle) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = StorageNodeGossipHandle { tx };
        let service = Self {
            storage_nodes,
            notification_service,
            broadcast_rx: rx,
            connected_peers: std::collections::HashSet::new(),
        };
        (service, handle)
    }

    /// プロトコル名を取得
    pub fn protocol_name() -> ProtocolName {
        STORAGE_NODE_PROTOCOL.into()
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
                            // 全ての受信接続を許可
                            let _ = result_tx.send(sc_network::service::traits::ValidationResult::Accept);
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
                        Some(BroadcastCommand::BroadcastRegistration { endpoint, registered_at, latency_ms }) => {
                            self.broadcast_node_registered(endpoint, registered_at, latency_ms).await;
                        }
                        None => {
                            // チャンネルがクローズされた（全ハンドルがドロップ）
                            debug!("Broadcast channel closed");
                        }
                    }
                }
            }
        }
    }

    /// ノード登録を全ピアにブロードキャスト
    async fn broadcast_node_registered(&mut self, endpoint: String, registered_at: u64, latency_ms: Option<u64>) {
        let message = StorageNodeGossipMessage::NodeRegistered {
            endpoint: endpoint.clone(),
            registered_at,
            latency_ms,
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
            } => {
                debug!("Received node registration from {:?}: {}", peer, endpoint);
                
                // レジストリに追加（重複チェック付き）
                let mut registry = self.storage_nodes.write().await;
                let node = RegisteredStorageNode {
                    endpoint: endpoint.clone(),
                    node_id: None,
                    registered_at,
                    is_online: true,
                    last_health_check: registered_at,
                    latency_ms,
                };
                
                if registry.register(node) {
                    info!("Added storage node from gossip: {} (total: {})", endpoint, registry.nodes.len());
                }
            }
            StorageNodeGossipMessage::SyncRequest => {
                debug!("Received sync request from {:?}", peer);
                self.send_sync_response_to(&peer).await;
            }
            StorageNodeGossipMessage::SyncResponse { nodes } => {
                let node_count = nodes.len();
                debug!("Received sync response from {:?}: {} nodes", peer, node_count);
                
                let mut registry = self.storage_nodes.write().await;
                for node_info in nodes {
                    let node = RegisteredStorageNode {
                        endpoint: node_info.endpoint.clone(),
                        node_id: None,
                        registered_at: node_info.registered_at,
                        is_online: true,
                        last_health_check: node_info.registered_at,
                        latency_ms: node_info.latency_ms,
                    };
                    
                    if registry.register(node) {
                        debug!("Added storage node from sync: {}", node_info.endpoint);
                    }
                }
                info!("Synced {} storage nodes from peer {:?}", node_count, peer);
            }
        }
    }

    /// 特定ピアに同期応答を送信
    async fn send_sync_response_to(&mut self, _peer: &sc_network::PeerId) {
        let registry = self.storage_nodes.read().await;
        let nodes: Vec<GossipNodeInfo> = registry.nodes.iter().map(GossipNodeInfo::from).collect();
        
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
    /// ノード登録をブロードキャスト
    BroadcastRegistration {
        endpoint: String,
        registered_at: u64,
        latency_ms: Option<u64>,
    },
}

impl StorageNodeGossipHandle {
    /// 新しいノード登録をブロードキャスト
    pub fn broadcast_registration(&self, endpoint: String, registered_at: u64, latency_ms: Option<u64>) {
        let _ = self.tx.send(BroadcastCommand::BroadcastRegistration {
            endpoint,
            registered_at,
            latency_ms,
        });
    }
}

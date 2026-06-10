//! P2P network layer using libp2p
//!
//! Implements the fragment exchange protocol using libp2p request-response.
//!
//! ## Protocols
//!
//! - **Fragment Protocol** (`/anarchy/fragment/1.0.0`): Fragment get/put operations
//! - **Repair Protocol** (`/anarchy/repair/1.0.0`): Self-repair share collection (013-slashing-repair)
//!
//! The repair protocol is defined in `crate::repair::protocol` and uses the same
//! request-response pattern as the fragment protocol.

pub mod endpoint_cache;
pub mod gossip;
pub mod reputation;
pub mod storage_node_cache;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use libp2p::{
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    identity::Keypair,
    noise, yamux,
    request_response::{self, Codec, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use futures::prelude::*;
use parity_scale_codec::{Decode, Encode};
use tracing::{info, warn, debug};
use anyhow::{Context, Result};

use crate::storage::{FragmentId, FragmentStore};
use endpoint_cache::BlockchainEndpoint;
use storage_node_cache::StorageNodeEndpoint;
use gossip::{
    build_gossipsub_config, EndpointMessage, StorageNodeMessage,
    ENDPOINT_TOPIC, STORAGE_NODE_TOPIC,
    validate_message, validate_storage_node_message, MessageValidation,
};

/// Protocol name for fragment exchange
pub const FRAGMENT_PROTOCOL: &str = "/anarchy/fragment/1.0.0";

/// チェーンノードが受理する断片の最大サイズ。
/// `apps/blockchain/node/src/rpc/storage.rs` の `MAX_FRAGMENT_SIZE` (128MB)
/// と揃えること。これより小さいとノード間複製 (Put/Get) だけが失敗し、
/// メディア断片の冗長性が静かに失われる。
pub const MAX_FRAGMENT_SIZE: usize = 128 * 1024 * 1024;

/// SCALE エンベロープ分の余裕 (enum tag 1B + fragment_id 32B + compact length 数B)。
const ENVELOPE_OVERHEAD: usize = 1024;

/// 1 メッセージ (length-prefix フレーム) の最大サイズ。
pub const MAX_MESSAGE_SIZE: usize = MAX_FRAGMENT_SIZE + ENVELOPE_OVERHEAD;

/// Request types for fragment protocol
#[derive(Debug, Clone, Encode, Decode)]
pub enum FragmentRequest {
    /// Get fragment by ID
    Get { fragment_id: FragmentId },
    /// Put fragment (for replication)
    Put { fragment_id: FragmentId, data: Vec<u8> },
}

/// Response types for fragment protocol
#[derive(Debug, Clone, Encode, Decode)]
pub enum FragmentResponse {
    /// Fragment data (None if not found)
    Data(Option<Vec<u8>>),
    /// Acknowledgement for Put
    Ack { success: bool, error: Option<String> },
}

/// Codec for fragment protocol messages
///
/// ワイヤ形式: `u32 (BE) length prefix + SCALE エンコード本体`。
/// 以前は serde_json (Vec<u8> が数値配列になり ~4x 膨張) + 10MB 上限で、
/// チェーンノード側の 128MB 上限の断片を複製できなかった。
/// 読み込みは length prefix を検査してから確保する (確保前に上限超過を拒否)。
#[derive(Debug, Clone, Default)]
pub struct FragmentCodec;

/// length-prefix 付きフレームを読み、SCALE デコードする。
/// 上限超過のフレームはバッファ確保「前」に InvalidData で拒否する。
async fn read_framed<T, M>(io: &mut T) -> std::io::Result<M>
where
    T: futures::AsyncRead + Unpin + Send,
    M: Decode,
{
    let mut length_buf = [0u8; 4];
    io.read_exact(&mut length_buf).await?;
    let length = u32::from_be_bytes(length_buf) as usize;

    if length > MAX_MESSAGE_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Message too large: {} > {} bytes", length, MAX_MESSAGE_SIZE),
        ));
    }

    let mut buf = vec![0u8; length];
    io.read_exact(&mut buf).await?;

    M::decode(&mut &buf[..]).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })
}

/// SCALE エンコードして length-prefix 付きで書き込む。
async fn write_framed<T, M>(io: &mut T, msg: &M) -> std::io::Result<()>
where
    T: futures::AsyncWrite + Unpin + Send,
    M: Encode,
{
    let data = msg.encode();
    if data.len() > MAX_MESSAGE_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "Refusing to send oversized message: {} > {} bytes",
                data.len(),
                MAX_MESSAGE_SIZE
            ),
        ));
    }
    let length = (data.len() as u32).to_be_bytes();
    io.write_all(&length).await?;
    io.write_all(&data).await?;
    io.flush().await?;
    Ok(())
}

#[async_trait::async_trait]
impl Codec for FragmentCodec {
    type Protocol = &'static str;
    type Request = FragmentRequest;
    type Response = FragmentResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        read_framed(io).await
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        read_framed(io).await
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        write_framed(io, &req).await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        write_framed(io, &res).await
    }
}

/// Network behaviour for the storage node
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "StorageNodeEvent")]
pub struct StorageNodeBehaviour {
    pub fragment_protocol: request_response::Behaviour<FragmentCodec>,
    pub identify: libp2p::identify::Behaviour,
    pub gossipsub: gossipsub::Behaviour,
}

/// Events emitted by the storage node behaviour
#[derive(Debug)]
pub enum StorageNodeEvent {
    Fragment(request_response::Event<FragmentRequest, FragmentResponse>),
    Identify(Box<libp2p::identify::Event>),
    Gossipsub(gossipsub::Event),
}

impl From<request_response::Event<FragmentRequest, FragmentResponse>> for StorageNodeEvent {
    fn from(e: request_response::Event<FragmentRequest, FragmentResponse>) -> Self {
        StorageNodeEvent::Fragment(e)
    }
}

impl From<libp2p::identify::Event> for StorageNodeEvent {
    fn from(e: libp2p::identify::Event) -> Self {
        StorageNodeEvent::Identify(Box::new(e))
    }
}

impl From<gossipsub::Event> for StorageNodeEvent {
    fn from(e: gossipsub::Event) -> Self {
        StorageNodeEvent::Gossipsub(e)
    }
}

/// P2P Network manager
pub struct Network {
    swarm: Swarm<StorageNodeBehaviour>,
    /// Connected peer tracking
    connected_peers: HashSet<PeerId>,
    keypair: Keypair,
    endpoint_topic: IdentTopic,
    /// Storage node endpoint sharing topic (FR-520)
    storage_node_topic: IdentTopic,
}

impl Network {
    /// Create a new network instance
    pub fn new(keypair: Keypair, _listen_addr: &str) -> Result<Self> {
        let peer_id = PeerId::from(keypair.public());
        info!(peer_id = %peer_id, "Creating network");
        
        let endpoint_topic = IdentTopic::new(ENDPOINT_TOPIC);
        let keypair_clone = keypair.clone();

        let swarm = SwarmBuilder::with_existing_identity(keypair.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .context("Failed to configure TCP transport")?
            .with_behaviour(|key| {
                // timeout 180s: MAX_FRAGMENT_SIZE (128MB) の断片転送を低速回線
                // (Tor 経由含む) でも完了させるため。チェーンノード側
                // (apps/blockchain/node/src/rpc/storage.rs) と同じ値。
                let fragment_protocol = request_response::Behaviour::new(
                    vec![(FRAGMENT_PROTOCOL, ProtocolSupport::Full)],
                    request_response::Config::default()
                        .with_request_timeout(Duration::from_secs(180)),
                );

                let identify = libp2p::identify::Behaviour::new(
                    libp2p::identify::Config::new(
                        "/anarchy/storage/1.0.0".to_string(),
                        key.public(),
                    ),
                );
                
                // Configure gossipsub for endpoint sharing
                let gossipsub_config = build_gossipsub_config();
                let gossipsub = gossipsub::Behaviour::new(
                    MessageAuthenticity::Signed(keypair_clone.clone()),
                    gossipsub_config,
                ).expect("Valid gossipsub config");

                StorageNodeBehaviour {
                    fragment_protocol,
                    identify,
                    gossipsub,
                }
            })
            .context("Failed to create behaviour")?
            .build();

        let storage_node_topic = IdentTopic::new(STORAGE_NODE_TOPIC);
        
        Ok(Self {
            swarm,
            connected_peers: HashSet::new(),
            keypair,
            endpoint_topic,
            storage_node_topic,
        })
    }
    
    /// Subscribe to the endpoint sharing topic
    pub fn subscribe_endpoints(&mut self) -> Result<()> {
        self.swarm.behaviour_mut().gossipsub
            .subscribe(&self.endpoint_topic)
            .context("Failed to subscribe to endpoint topic")?;
        info!(topic = ENDPOINT_TOPIC, "Subscribed to endpoint topic");
        Ok(())
    }
    
    /// Broadcast known endpoints to the network
    pub fn broadcast_endpoints(&mut self, endpoints: Vec<BlockchainEndpoint>) -> Result<()> {
        let peer_id = PeerId::from(self.keypair.public());
        let message = EndpointMessage::new(endpoints, peer_id, &self.keypair)
            .context("Failed to create endpoint message")?;
        
        let data = message.to_bytes()
            .context("Failed to serialize endpoint message")?;
        
        self.swarm.behaviour_mut().gossipsub
            .publish(self.endpoint_topic.clone(), data)
            .map_err(|e| anyhow::anyhow!("Failed to publish: {:?}", e))?;
        
        debug!("Broadcast endpoint update");
        Ok(())
    }
    
    /// Subscribe to the storage node sharing topic (FR-520)
    pub fn subscribe_storage_nodes(&mut self) -> Result<()> {
        self.swarm.behaviour_mut().gossipsub
            .subscribe(&self.storage_node_topic)
            .context("Failed to subscribe to storage node topic")?;
        info!(topic = STORAGE_NODE_TOPIC, "Subscribed to storage node topic");
        Ok(())
    }
    
    /// Broadcast known storage node endpoints to the network (FR-515, FR-519)
    pub fn broadcast_storage_nodes(&mut self, nodes: Vec<StorageNodeEndpoint>) -> Result<()> {
        let peer_id = PeerId::from(self.keypair.public());
        let message = StorageNodeMessage::new(nodes, peer_id, &self.keypair)
            .context("Failed to create storage node message")?;
        
        let data = message.to_bytes()
            .context("Failed to serialize storage node message")?;
        
        self.swarm.behaviour_mut().gossipsub
            .publish(self.storage_node_topic.clone(), data)
            .map_err(|e| anyhow::anyhow!("Failed to publish storage nodes: {:?}", e))?;
        
        debug!(nodes = message.nodes.len(), "Broadcast storage node update");
        Ok(())
    }

    /// Start listening on the configured address
    pub fn listen(&mut self, addr: &str) -> Result<()> {
        let multiaddr: Multiaddr = addr.parse()
            .context("Failed to parse listen address")?;
        self.swarm.listen_on(multiaddr)?;
        info!(addr = addr, "Listening for connections");
        Ok(())
    }

    /// Connect to a peer
    pub fn dial(&mut self, addr: &str) -> Result<()> {
        let multiaddr: Multiaddr = addr.parse()
            .context("Failed to parse peer address")?;
        self.swarm.dial(multiaddr)?;
        Ok(())
    }

    /// Get connected peer count
    pub fn peer_count(&self) -> usize {
        self.connected_peers.len()
    }

    /// Request a fragment from a peer
    pub fn request_fragment(&mut self, peer: PeerId, fragment_id: FragmentId) {
        let request = FragmentRequest::Get { fragment_id };
        self.swarm.behaviour_mut().fragment_protocol.send_request(&peer, request);
    }

    /// Process incoming events (call in event loop).
    ///
    /// `chain_client` is required so that `Put` requests can be authenticated
    /// against on-chain fragment registration (#30-C-2).
    pub async fn handle_event(
        &mut self,
        store: &Arc<FragmentStore>,
        chain_client: &crate::chain::ChainClient,
    ) -> Result<Option<NetworkEvent>> {
        match self.swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!(addr = %address, "Listening on new address");
                Ok(Some(NetworkEvent::Listening(address)))
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                self.connected_peers.insert(peer_id);
                info!(peer = %peer_id, "Peer connected");
                Ok(Some(NetworkEvent::PeerConnected(peer_id)))
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                self.connected_peers.remove(&peer_id);
                info!(peer = %peer_id, "Peer disconnected");
                Ok(Some(NetworkEvent::PeerDisconnected(peer_id)))
            }
            SwarmEvent::Behaviour(StorageNodeEvent::Fragment(
                request_response::Event::Message { peer, message }
            )) => {
                match message {
                    request_response::Message::Request { request, channel, .. } => {
                        debug!(peer = %peer, "Received request: {:?}", request);
                        let (response, stored_fragment) = self.handle_request(store, chain_client, request).await?;
                        if let Err(e) = self.swarm.behaviour_mut().fragment_protocol.send_response(channel, response) {
                            warn!(peer = %peer, error = ?e, "Failed to send response");
                        }
                        // Return FragmentStored event for auto-declare (T056)
                        if let Some(fragment_id) = stored_fragment {
                            return Ok(Some(NetworkEvent::FragmentStored { fragment_id }));
                        }
                    }
                    request_response::Message::Response { response, .. } => {
                        debug!(peer = %peer, "Received response");
                        return Ok(Some(NetworkEvent::FragmentResponse { peer, response }));
                    }
                }
                Ok(None)
            }
            SwarmEvent::Behaviour(StorageNodeEvent::Identify(event)) => {
                if let libp2p::identify::Event::Received { peer_id, info, .. } = *event {
                    debug!(peer = %peer_id, agent = %info.agent_version, "Identified peer");
                }
                Ok(None)
            }
            SwarmEvent::Behaviour(StorageNodeEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source,
                message,
                ..
            })) => {
                let topic_str = message.topic.as_str();
                
                // Handle blockchain endpoint messages
                if topic_str == ENDPOINT_TOPIC {
                    match validate_message(&message.data) {
                        MessageValidation::Valid => {
                            if let Ok(endpoint_msg) = EndpointMessage::from_bytes(&message.data) {
                                debug!(
                                    peer = %propagation_source,
                                    endpoints = endpoint_msg.endpoints.len(),
                                    "Received endpoint update"
                                );
                                return Ok(Some(NetworkEvent::EndpointUpdate {
                                    from: propagation_source,
                                    endpoints: endpoint_msg.endpoints,
                                }));
                            }
                        }
                        validation => {
                            warn!(
                                peer = %propagation_source,
                                validation = ?validation,
                                "Rejected invalid endpoint message"
                            );
                        }
                    }
                }
                // Handle storage node messages (FR-515, FR-519)
                else if topic_str == STORAGE_NODE_TOPIC {
                    match validate_storage_node_message(&message.data) {
                        MessageValidation::Valid => {
                            if let Ok(node_msg) = StorageNodeMessage::from_bytes(&message.data) {
                                debug!(
                                    peer = %propagation_source,
                                    nodes = node_msg.nodes.len(),
                                    "Received storage node update"
                                );
                                return Ok(Some(NetworkEvent::StorageNodeUpdate {
                                    from: propagation_source,
                                    nodes: node_msg.nodes,
                                }));
                            }
                        }
                        validation => {
                            warn!(
                                peer = %propagation_source,
                                validation = ?validation,
                                "Rejected invalid storage node message"
                            );
                        }
                    }
                } else {
                    debug!(topic = topic_str, "Received message on unknown topic");
                }
                Ok(None)
            }
            SwarmEvent::Behaviour(StorageNodeEvent::Gossipsub(gossipsub::Event::Subscribed {
                peer_id,
                topic,
            })) => {
                debug!(peer = %peer_id, topic = %topic, "Peer subscribed to topic");
                Ok(None)
            }
            SwarmEvent::Behaviour(StorageNodeEvent::Gossipsub(_)) => {
                // Other gossipsub events (unsubscribed, etc.)
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Handle an incoming fragment request.
    ///
    /// (#30-C-2) Put requests are now authenticated against the chain: the
    /// fragment_id MUST be registered in `Storage::Fragments` for the write
    /// to succeed. Without this, any peer could send arbitrary `Put` to fill
    /// our disk with garbage. We **fail closed** when the chain query errors
    /// (e.g. chain unreachable), since accepting unverified writes is worse
    /// than dropping them.
    async fn handle_request(
        &self,
        store: &Arc<FragmentStore>,
        chain_client: &crate::chain::ChainClient,
        request: FragmentRequest,
    ) -> Result<(FragmentResponse, Option<FragmentId>)> {
        match request {
            FragmentRequest::Get { fragment_id } => {
                // redb の同期 I/O で libp2p イベントループを塞がないよう
                // spawn_blocking に逃がす
                let store = Arc::clone(store);
                let data = tokio::task::spawn_blocking(move || store.retrieve(&fragment_id))
                    .await
                    .context("retrieve task panicked")??;
                Ok((FragmentResponse::Data(data), None))
            }
            FragmentRequest::Put { fragment_id, data } => {
                match chain_client.fragment_exists(&fragment_id).await {
                    Ok(true) => {} // authorized — proceed to store
                    Ok(false) => {
                        warn!(
                            fragment_id = %hex::encode(fragment_id),
                            "Rejecting Put: fragment not registered on chain"
                        );
                        return Ok((
                            FragmentResponse::Ack {
                                success: false,
                                error: Some("fragment not registered on chain".into()),
                            },
                            None,
                        ));
                    }
                    Err(e) => {
                        warn!(
                            fragment_id = %hex::encode(fragment_id),
                            error = %e,
                            "Rejecting Put: chain check failed (fail-closed)"
                        );
                        return Ok((
                            FragmentResponse::Ack {
                                success: false,
                                error: Some("chain unavailable, cannot authorize".into()),
                            },
                            None,
                        ));
                    }
                }
                // redb の書き込み commit (fsync) を libp2p イベントループから逃がす
                let store = Arc::clone(store);
                let store_result = tokio::task::spawn_blocking(move || store.store(fragment_id, &data))
                    .await
                    .context("store task panicked")?;
                match store_result {
                    Ok(()) => {
                        // Return fragment_id for auto-declare (T056)
                        Ok((FragmentResponse::Ack { success: true, error: None }, Some(fragment_id)))
                    }
                    Err(e) => Ok((FragmentResponse::Ack {
                        success: false,
                        error: Some(e.to_string()),
                    }, None)),
                }
            }
        }
    }
}

/// Events returned by the network
#[derive(Debug)]
pub enum NetworkEvent {
    Listening(Multiaddr),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    FragmentResponse {
        peer: PeerId,
        response: FragmentResponse,
    },
    /// Fragment was stored locally - trigger auto-declare (T056)
    FragmentStored {
        fragment_id: FragmentId,
    },
    /// Endpoint update received via Gossipsub (FR-502, FR-512)
    EndpointUpdate {
        from: PeerId,
        endpoints: Vec<BlockchainEndpoint>,
    },
    /// Storage node update received via Gossipsub (FR-515, FR-519)
    StorageNodeUpdate {
        from: PeerId,
        nodes: Vec<StorageNodeEndpoint>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::io::Cursor;

    #[tokio::test]
    async fn test_codec_roundtrip_request() {
        let mut codec = FragmentCodec;
        let request = FragmentRequest::Get {
            fragment_id: [42u8; 32],
        };

        let mut buf = Cursor::new(Vec::new());
        codec
            .write_request(&FRAGMENT_PROTOCOL, &mut buf, request)
            .await
            .unwrap();

        let mut reader = Cursor::new(buf.into_inner());
        let deserialized = codec
            .read_request(&FRAGMENT_PROTOCOL, &mut reader)
            .await
            .unwrap();
        match deserialized {
            FragmentRequest::Get { fragment_id } => {
                assert_eq!(fragment_id, [42u8; 32]);
            }
            _ => panic!("Wrong request type"),
        }
    }

    #[tokio::test]
    async fn test_codec_roundtrip_response() {
        let mut codec = FragmentCodec;
        let response = FragmentResponse::Data(Some(vec![1, 2, 3]));

        let mut buf = Cursor::new(Vec::new());
        codec
            .write_response(&FRAGMENT_PROTOCOL, &mut buf, response)
            .await
            .unwrap();

        let mut reader = Cursor::new(buf.into_inner());
        let deserialized = codec
            .read_response(&FRAGMENT_PROTOCOL, &mut reader)
            .await
            .unwrap();
        match deserialized {
            FragmentResponse::Data(Some(data)) => {
                assert_eq!(data, vec![1, 2, 3]);
            }
            _ => panic!("Wrong response type"),
        }
    }

    /// 旧 JSON コーデックの 10MB 上限では失敗していたサイズ (>10MB) の
    /// Put が往復できることを確認する (メディア断片の複製パス)。
    #[tokio::test]
    async fn test_codec_roundtrip_put_over_10mb() {
        let mut codec = FragmentCodec;
        let payload = vec![0xABu8; 12 * 1024 * 1024]; // 12MB
        let request = FragmentRequest::Put {
            fragment_id: [7u8; 32],
            data: payload.clone(),
        };

        let mut buf = Cursor::new(Vec::new());
        codec
            .write_request(&FRAGMENT_PROTOCOL, &mut buf, request)
            .await
            .unwrap();

        let encoded = buf.into_inner();
        // SCALE はほぼ生バイト: JSON の数値配列 (~4x) と違い
        // エンベロープ分の小さなオーバーヘッドのみ
        assert!(
            encoded.len() < payload.len() + ENVELOPE_OVERHEAD,
            "encoded size {} should be close to payload size {}",
            encoded.len(),
            payload.len()
        );

        let mut reader = Cursor::new(encoded);
        let deserialized = codec
            .read_request(&FRAGMENT_PROTOCOL, &mut reader)
            .await
            .unwrap();
        match deserialized {
            FragmentRequest::Put { fragment_id, data } => {
                assert_eq!(fragment_id, [7u8; 32]);
                assert_eq!(data, payload);
            }
            _ => panic!("Wrong request type"),
        }
    }

    /// 上限超過の length prefix は本体バッファを確保する前に
    /// InvalidData で拒否されること。本体を読みに行っていたら
    /// (= 確保後に read していたら) UnexpectedEof になるはずなので、
    /// InvalidData であることが「確保前拒否」の証拠になる。
    #[tokio::test]
    async fn test_codec_rejects_oversized_before_allocation() {
        let mut codec = FragmentCodec;

        // length prefix だけで本体なし: u32::MAX (~4GiB) を主張
        let frame = u32::MAX.to_be_bytes().to_vec();
        let mut reader = Cursor::new(frame);
        let err = codec
            .read_request(&FRAGMENT_PROTOCOL, &mut reader)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);

        // 境界値: MAX_MESSAGE_SIZE + 1 も拒否 (response 側)
        let frame = ((MAX_MESSAGE_SIZE + 1) as u32).to_be_bytes().to_vec();
        let mut reader = Cursor::new(frame);
        let err = codec
            .read_response(&FRAGMENT_PROTOCOL, &mut reader)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    /// 上限超過メッセージは送信側でも拒否されること。
    #[tokio::test]
    async fn test_codec_refuses_to_write_oversized() {
        let mut codec = FragmentCodec;
        let request = FragmentRequest::Put {
            fragment_id: [0u8; 32],
            data: vec![0u8; MAX_MESSAGE_SIZE + 1],
        };
        let mut buf = Cursor::new(Vec::new());
        let err = codec
            .write_request(&FRAGMENT_PROTOCOL, &mut buf, request)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_protocol_name() {
        assert_eq!(FRAGMENT_PROTOCOL, "/anarchy/fragment/1.0.0");
    }

    #[test]
    fn test_message_size_covers_chain_fragment_limit() {
        // チェーンノードの MAX_FRAGMENT_SIZE (128MB) + エンベロープが収まること
        assert!(MAX_MESSAGE_SIZE > MAX_FRAGMENT_SIZE);
        assert_eq!(MAX_FRAGMENT_SIZE, 128 * 1024 * 1024);
    }
}

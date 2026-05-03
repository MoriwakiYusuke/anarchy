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
use serde::{Deserialize, Serialize};
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

/// Request types for fragment protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FragmentRequest {
    /// Get fragment by ID
    Get { fragment_id: FragmentId },
    /// Put fragment (for replication)
    Put { fragment_id: FragmentId, data: Vec<u8> },
}

/// Response types for fragment protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FragmentResponse {
    /// Fragment data (None if not found)
    Data(Option<Vec<u8>>),
    /// Acknowledgement for Put
    Ack { success: bool, error: Option<String> },
}

/// Codec for fragment protocol messages
#[derive(Debug, Clone, Default)]
pub struct FragmentCodec;

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
        let mut buf = Vec::new();
        let mut length_buf = [0u8; 4];
        io.read_exact(&mut length_buf).await?;
        let length = u32::from_be_bytes(length_buf) as usize;
        
        if length > 10 * 1024 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Message too large",
            ));
        }
        
        buf.resize(length, 0);
        io.read_exact(&mut buf).await?;
        
        serde_json::from_slice(&buf).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let mut length_buf = [0u8; 4];
        io.read_exact(&mut length_buf).await?;
        let length = u32::from_be_bytes(length_buf) as usize;
        
        if length > 10 * 1024 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Message too large",
            ));
        }
        
        let mut buf = vec![0u8; length];
        io.read_exact(&mut buf).await?;
        
        serde_json::from_slice(&buf).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })
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
        let data = serde_json::to_vec(&req).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        let length = (data.len() as u32).to_be_bytes();
        io.write_all(&length).await?;
        io.write_all(&data).await?;
        io.flush().await?;
        Ok(())
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
        let data = serde_json::to_vec(&res).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        let length = (data.len() as u32).to_be_bytes();
        io.write_all(&length).await?;
        io.write_all(&data).await?;
        io.flush().await?;
        Ok(())
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
                let fragment_protocol = request_response::Behaviour::new(
                    vec![(FRAGMENT_PROTOCOL, ProtocolSupport::Full)],
                    request_response::Config::default()
                        .with_request_timeout(Duration::from_secs(30)),
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
        store: &FragmentStore,
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
        store: &FragmentStore,
        chain_client: &crate::chain::ChainClient,
        request: FragmentRequest,
    ) -> Result<(FragmentResponse, Option<FragmentId>)> {
        match request {
            FragmentRequest::Get { fragment_id } => {
                let data = store.retrieve(&fragment_id)?;
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
                match store.store(fragment_id, &data) {
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

    #[test]
    fn test_codec_roundtrip_request() {
        let request = FragmentRequest::Get {
            fragment_id: [42u8; 32],
        };
        let serialized = serde_json::to_vec(&request).unwrap();
        let deserialized: FragmentRequest = serde_json::from_slice(&serialized).unwrap();
        match deserialized {
            FragmentRequest::Get { fragment_id } => {
                assert_eq!(fragment_id, [42u8; 32]);
            }
            _ => panic!("Wrong request type"),
        }
    }

    #[test]
    fn test_codec_roundtrip_response() {
        let response = FragmentResponse::Data(Some(vec![1, 2, 3]));
        let serialized = serde_json::to_vec(&response).unwrap();
        let deserialized: FragmentResponse = serde_json::from_slice(&serialized).unwrap();
        match deserialized {
            FragmentResponse::Data(Some(data)) => {
                assert_eq!(data, vec![1, 2, 3]);
            }
            _ => panic!("Wrong response type"),
        }
    }

    #[test]
    fn test_protocol_name() {
        assert_eq!(FRAGMENT_PROTOCOL, "/anarchy/fragment/1.0.0");
    }
}

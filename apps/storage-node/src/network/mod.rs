//! P2P network layer using libp2p
//!
//! Implements the fragment exchange protocol using libp2p request-response.

pub mod endpoint_cache;
pub mod gossip;
pub mod reputation;

use std::collections::HashSet;
use std::time::Duration;
use libp2p::{
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
}

/// Events emitted by the storage node behaviour
#[derive(Debug)]
pub enum StorageNodeEvent {
    Fragment(request_response::Event<FragmentRequest, FragmentResponse>),
    Identify(Box<libp2p::identify::Event>),
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

/// P2P Network manager
pub struct Network {
    swarm: Swarm<StorageNodeBehaviour>,
    connected_peers: HashSet<PeerId>,
}

impl Network {
    /// Create a new network instance
    pub fn new(keypair: Keypair, _listen_addr: &str) -> Result<Self> {
        let peer_id = PeerId::from(keypair.public());
        info!(peer_id = %peer_id, "Creating network");

        let swarm = SwarmBuilder::with_existing_identity(keypair)
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

                StorageNodeBehaviour {
                    fragment_protocol,
                    identify,
                }
            })
            .context("Failed to create behaviour")?
            .build();

        Ok(Self {
            swarm,
            connected_peers: HashSet::new(),
        })
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

    /// Process incoming events (call in event loop)
    pub async fn handle_event(
        &mut self,
        store: &FragmentStore,
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
                        let (response, stored_fragment) = self.handle_request(store, request)?;
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
            _ => Ok(None),
        }
    }

    /// Handle an incoming fragment request
    /// Returns (response, Option<fragment_id>) where fragment_id is set if a fragment was stored
    fn handle_request(
        &self,
        store: &FragmentStore,
        request: FragmentRequest,
    ) -> Result<(FragmentResponse, Option<FragmentId>)> {
        match request {
            FragmentRequest::Get { fragment_id } => {
                let data = store.retrieve(&fragment_id)?;
                Ok((FragmentResponse::Data(data), None))
            }
            FragmentRequest::Put { fragment_id, data } => {
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

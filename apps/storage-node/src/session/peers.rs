//! Connected peers tracking
//!
//! Maintains a set of currently connected P2P peers.
//! Used to verify that session requests come from actual P2P connections.

use std::collections::HashSet;
use std::sync::Arc;
use libp2p::PeerId;
use parking_lot::RwLock;
use tracing::debug;

/// Tracks connected P2P peers
///
/// Updated by the libp2p swarm event handler:
/// - `add()` on `SwarmEvent::ConnectionEstablished`
/// - `remove()` on `SwarmEvent::ConnectionClosed`
#[derive(Clone, Default)]
pub struct ConnectedPeers {
    inner: Arc<RwLock<HashSet<PeerId>>>,
}

impl ConnectedPeers {
    /// Create a new empty set
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a connected peer
    ///
    /// Called on `SwarmEvent::ConnectionEstablished`
    pub fn add(&self, peer_id: PeerId) {
        let mut peers = self.inner.write();
        if peers.insert(peer_id) {
            debug!(peer_id = %peer_id, count = peers.len(), "Peer connected");
        }
    }

    /// Remove a disconnected peer
    ///
    /// Called on `SwarmEvent::ConnectionClosed`
    pub fn remove(&self, peer_id: &PeerId) {
        let mut peers = self.inner.write();
        if peers.remove(peer_id) {
            debug!(peer_id = %peer_id, count = peers.len(), "Peer disconnected");
        }
    }

    /// Check if a peer is currently connected
    pub fn contains(&self, peer_id: &PeerId) -> bool {
        self.inner.read().contains(peer_id)
    }

    /// Get the number of connected peers
    pub fn count(&self) -> usize {
        self.inner.read().len()
    }

    /// Get all connected peer IDs
    pub fn list(&self) -> Vec<PeerId> {
        self.inner.read().iter().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_remove() {
        let peers = ConnectedPeers::new();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        assert_eq!(peers.count(), 0);
        assert!(!peers.contains(&peer1));

        peers.add(peer1);
        assert_eq!(peers.count(), 1);
        assert!(peers.contains(&peer1));

        peers.add(peer2);
        assert_eq!(peers.count(), 2);
        assert!(peers.contains(&peer2));

        peers.remove(&peer1);
        assert_eq!(peers.count(), 1);
        assert!(!peers.contains(&peer1));
        assert!(peers.contains(&peer2));
    }

    #[test]
    fn test_duplicate_add() {
        let peers = ConnectedPeers::new();
        let peer = PeerId::random();

        peers.add(peer);
        peers.add(peer); // Duplicate add

        assert_eq!(peers.count(), 1);
    }

    #[test]
    fn test_remove_nonexistent() {
        let peers = ConnectedPeers::new();
        let peer = PeerId::random();

        // Should not panic
        peers.remove(&peer);
        assert_eq!(peers.count(), 0);
    }

    #[test]
    fn test_list() {
        let peers = ConnectedPeers::new();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        peers.add(peer1);
        peers.add(peer2);

        let list = peers.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&peer1));
        assert!(list.contains(&peer2));
    }
}

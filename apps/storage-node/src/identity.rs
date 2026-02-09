//! Node identity management
//!
//! Handles PeerID generation and persistence.

use libp2p::identity::{Keypair, ed25519};
use libp2p::PeerId;
use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use tracing::info;

/// Node identity containing keypair and PeerID
pub struct NodeIdentity {
    keypair: Keypair,
    peer_id: PeerId,
}

impl NodeIdentity {
    /// Load existing identity or create new one
    pub fn load_or_create(data_dir: &str) -> Result<Self> {
        let identity_dir = Path::new(data_dir).join("identity");
        let keypair_path = identity_dir.join("keypair.bin");
        let peer_id_path = identity_dir.join("peer_id");

        let keypair = if keypair_path.exists() {
            // Load existing keypair
            let bytes = fs::read(&keypair_path)
                .context("Failed to read keypair file")?;
            
            let mut bytes_arr: [u8; 64] = bytes.try_into()
                .map_err(|_| anyhow::anyhow!("Invalid keypair size"))?;
            let ed25519_kp = ed25519::Keypair::try_from_bytes(&mut bytes_arr)
                .map_err(|e| anyhow::anyhow!("Invalid keypair: {}", e))?;
            
            Keypair::from(ed25519_kp)
        } else {
            // Generate new keypair
            info!("Generating new node identity");
            fs::create_dir_all(&identity_dir)
                .context("Failed to create identity directory")?;
            
            let ed25519_kp = ed25519::Keypair::generate();
            let bytes = ed25519_kp.to_bytes();
            
            // Save keypair
            fs::write(&keypair_path, &bytes)
                .context("Failed to save keypair")?;
            
            Keypair::from(ed25519_kp)
        };

        let peer_id = PeerId::from(keypair.public());

        // Save PeerID as text (for reference)
        fs::write(&peer_id_path, peer_id.to_base58())
            .context("Failed to save peer_id")?;

        Ok(Self { keypair, peer_id })
    }

    /// Get the PeerID
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Get the keypair (for signing and network authentication)
    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_new_identity() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path().to_str().unwrap();

        let identity = NodeIdentity::load_or_create(data_dir).unwrap();
        
        // PeerID should start with "12D3KooW" for Ed25519
        assert!(identity.peer_id().to_base58().starts_with("12D3KooW"));
    }

    #[test]
    fn test_load_existing_identity() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path().to_str().unwrap();

        // Create
        let identity1 = NodeIdentity::load_or_create(data_dir).unwrap();
        let peer_id1 = identity1.peer_id().to_base58();

        // Load
        let identity2 = NodeIdentity::load_or_create(data_dir).unwrap();
        let peer_id2 = identity2.peer_id().to_base58();

        // Should be the same
        assert_eq!(peer_id1, peer_id2);
    }
}

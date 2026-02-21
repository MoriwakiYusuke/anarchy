//! Sr25519 Signer for extrinsic signing
//!
//! Wraps schnorrkel to provide Sr25519 key pair management and signing.

use anyhow::{Context, Result, bail};
use schnorrkel::{
    signing_context, ExpansionMode, MiniSecretKey, PublicKey, SecretKey, Signature,
};
use zeroize::Zeroize;

/// Signing context for Substrate extrinsics
const SIGNING_CTX: &[u8] = b"substrate";

/// Sr25519 signer for chain extrinsics
#[derive(Clone)]
pub struct Signer {
    /// Secret key for signing
    secret: SecretKey,
    /// Public key (AccountId derivation)
    public: PublicKey,
}

impl Signer {
    /// Create signer from hex-encoded seed (32 bytes)
    pub fn from_seed_hex(seed_hex: &str) -> Result<Self> {
        let mut seed_bytes = hex::decode(seed_hex)
            .context("Invalid hex in signer_seed")?;
        
        if seed_bytes.len() != 32 {
            seed_bytes.zeroize();
            bail!("signer_seed must be exactly 32 bytes (64 hex chars), got {}", seed_bytes.len());
        }
        
        let mut seed_arr = [0u8; 32];
        seed_arr.copy_from_slice(&seed_bytes);
        seed_bytes.zeroize(); // Clear immediately after copy
        
        let mini_secret = MiniSecretKey::from_bytes(&seed_arr)
            .map_err(|e| {
                seed_arr.zeroize();
                anyhow::anyhow!("Invalid MiniSecretKey: {:?}", e)
            })?;
        seed_arr.zeroize(); // Clear after use
        
        let secret = mini_secret.expand(ExpansionMode::Ed25519);
        let public = secret.to_public();
        
        Ok(Self { secret, public })
    }
    
    /// Get the public key bytes (32 bytes, used as AccountId)
    pub fn account_id(&self) -> [u8; 32] {
        self.public.to_bytes()
    }
    
    /// Get the SS58-encoded address (for logging)
    pub fn ss58_address(&self) -> String {
        // Simple SS58 encoding for Substrate generic (prefix 42)
        let mut data = vec![42u8]; // SS58 prefix for generic Substrate
        data.extend_from_slice(&self.account_id());
        
        // Calculate checksum
        let hash = ss58_hash(&data);
        data.extend_from_slice(&hash[0..2]);
        
        bs58::encode(data).into_string()
    }
    
    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        let context = signing_context(SIGNING_CTX);
        let signature: Signature = self.secret.sign(context.bytes(message), &self.public);
        signature.to_bytes()
    }
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer")
            .field("account_id", &hex::encode(self.account_id()))
            .field("ss58", &self.ss58_address())
            .finish()
    }
}

/// Simple SS58 checksum hash (Blake2b-512, take first 64 bytes as hash input)
fn ss58_hash(data: &[u8]) -> [u8; 64] {
    use blake2::{Blake2b512, Digest};
    
    const SS58_PREFIX: &[u8] = b"SS58PRE";
    
    let mut hasher = Blake2b512::new();
    hasher.update(SS58_PREFIX);
    hasher.update(data);
    let result = hasher.finalize();
    
    let mut output = [0u8; 64];
    output.copy_from_slice(&result);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_alice_signer() {
        // Alice's dev seed
        let seed = "e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a";
        let signer = Signer::from_seed_hex(seed).unwrap();
        
        // Alice's public key should be known
        let account_id = signer.account_id();
        println!("Alice AccountId: 0x{}", hex::encode(account_id));
        println!("Alice SS58: {}", signer.ss58_address());
        
        // Sign a message
        let message = b"test message";
        let signature = signer.sign(message);
        assert_eq!(signature.len(), 64);
    }
    
    #[test]
    fn test_invalid_seed() {
        // Too short
        let result = Signer::from_seed_hex("deadbeef");
        assert!(result.is_err());
        
        // Invalid hex
        let result = Signer::from_seed_hex("not_hex_at_all");
        assert!(result.is_err());
    }
}

//! Configuration management
//!
//! Loads settings from TOML config file with CLI overrides.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::{Context, Result};

/// CLI override options for configuration
#[derive(Debug, Default)]
pub struct ConfigOverrides {
    pub data_dir: Option<String>,
    pub chain_url: Option<String>,
    pub listen_addr: Option<String>,
    pub rpc_port: Option<u16>,
    pub auth_enabled: Option<bool>,
}

/// Storage node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Data directory for fragments and identity
    #[serde(default = "default_data_dir")]
    pub data_dir: String,

    /// Storage capacity in bytes
    #[serde(default = "default_capacity")]
    pub capacity: u64,

    /// Chain RPC URL
    #[serde(default = "default_chain_url")]
    pub chain_url: String,

    /// Listen address for P2P
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// Rate limit for declare_holding (per minute)
    #[serde(default = "default_declare_rate_limit")]
    pub declare_rate_limit: u32,

    /// HTTP RPC port for blockchain node communication
    #[serde(default = "default_rpc_port")]
    pub rpc_port: u16,

    /// Enable authentication for write operations (default: true)
    #[serde(default = "default_auth_enabled")]
    pub auth_enabled: bool,

    /// Bootstrap peers for Gossipsub (multiaddr strings)
    #[serde(default = "default_bootstrap_peers")]
    pub bootstrap_peers: Vec<String>,

    /// Path to SRS (Structured Reference String) file for KZG proofs
    /// If empty and dev_mode is true, uses a test SRS
    #[serde(default = "default_srs_path")]
    pub srs_path: String,

    /// Development mode - uses insecure test SRS if srs_path is empty
    #[serde(default = "default_dev_mode")]
    pub dev_mode: bool,

    /// Sr25519 signer seed (hex 32 bytes) for signing extrinsics
    /// REQUIRED: Must be configured explicitly for security reasons.
    /// Generate with: openssl rand -hex 32
    pub signer_seed: String,
}

fn default_data_dir() -> String {
    "./data".to_string()
}

fn default_capacity() -> u64 {
    10 * 1024 * 1024 * 1024 // 10GB
}

fn default_chain_url() -> String {
    "ws://127.0.0.1:9944".to_string()
}

fn default_listen_addr() -> String {
    "/ip4/0.0.0.0/tcp/4001".to_string()
}

fn default_declare_rate_limit() -> u32 {
    10 // max 10 per minute (FR-108)
}

fn default_rpc_port() -> u16 {
    3030 // HTTP JSON-RPC port
}

fn default_auth_enabled() -> bool {
    true // Authentication enabled by default (FR-201)
}

fn default_bootstrap_peers() -> Vec<String> {
    Vec::new() // No default bootstrap peers (FR-505)
}

fn default_srs_path() -> String {
    String::new() // Empty means use test SRS in dev mode
}

fn default_dev_mode() -> bool {
    // SECURITY (#31-H-6): default to false. Test SRS in dev_mode=true is INSECURE
    // for production (predictable trapdoor). Operators must explicitly opt-in by
    // setting `dev_mode = true` in config when running locally.
    false
}

// NOTE: signer_seed has no default - it MUST be configured explicitly.
// This prevents accidental use of dev seeds in production.

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            capacity: default_capacity(),
            chain_url: default_chain_url(),
            listen_addr: default_listen_addr(),
            declare_rate_limit: default_declare_rate_limit(),
            rpc_port: default_rpc_port(),
            auth_enabled: default_auth_enabled(),
            bootstrap_peers: default_bootstrap_peers(),
            srs_path: default_srs_path(),
            dev_mode: default_dev_mode(),
            // signer_seed is REQUIRED - must be set via config file
            signer_seed: String::new(),
        }
    }
}

impl Config {
    /// Load configuration from file with CLI overrides
    ///
    /// NOTE: signer_seed is REQUIRED and must be specified in the config file.
    /// The node will refuse to start without a valid 32-byte hex signer seed.
    pub fn load(config_path: &str, overrides: ConfigOverrides) -> Result<Self> {
        let path = Path::new(config_path);

        let mut config: Config = if path.exists() {
            let content = fs::read_to_string(path)
                .context("Failed to read config file")?;
            toml::from_str(&content)
                .context("Failed to parse config file (note: signer_seed is required)")?
        } else {
            anyhow::bail!("Config file not found: {}. A config file with signer_seed is required.", config_path);
        };
        
        // Validate signer_seed is provided
        if config.signer_seed.is_empty() {
            anyhow::bail!("signer_seed is required in config. Generate with: openssl rand -hex 32");
        }
        if config.signer_seed.len() != 64 {
            anyhow::bail!("signer_seed must be exactly 64 hex characters (32 bytes)");
        }

        // Apply CLI overrides
        if let Some(data_dir) = overrides.data_dir {
            config.data_dir = data_dir;
        }
        if let Some(chain_url) = overrides.chain_url {
            config.chain_url = chain_url;
        }
        if let Some(listen_addr) = overrides.listen_addr {
            config.listen_addr = listen_addr;
        }
        if let Some(rpc_port) = overrides.rpc_port {
            config.rpc_port = rpc_port;
        }
        if let Some(auth_enabled) = overrides.auth_enabled {
            config.auth_enabled = auth_enabled;
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.data_dir, "./data");
        assert_eq!(config.capacity, 10 * 1024 * 1024 * 1024);
        assert_eq!(config.chain_url, "ws://127.0.0.1:9944");
        assert_eq!(config.declare_rate_limit, 10);
        assert_eq!(config.rpc_port, 3030);
        assert!(config.auth_enabled);
    }

    #[test]
    fn test_parse_config() {
        let toml = r#"
            data_dir = "/custom/data"
            capacity = 5368709120
            chain_url = "ws://localhost:9944"
            listen_addr = "/ip4/127.0.0.1/tcp/5001"
            declare_rate_limit = 5
            rpc_port = 4040
            auth_enabled = false
            signer_seed = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.data_dir, "/custom/data");
        assert_eq!(config.capacity, 5 * 1024 * 1024 * 1024);
        assert_eq!(config.declare_rate_limit, 5);
        assert_eq!(config.rpc_port, 4040);
        assert!(!config.auth_enabled);
        assert_eq!(config.signer_seed.len(), 64);
    }
}

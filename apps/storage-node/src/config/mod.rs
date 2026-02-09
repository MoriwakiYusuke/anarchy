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

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            capacity: default_capacity(),
            chain_url: default_chain_url(),
            listen_addr: default_listen_addr(),
            declare_rate_limit: default_declare_rate_limit(),
        }
    }
}

impl Config {
    /// Load configuration from file with CLI overrides
    pub fn load(config_path: &str, overrides: ConfigOverrides) -> Result<Self> {
        let path = Path::new(config_path);

        let mut config = if path.exists() {
            let content = fs::read_to_string(path)
                .context("Failed to read config file")?;
            toml::from_str(&content)
                .context("Failed to parse config file")?
        } else {
            Config::default()
        };

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
    }

    #[test]
    fn test_parse_config() {
        let toml = r#"
            data_dir = "/custom/data"
            capacity = 5368709120
            chain_url = "ws://localhost:9944"
            listen_addr = "/ip4/127.0.0.1/tcp/5001"
            declare_rate_limit = 5
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.data_dir, "/custom/data");
        assert_eq!(config.capacity, 5 * 1024 * 1024 * 1024);
        assert_eq!(config.declare_rate_limit, 5);
    }
}

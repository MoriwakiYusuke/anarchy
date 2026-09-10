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
    pub public_url: Option<String>,
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

    /// When true, every fragment retrieve recomputes Blake2-256 and
    /// compares it to the stored `Metadata::data_hash`. Catches bit-rot
    /// on disk at the cost of one hash per read. Off by default — enable
    /// on operators who care about silent corruption.
    #[serde(default = "default_verify_on_read")]
    pub verify_on_read: bool,

    /// 他ホストのチェーンノードに広告する外部到達可能な URL。
    /// 未設定なら `http://127.0.0.1:{rpc_port}` にフォールバックする。
    ///
    /// チェーンノードは登録された URL に対して直接 fan-out するため、
    /// 別ホストのチェーンから使われる構成では loopback ではなく
    /// 到達可能なアドレスを広告する必要がある。
    /// 例: `http://<onion>:3030` / `https://s1.example.com:3030`
    #[serde(default)]
    pub public_url: Option<String>,
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

fn default_verify_on_read() -> bool {
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
            verify_on_read: default_verify_on_read(),
            public_url: None,
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

        config.apply_overrides(overrides);

        // 広告 URL は起動時に検証する。ここで落とさないと、正常起動したまま
        // 登録だけが延々失敗し続ける状態になる。
        config.validate()?;

        Ok(config)
    }

    /// CLI 由来の override を適用する。設定ファイルより CLI が優先される。
    pub fn apply_overrides(&mut self, overrides: ConfigOverrides) {
        if let Some(data_dir) = overrides.data_dir {
            self.data_dir = data_dir;
        }
        if let Some(chain_url) = overrides.chain_url {
            self.chain_url = chain_url;
        }
        if let Some(listen_addr) = overrides.listen_addr {
            self.listen_addr = listen_addr;
        }
        if let Some(rpc_port) = overrides.rpc_port {
            self.rpc_port = rpc_port;
        }
        if let Some(auth_enabled) = overrides.auth_enabled {
            self.auth_enabled = auth_enabled;
        }
        if let Some(public_url) = overrides.public_url {
            self.public_url = Some(public_url);
        }
    }

    /// 起動時の設定検証。**登録が延々失敗し続ける状態を避けるため早期に落とす。**
    ///
    /// `public_url` はチェーンノードに登録され、その後すべてのチェーンが
    /// この URL に直接 fan-out する。スキーム欠落や typo があるとチェーン側の
    /// endpoint policy に弾かれ、ストレージ自身は正常起動したまま 30 秒ごとに
    /// 登録を再試行し続けるだけになる (ログを見ないと気付けない)。
    pub fn validate(&self) -> Result<()> {
        if let Some(url) = &self.public_url {
            validate_public_url(url)?;
        }
        Ok(())
    }

    /// チェーンノードへの登録時に広告する URL を解決する。
    ///
    /// `public_url` が未設定の場合のみ loopback にフォールバックする。
    /// 単一ホスト構成ではこれで足りるが、別ホストのチェーンノードから
    /// fan-out される構成では `--public-url` の指定が必須になる。
    pub fn advertised_url(&self) -> String {
        self.public_url
            .clone()
            .unwrap_or_else(|| format!("http://127.0.0.1:{}", self.rpc_port))
    }
}

/// 広告 URL がチェーンノードから到達可能な形式か検証する。
///
/// ここで見るのは形式だけ (実際に到達できるかは起動時には分からない)。
/// チェーン側の endpoint policy と同じ条件 — http/https スキームとホストの存在 —
/// を先に確認しておく。
fn validate_public_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url)
        .with_context(|| format!("public_url をパースできません: {url:?} (例: http://<onion>:3030)"))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => anyhow::bail!(
            "public_url のスキームは http か https である必要があります (指定値: {other:?}, url: {url:?})"
        ),
    }

    if parsed.host_str().is_none() {
        anyhow::bail!("public_url にホストがありません: {url:?}");
    }

    Ok(())
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

    #[test]
    fn validate_public_url_accepts_reachable_forms() {
        for url in [
            "http://abc123.onion:3030",
            "https://s1.example.com:3030",
            "http://203.0.113.10:3030",
        ] {
            assert!(validate_public_url(url).is_ok(), "{} は通るべき", url);
        }
    }

    #[test]
    fn validate_public_url_rejects_malformed() {
        // スキーム欠落: 一番ありがちなタイポ
        assert!(validate_public_url("abc123.onion:3030").is_err());
        // スキーム違い
        assert!(validate_public_url("ws://abc123.onion:3030").is_err());
        // ホスト無し
        assert!(validate_public_url("http://").is_err());
        // そもそも URL でない
        assert!(validate_public_url("not a url").is_err());
    }

    #[test]
    fn load_rejects_invalid_public_url_at_startup() {
        // 起動時に落とす。登録が延々失敗し続ける状態を避けるため。
        let mut config = Config::default();
        config.public_url = Some("abc123.onion:3030".to_string());
        assert!(config.validate().is_err());

        config.public_url = Some("http://abc123.onion:3030".to_string());
        assert!(config.validate().is_ok());

        // 未設定ならフォールバックするので検証対象外
        config.public_url = None;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn advertised_url_defaults_to_loopback() {
        let mut config = Config::default();
        config.rpc_port = 3030;
        config.public_url = None;
        assert_eq!(config.advertised_url(), "http://127.0.0.1:3030");
    }

    #[test]
    fn advertised_url_uses_public_url_when_set() {
        let mut config = Config::default();
        config.rpc_port = 3030;
        config.public_url = Some("http://abc123.onion:3030".to_string());
        assert_eq!(config.advertised_url(), "http://abc123.onion:3030");
    }

    #[test]
    fn public_url_override_wins_over_config_file() {
        let mut config = Config::default();
        config.public_url = Some("http://from-file.onion:3030".to_string());
        config.apply_overrides(ConfigOverrides {
            public_url: Some("http://from-cli.onion:3030".to_string()),
            ..Default::default()
        });
        assert_eq!(config.advertised_url(), "http://from-cli.onion:3030");
    }
}

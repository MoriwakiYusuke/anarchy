//! Stale Holder GC Module (T059)
//!
//! Removes excess holders from fragments that have more than fragment_count holders
//! due to repairs while old nodes were offline.
//!
//! This module queries the chain for fragments with excess holders and submits
//! evict_stale_holder extrinsics to clean them up.

use crate::chain::ChainClient;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Configuration for stale holder GC
#[derive(Debug, Clone)]
pub struct StaleHolderGcConfig {
    /// How often to check for excess holders (seconds)
    pub check_interval_secs: u64,
    /// Maximum evictions per check cycle
    pub max_evictions_per_cycle: u32,
    /// Whether to enable this GC
    pub enabled: bool,
}

impl Default for StaleHolderGcConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 300, // 5 minutes
            max_evictions_per_cycle: 10,
            enabled: true,
        }
    }
}

/// Stale holder garbage collector
///
/// Periodically queries the chain for fragments with excess holders
/// and submits evict_stale_holder transactions to clean them up.
pub struct StaleHolderGc {
    config: StaleHolderGcConfig,
    _chain_client: Arc<ChainClient>,
}

impl StaleHolderGc {
    /// Create a new stale holder GC instance
    pub fn new(config: StaleHolderGcConfig, chain_client: Arc<ChainClient>) -> Self {
        Self {
            config,
            _chain_client: chain_client,
        }
    }

    /// Get the check interval duration
    pub fn check_interval(&self) -> Duration {
        Duration::from_secs(self.config.check_interval_secs)
    }

    /// Check if GC is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Run a single GC cycle
    ///
    /// 本来の処理:
    /// 1. get_fragments_with_excess_holders Runtime API で過剰ホルダー断片を照会
    /// 2. 各断片について get_eviction_candidates で対象を決定
    /// 3. evict_stale_holder extrinsic を提出 (最大 max_evictions_per_cycle 件)
    ///
    /// 未実装: 以前は `Ok(0)` を返して「過剰ホルダーなし」を装っていたため、
    /// 運用者は GC が動いていると誤認していた。呼び出し元が未実装を検知して
    /// 警告できるよう明示的にエラーを返す。
    pub async fn run_cycle(&self) -> Result<u32> {
        if !self.config.enabled {
            debug!("Stale holder GC is disabled");
            return Ok(0);
        }

        anyhow::bail!(
            "stale holder GC is not implemented: \
             get_fragments_with_excess_holders query / evict_stale_holder submission are missing"
        )
    }

    /// Run the GC loop continuously
    pub async fn run_loop(&self) -> Result<()> {
        info!(
            "Starting stale holder GC loop (interval: {} seconds)",
            self.config.check_interval_secs
        );

        loop {
            match self.run_cycle().await {
                Ok(_) => {}
                Err(e) => {
                    warn!("Stale holder GC cycle failed: {:?}", e);
                }
            }

            tokio::time::sleep(self.check_interval()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = StaleHolderGcConfig::default();
        assert_eq!(config.check_interval_secs, 300);
        assert_eq!(config.max_evictions_per_cycle, 10);
        assert!(config.enabled);
    }

    #[test]
    fn test_check_interval() {
        let config = StaleHolderGcConfig {
            check_interval_secs: 60,
            ..Default::default()
        };
        
        // We can't create a real ChainClient without network, so skip that test
        assert_eq!(config.check_interval_secs, 60);
    }
}

//! TSTS F9: Substrate-native Prometheus emit for Anarchy economic metrics.
//!
//! Substrate ノードの `prometheus_registry` (sc_service が提供) に `anarchy_*`
//! Gauge を register し、定期的に `pallet_economic_params::EconomicMetricsApi::snapshot`
//! を呼んで値を更新する。
//!
//! 外部 [`scripts/prometheus-exporter.mjs`](../../../../scripts/prometheus-exporter.mjs)
//! は同等の機能を JS で提供するが、本モジュールは **node プロセス自体が** メトリクスを
//! emit するので、外部 exporter なしでも単体で監視可能。
//!
//! ## 起動フロー
//!
//! 1. `service.rs::new_full` 内で `prometheus_registry` を取得
//! 2. `EconomicMetrics::register(&registry)?` で Gauge を register
//! 3. `tokio::spawn(collector(client, metrics))` で定期 task を spawn
//! 4. 30 秒間隔で `client.runtime_api().snapshot(at)` を呼び Gauge を update
//!
//! prometheus_registry が無い (`--prometheus-port` 未指定) 場合は何もしない。

use std::sync::Arc;
use std::time::Duration;

use log::{debug, warn};
use pallet_economic_params::EconomicMetricsApi;
use sc_client_api::HeaderBackend;
use sp_api::ProvideRuntimeApi;
use sp_runtime::traits::Block as BlockT;
use substrate_prometheus_endpoint::{register, Gauge, PrometheusError, Registry, U64};

/// Anarchy 経済モデル v1 (TSTS) の主要 Gauge メトリクス集.
///
/// 単位は基本的に MORAL 最小単位 (1 MORAL = 1e12 units) または bytes / count.
/// Grafana 側で human readable へ変換する想定。
#[derive(Clone)]
pub struct EconomicMetrics {
    pub storage_pool: Gauge<U64>,
    pub reaction_pool: Gauge<U64>,
    pub stealth_pool: Gauge<U64>,
    pub base_fee: Gauge<U64>,
    pub total_active_bond: Gauge<U64>,
    pub faucet_minted: Gauge<U64>,
    pub total_issuance: Gauge<U64>,
    pub gas_used_this_block: Gauge<U64>,
    pub reactor_locks_count: Gauge<U64>,
}

impl EconomicMetrics {
    /// 引数の `Registry` に全 Gauge を register する.
    /// `Some(metrics)` を返せば collector が起動できる状態。
    pub fn register(registry: &Registry) -> Result<Self, PrometheusError> {
        Ok(Self {
            storage_pool: register(
                Gauge::new(
                    "anarchy_storage_reward_pool_balance",
                    "pallet_storage::RewardPoolBalance (units, 1 MORAL = 1e12 units)",
                )?,
                registry,
            )?,
            reaction_pool: register(
                Gauge::new(
                    "anarchy_reaction_reward_pool",
                    "pallet_reaction::ReactionRewardPool",
                )?,
                registry,
            )?,
            stealth_pool: register(
                Gauge::new(
                    "anarchy_stealth_reward_pool",
                    "pallet_stealth::StealthRewardPool",
                )?,
                registry,
            )?,
            base_fee: register(
                Gauge::new(
                    "anarchy_base_fee",
                    "pallet_base_fee::BaseFee (units/byte)",
                )?,
                registry,
            )?,
            total_active_bond: register(
                Gauge::new(
                    "anarchy_total_active_bond",
                    "pallet_storage_stake::TotalActiveBond",
                )?,
                registry,
            )?,
            faucet_minted: register(
                Gauge::new(
                    "anarchy_faucet_total_minted",
                    "pallet_faucet::TotalMinted (cap=100k MORAL after which claims fail)",
                )?,
                registry,
            )?,
            total_issuance: register(
                Gauge::new(
                    "anarchy_total_issuance",
                    "pallet_balances::TotalIssuance",
                )?,
                registry,
            )?,
            gas_used_this_block: register(
                Gauge::new(
                    "anarchy_gas_used_this_block",
                    "pallet_base_fee::GasUsedThisBlock (bytes used in current block)",
                )?,
                registry,
            )?,
            reactor_locks_count: register(
                Gauge::new(
                    "anarchy_reactor_locks_count",
                    "pallet_reaction::ReactorLocksCount (active reactor lock entries, O(1) counter)",
                )?,
                registry,
            )?,
        })
    }
}

/// 30 秒ごとに RuntimeApi::snapshot を呼んで Gauge を更新する非同期 task.
///
/// `Block` は generic だが、runtime API impl に bound している。`u128 → u64` の
/// truncate は本番値で問題になるレンジに到達しない (1e12 × 1e6 = 1e18 < 2^64).
/// ただし `u128::MAX` が来た場合は `saturating` cast で 0 ではなく `u64::MAX` を入れる。
pub async fn run_collector<B, C>(client: Arc<C>, metrics: EconomicMetrics)
where
    B: BlockT,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: EconomicMetricsApi<B>,
{
    let mut ticker = tokio::time::interval(Duration::from_secs(30));
    loop {
        ticker.tick().await;

        let best = client.info().best_hash;
        let api = client.runtime_api();
        match api.snapshot(best) {
            Ok(snap) => {
                metrics.storage_pool.set(saturating_u128_to_u64(snap.storage_pool));
                metrics.reaction_pool.set(saturating_u128_to_u64(snap.reaction_pool));
                metrics.stealth_pool.set(saturating_u128_to_u64(snap.stealth_pool));
                metrics.base_fee.set(saturating_u128_to_u64(snap.base_fee));
                metrics.total_active_bond.set(saturating_u128_to_u64(snap.total_active_bond));
                metrics.faucet_minted.set(saturating_u128_to_u64(snap.faucet_minted));
                metrics.total_issuance.set(saturating_u128_to_u64(snap.total_issuance));
                metrics.gas_used_this_block.set(snap.gas_used_this_block as u64);
                metrics.reactor_locks_count.set(snap.reactor_locks_count as u64);
                debug!(target: "anarchy_metrics", "snapshot ok at {:?}", best);
            }
            Err(e) => {
                warn!(target: "anarchy_metrics", "snapshot RuntimeApi failed: {:?}", e);
            }
        }
    }
}

/// u128 → u64 の saturating cast. オーバーフロー時は `u64::MAX` を返す.
fn saturating_u128_to_u64(v: u128) -> u64 {
    if v > u64::MAX as u128 {
        u64::MAX
    } else {
        v as u64
    }
}

#!/usr/bin/env node
/**
 * Anarchy Prometheus Exporter (TSTS F3)
 *
 * チェーン RPC を polling して `anarchy_*` メトリクスを Prometheus exposition
 * format で /metrics エンドポイントから serve する。
 *
 * `infra/grafana/anarchy-economy.json` ダッシュボードはこの exporter を
 * 前提に作られている。
 *
 * 使用方法:
 *   node scripts/prometheus-exporter.mjs
 *   PORT=9620 WS_ENDPOINT=ws://127.0.0.1:9944 POLL_INTERVAL_MS=15000 \
 *     node scripts/prometheus-exporter.mjs
 *
 * メトリクス一覧:
 *   - anarchy_storage_reward_pool_balance         (gauge, units)
 *   - anarchy_reaction_reward_pool                (gauge, units)
 *   - anarchy_stealth_reward_pool                 (gauge, units)
 *   - anarchy_total_issuance                      (gauge, units)
 *   - anarchy_base_fee                            (gauge, units/byte)
 *   - anarchy_gas_used_this_block                 (gauge, bytes)
 *   - anarchy_total_active_bond                   (gauge, units)
 *   - anarchy_faucet_total_minted                 (gauge, units)
 *   - anarchy_block_height                        (counter, blocks)
 *   - anarchy_block_reward_miner_total            (counter, units cumulative)
 *   - anarchy_node_slashed_total                  (counter, events)
 *   - anarchy_reactor_locks_count                 (gauge, count)
 *   - anarchy_exporter_poll_errors_total          (counter, errors)
 *   - anarchy_exporter_last_poll_timestamp        (gauge, unix epoch sec)
 *
 * 設計メモ:
 * - PAPI の getUnsafeApi で raw storage を読む。Substrate の組込
 *   Prometheus (port 9615) とは別経路で動く。
 * - block_reward_miner_total / node_slashed_total は **イベント駆動** なので
 *   毎ブロック finalizedHead を購読してインクリメントする。
 * - poll エラーは継続を妨げず、エラーカウンタを増やすのみ。
 */

import http from 'node:http'
import process from 'node:process'
import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'

const WS_ENDPOINT = process.env.WS_ENDPOINT || 'ws://127.0.0.1:9944'
const PORT = parseInt(process.env.PORT || '9620', 10)
const POLL_INTERVAL_MS = parseInt(process.env.POLL_INTERVAL_MS || '15000', 10)

// メトリクス値のキャッシュ。Prometheus の scrape ごとに最新値を返す。
const metrics = {
  storage_reward_pool_balance: 0n,
  reaction_reward_pool: 0n,
  stealth_reward_pool: 0n,
  total_issuance: 0n,
  base_fee: 0n,
  gas_used_this_block: 0,
  total_active_bond: 0n,
  faucet_total_minted: 0n,
  block_height: 0n,
  block_reward_miner_total: 0n,
  node_slashed_total: 0n,
  reactor_locks_count: 0,
  poll_errors_total: 0n,
  last_poll_timestamp: 0,
}

// ─── Prometheus exposition format renderer ────────────────────────────────

function renderMetric(name, type, help, value) {
  const v = typeof value === 'bigint' ? value.toString() : String(value)
  return `# HELP ${name} ${help}\n# TYPE ${name} ${type}\n${name} ${v}\n`
}

function renderAll() {
  return [
    renderMetric(
      'anarchy_storage_reward_pool_balance',
      'gauge',
      'pallet_storage::RewardPoolBalance (units, 1 MORAL = 1e12 units)',
      metrics.storage_reward_pool_balance,
    ),
    renderMetric(
      'anarchy_reaction_reward_pool',
      'gauge',
      'pallet_reaction::ReactionRewardPool',
      metrics.reaction_reward_pool,
    ),
    renderMetric(
      'anarchy_stealth_reward_pool',
      'gauge',
      'pallet_stealth::StealthRewardPool',
      metrics.stealth_reward_pool,
    ),
    renderMetric(
      'anarchy_total_issuance',
      'gauge',
      'pallet_balances::TotalIssuance',
      metrics.total_issuance,
    ),
    renderMetric(
      'anarchy_base_fee',
      'gauge',
      'pallet_base_fee::BaseFee (units/byte)',
      metrics.base_fee,
    ),
    renderMetric(
      'anarchy_gas_used_this_block',
      'gauge',
      'pallet_base_fee::GasUsedThisBlock (bytes)',
      metrics.gas_used_this_block,
    ),
    renderMetric(
      'anarchy_total_active_bond',
      'gauge',
      'pallet_storage_stake::TotalActiveBond',
      metrics.total_active_bond,
    ),
    renderMetric(
      'anarchy_faucet_total_minted',
      'gauge',
      'pallet_faucet::TotalMinted (cap=100k MORAL after which claims fail)',
      metrics.faucet_total_minted,
    ),
    renderMetric(
      'anarchy_block_height',
      'counter',
      'finalized block number',
      metrics.block_height,
    ),
    renderMetric(
      'anarchy_block_reward_miner_total',
      'counter',
      'cumulative miner block reward minted (sum of BlockRewardSplit.miner events)',
      metrics.block_reward_miner_total,
    ),
    renderMetric(
      'anarchy_node_slashed_total',
      'counter',
      'cumulative NodeSlashed events (storage proof failures)',
      metrics.node_slashed_total,
    ),
    renderMetric(
      'anarchy_reactor_locks_count',
      'gauge',
      'pallet_reaction::ReactorLocks active entry count',
      metrics.reactor_locks_count,
    ),
    renderMetric(
      'anarchy_exporter_poll_errors_total',
      'counter',
      'cumulative chain RPC polling errors (does not block other metrics)',
      metrics.poll_errors_total,
    ),
    renderMetric(
      'anarchy_exporter_last_poll_timestamp',
      'gauge',
      'unix epoch seconds of last successful poll',
      metrics.last_poll_timestamp,
    ),
  ].join('\n')
}

// ─── Chain RPC polling ───────────────────────────────────────────────────

let api = null
let unsubFinalized = null

async function connect() {
  console.log(`[exporter] connecting to ${WS_ENDPOINT} ...`)
  const provider = getWsProvider(WS_ENDPOINT)
  const client = createClient(provider)
  api = client.getUnsafeApi()
  console.log(`[exporter] connected`)
  return client
}

/**
 * 直近の finalized block を読み取り gauge / counter を更新する。
 *
 * 注: 本実装は metadata-aware な PAPI の getValue API を使う想定。実 chain の
 * storage 名 (`storage.RewardPoolBalance` 等) は metadata から導出される。
 * pallet 名と storage 名が一致しない場合は実環境で修正する。
 */
async function pollOnce() {
  try {
    // pallet_storage::RewardPoolBalance
    if (api.query.storage?.rewardPoolBalance) {
      const v = await api.query.storage.rewardPoolBalance.getValue()
      metrics.storage_reward_pool_balance = BigInt(v ?? 0)
    }

    // pallet_reaction::ReactionRewardPool
    if (api.query.reaction?.reactionRewardPool) {
      const v = await api.query.reaction.reactionRewardPool.getValue()
      metrics.reaction_reward_pool = BigInt(v ?? 0)
    }

    // pallet_stealth::StealthRewardPool
    if (api.query.stealth?.stealthRewardPool) {
      const v = await api.query.stealth.stealthRewardPool.getValue()
      metrics.stealth_reward_pool = BigInt(v ?? 0)
    }

    // pallet_balances::TotalIssuance
    if (api.query.balances?.totalIssuance) {
      const v = await api.query.balances.totalIssuance.getValue()
      metrics.total_issuance = BigInt(v ?? 0)
    }

    // pallet_base_fee::BaseFee + GasUsedThisBlock
    if (api.query.baseFee?.baseFee) {
      const v = await api.query.baseFee.baseFee.getValue()
      metrics.base_fee = BigInt(v ?? 0)
    }
    if (api.query.baseFee?.gasUsedThisBlock) {
      const v = await api.query.baseFee.gasUsedThisBlock.getValue()
      metrics.gas_used_this_block = Number(v ?? 0)
    }

    // pallet_storage_stake::TotalActiveBond
    if (api.query.storageStake?.totalActiveBond) {
      const v = await api.query.storageStake.totalActiveBond.getValue()
      metrics.total_active_bond = BigInt(v ?? 0)
    }

    // pallet_faucet::TotalMinted
    if (api.query.faucet?.totalMinted) {
      const v = await api.query.faucet.totalMinted.getValue()
      metrics.faucet_total_minted = BigInt(v ?? 0)
    }

    // ReactorLocks エントリ数 (iter で count)
    if (api.query.reaction?.reactorLocks?.getEntries) {
      const entries = await api.query.reaction.reactorLocks.getEntries()
      metrics.reactor_locks_count = entries.length
    }

    metrics.last_poll_timestamp = Math.floor(Date.now() / 1000)
  } catch (err) {
    console.error('[exporter] poll error:', err?.message ?? err)
    metrics.poll_errors_total = metrics.poll_errors_total + 1n
  }
}

/**
 * finalized block ヘッダを subscribe して block_height とイベント駆動メトリクス
 * (block_reward_miner_total / node_slashed_total) を更新する。
 */
async function subscribeBlocks(client) {
  try {
    const finalized$ = client.finalizedBlock$
    unsubFinalized = finalized$.subscribe(async (blockInfo) => {
      try {
        metrics.block_height = BigInt(blockInfo.number ?? 0)

        // ブロックイベントを取得
        const events = await api.query.system?.events?.getValue?.()
        if (!events || !Array.isArray(events)) return

        for (const ev of events) {
          // BlockRewardSplit { author, miner, storage, reaction }
          if (ev?.event?.type === 'BlockReward' && ev?.event?.value?.type === 'BlockRewardSplit') {
            const miner = BigInt(ev.event.value.value.miner ?? 0)
            metrics.block_reward_miner_total = metrics.block_reward_miner_total + miner
          }
          // NodeSlashed { node, content_hash, penalty_amount }
          if (ev?.event?.type === 'Storage' && ev?.event?.value?.type === 'NodeSlashed') {
            metrics.node_slashed_total = metrics.node_slashed_total + 1n
          }
        }
      } catch (e) {
        // イベントスキーマが異なる runtime では一部 metric 欠落するが他は動く
        console.error('[exporter] block subscribe error:', e?.message ?? e)
      }
    })
  } catch (err) {
    console.error('[exporter] subscribe failed:', err?.message ?? err)
  }
}

// ─── HTTP server ──────────────────────────────────────────────────────────

const server = http.createServer((req, res) => {
  if (req.url === '/metrics') {
    res.writeHead(200, { 'Content-Type': 'text/plain; charset=utf-8' })
    res.end(renderAll())
    return
  }
  if (req.url === '/healthz') {
    const healthy = metrics.last_poll_timestamp > 0 &&
      Math.floor(Date.now() / 1000) - metrics.last_poll_timestamp < (POLL_INTERVAL_MS / 1000) * 5
    res.writeHead(healthy ? 200 : 503, { 'Content-Type': 'text/plain' })
    res.end(healthy ? 'ok\n' : 'unhealthy: stale poll\n')
    return
  }
  res.writeHead(404, { 'Content-Type': 'text/plain' })
  res.end('Anarchy prometheus exporter — see /metrics\n')
})

// ─── Main ────────────────────────────────────────────────────────────────

async function main() {
  const client = await connect()
  await subscribeBlocks(client)
  await pollOnce()

  setInterval(pollOnce, POLL_INTERVAL_MS)

  server.listen(PORT, () => {
    console.log(`[exporter] listening on http://0.0.0.0:${PORT}/metrics (poll every ${POLL_INTERVAL_MS}ms)`)
  })
}

main().catch((err) => {
  console.error('[exporter] fatal:', err)
  process.exit(1)
})

// Graceful shutdown
process.on('SIGTERM', () => {
  console.log('[exporter] SIGTERM received, shutting down')
  if (unsubFinalized) unsubFinalized.unsubscribe()
  server.close(() => process.exit(0))
})
process.on('SIGINT', () => {
  console.log('[exporter] SIGINT received, shutting down')
  if (unsubFinalized) unsubFinalized.unsubscribe()
  server.close(() => process.exit(0))
})

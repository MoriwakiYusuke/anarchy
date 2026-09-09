/**
 * Chain client provider — WebSocket via polkadot-api `getWsProvider`.
 * @module lib/chain-client
 *
 * Phase B (PoW migration) で smoldot から WebSocket に切り替えた:
 *   - smoldot は consensus enum に Babe/Aura/AllAuthorized しか持たず PoW chain を
 *     verify できない (PR #53 で実機検証)
 *   - Anarchy の post / DM / storage は \`storage_uploadFragment\` 等の chain-node
 *     RPC 拡張に依存しており、これは smoldot からは呼べない (CLAUDE.md
 *     Security Principle #5: frontend → storage-node 直接禁止 → chain-node 経由必須)
 *   - 上記 2 点から smoldot を残す価値が無く、WebSocket 一本に統一
 *
 * Anonymity 原則 (CLAUDE.md Principle #1) は chain-node を Tor hidden service として
 * 公開し \`wss://<onion>:9944\` で接続する運用で担保する (docs/operations/tor-overview.md 参照)。
 */

import { createClient, PolkadotClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/web'

/** Chain-node WebSocket endpoint。dev は localhost、production は env で onion address を注入する想定。 */
const CHAIN_RPC_URL =
  (typeof process !== 'undefined' && process.env?.NEXT_PUBLIC_CHAIN_RPC_URL) ||
  'ws://127.0.0.1:9944'

let papiClient: PolkadotClient | null = null
let initPromise: Promise<PolkadotClient> | null = null

/**
 * Initialize the WebSocket-backed PAPI client.
 * Singleton — repeated calls return the same client.
 */
export async function initChainClient(): Promise<PolkadotClient> {
  if (papiClient) {
    return papiClient
  }
  if (initPromise) {
    return initPromise
  }

  initPromise = (async () => {
    try {
      // heartbeatTimeout: ws-provider のデフォルト 40s は PoW chain には短すぎる。
      // block 間隔 (目標 30s) は指数分布で 40s 超のギャップが日常的に発生し、
      // その間 WS メッセージが流れないと "Terminate: heartbeat timeout" で切断され、
      // in-flight の client.submit (finalize 待ち) が巻き添えで失敗する (faucet E2E で実測)。
      // 5 分あれば P(gap > 300s) ≈ e^-10 で実質発生しない。
      const provider = getWsProvider({
        endpoints: [CHAIN_RPC_URL],
        heartbeatTimeout: 300_000,
      })
      papiClient = createClient(provider)
      return papiClient
    } catch (error) {
      initPromise = null
      destroyChainClient()
      throw error
    }
  })()

  return initPromise
}

export function getChainClient(): PolkadotClient | null {
  return papiClient
}

export function isChainClientInitialized(): boolean {
  return papiClient !== null
}

export function destroyChainClient(): void {
  if (papiClient) {
    try {
      papiClient.destroy()
    } catch {
      // ignore destroy error
    }
    papiClient = null
  }
  initPromise = null
}

export function getChainClientDebugInfo(): {
  isInitialized: boolean
  hasClient: boolean
  endpoint: string
} {
  return {
    isInitialized: papiClient !== null,
    hasClient: papiClient !== null,
    endpoint: CHAIN_RPC_URL,
  }
}

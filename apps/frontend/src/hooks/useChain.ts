/**
 * useChain Hook — Chain (WebSocket) 接続のライフサイクル管理。
 * @module hooks/useChain
 *
 * Phase B 移行: useSmoldot を WebSocket-backed に置き換えた版。
 * インターフェースは UseSmoldotResult 互換 (status / blockNumber / unsafeApi / client)。
 */

'use client'

import { useState, useEffect, useRef } from 'react'
import { PolkadotClient } from 'polkadot-api'
import {
  initChainClient,
  destroyChainClient,
  isChainClientInitialized,
} from '@/lib/chain-client'
import type { ConnectionState, ConnectionStatus } from '@/types/connection'

/** Initial sync timeout (ms)。WS は smoldot より速いので短めでよい。 */
const SYNC_TIMEOUT_MS = 30_000

export interface UseChainResult {
  client: PolkadotClient | null
  unsafeApi: any
  connectionState: ConnectionState
  blockNumber: number | null
}

/**
 * Chain client (WebSocket) のライフサイクルを管理する hook。
 *
 * Lifecycle:
 *   1. initializing — WS 接続を開いている
 *   2. syncing      — 最初の System.Number クエリ待ち
 *   3. connected    — block 取得済み、定期更新中
 *   4. error        — タイムアウトまたは接続エラー
 */
export function useChain(): UseChainResult {
  const [client, setClient] = useState<PolkadotClient | null>(null)
  const [unsafeApi, setUnsafeApi] = useState<any>(null)
  const [status, setStatus] = useState<ConnectionStatus>('initializing')
  const [errorMessage, setErrorMessage] = useState<string | undefined>()
  const [blockNumber, setBlockNumber] = useState<number | null>(null)

  const mountedRef = useRef(true)
  const syncTimeoutRef = useRef<NodeJS.Timeout | null>(null)
  const blockUpdateIntervalRef = useRef<NodeJS.Timeout | null>(null)
  const hasInitializedRef = useRef(false)

  useEffect(() => {
    mountedRef.current = true

    if (hasInitializedRef.current) {
      return
    }
    hasInitializedRef.current = true

    const init = async () => {
      const startBlockUpdates = (apiInstance: any) => {
        if (blockUpdateIntervalRef.current) {
          clearInterval(blockUpdateIntervalRef.current)
        }
        // PoW 後 block time 30s なので 10s ごとに更新で十分。
        blockUpdateIntervalRef.current = setInterval(async () => {
          if (!mountedRef.current) {
            if (blockUpdateIntervalRef.current) {
              clearInterval(blockUpdateIntervalRef.current)
              blockUpdateIntervalRef.current = null
            }
            return
          }
          try {
            const currentBlock = await apiInstance.query.System.Number.getValue()
            setBlockNumber(currentBlock)
          } catch (err) {
            console.warn('[useChain] Failed to update block number:', err)
          }
        }, 10000)
      }

      try {
        const alreadyInitialized = isChainClientInitialized()
        const clientInstance = await initChainClient()
        if (!mountedRef.current) return

        setClient(clientInstance)
        const api = clientInstance.getUnsafeApi()

        if (alreadyInitialized) {
          try {
            const currentBlock = await api.query.System.Number.getValue()
            if (!mountedRef.current) return
            setUnsafeApi(api)
            setStatus('connected')
            setBlockNumber(currentBlock)
            startBlockUpdates(api)
            return
          } catch {
            // existing connection check failed → proceed to fresh sync
          }
        }

        setStatus('syncing')

        syncTimeoutRef.current = setTimeout(() => {
          if (mountedRef.current) {
            console.error('[useChain] Sync timeout')
            setStatus('error')
            setErrorMessage('チェーン接続がタイムアウトしました (30秒)')
          }
        }, SYNC_TIMEOUT_MS)

        const clearSyncTimeout = () => {
          if (syncTimeoutRef.current) {
            clearTimeout(syncTimeoutRef.current)
            syncTimeoutRef.current = null
          }
        }

        const pollForSync = async () => {
          const pollInterval = 1500
          let retries = 0
          const maxRetries = SYNC_TIMEOUT_MS / pollInterval

          while (mountedRef.current && retries < maxRetries) {
            try {
              const currentBlock = await api.query.System.Number.getValue()
              // クエリ成功 → 30s タイムアウトを即座に解除 (mounted 判定より先)
              clearSyncTimeout()
              if (!mountedRef.current) return

              setUnsafeApi(api)
              setStatus('connected')
              setBlockNumber(currentBlock)
              startBlockUpdates(api)
              return
            } catch {
              retries++
              await new Promise((resolve) => setTimeout(resolve, pollInterval))
            }
          }

          // リトライ尽き: 旧実装はここで何もせず 'syncing' のまま固まっていた。
          // タイムアウトタイマーも解除してエラー状態を明示する。
          clearSyncTimeout()
          if (mountedRef.current) {
            console.error('[useChain] Sync polling exhausted retries')
            setStatus('error')
            setErrorMessage('チェーン接続がタイムアウトしました (30秒)')
          }
        }

        pollForSync()
      } catch (err) {
        if (!mountedRef.current) return
        console.error('[useChain] Initialization failed:', err)
        setStatus('error')
        setErrorMessage(
          err instanceof Error ? err.message : 'チェーン接続初期化に失敗しました',
        )
      }
    }

    init()

    return () => {
      mountedRef.current = false
      if (blockUpdateIntervalRef.current) {
        clearInterval(blockUpdateIntervalRef.current)
        blockUpdateIntervalRef.current = null
      }
      if (syncTimeoutRef.current) {
        clearTimeout(syncTimeoutRef.current)
        syncTimeoutRef.current = null
      }
      // Singleton client は unmount で破棄しない (他コンポーネントが利用)。
      // ページ離脱時に GC 回収される。
    }
  }, [])

  const connectionState: ConnectionState = {
    status,
    blockNumber: status === 'connected' ? blockNumber ?? undefined : undefined,
    errorMessage: status === 'error' ? errorMessage : undefined,
  }

  return { client, unsafeApi, connectionState, blockNumber }
}

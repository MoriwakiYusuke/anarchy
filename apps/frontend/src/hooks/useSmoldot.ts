/**
 * useSmoldot Hook - smoldot Light Client state management
 * @module hooks/useSmoldot
 * 
 * This hook manages the smoldot light client lifecycle and provides
 * connection state information for components.
 */

'use client'

import { useState, useEffect, useRef } from 'react'
import { PolkadotClient } from 'polkadot-api'
import {
  initSmoldotClient,
  destroySmoldotClient,
} from '@/lib/smoldot-provider'
import type { ConnectionState, ConnectionStatus } from '@/types/connection'

/** Timeout for initial sync in milliseconds (60 seconds per spec) */
const SYNC_TIMEOUT_MS = 60_000

export interface UseSmoldotResult {
  /** PAPI client instance (null until connected) */
  client: PolkadotClient | null
  /** Unsafe API for direct chain queries */
  unsafeApi: any
  /** Current connection state */
  connectionState: ConnectionState
  /** Latest block number (null if not connected) */
  blockNumber: number | null
}

/**
 * React hook for managing smoldot light client connection
 * 
 * Lifecycle:
 * 1. initializing - smoldot worker starting
 * 2. syncing - chain added, waiting for first block
 * 3. connected - ready for operations
 * 4. error - initialization or sync failed
 * 
 * @returns UseSmoldotResult with client, API, connection state, and block number
 */
export function useSmoldot(): UseSmoldotResult {
  const [client, setClient] = useState<PolkadotClient | null>(null)
  const [unsafeApi, setUnsafeApi] = useState<any>(null)
  const [status, setStatus] = useState<ConnectionStatus>('initializing')
  const [errorMessage, setErrorMessage] = useState<string | undefined>()
  const [blockNumber, setBlockNumber] = useState<number | null>(null)
  
  // Refs for cleanup
  const mountedRef = useRef(true)
  const syncTimeoutRef = useRef<NodeJS.Timeout | null>(null)

  useEffect(() => {
    mountedRef.current = true
    let subscription: { unsubscribe: () => void } | null = null

    const init = async () => {
      try {
        // Phase 1: Initialize smoldot
        console.log('[useSmoldot] Starting initialization...')
        
        const clientInstance = await initSmoldotClient()
        if (!mountedRef.current) return
        
        setClient(clientInstance)
        setStatus('syncing')
        console.log('[useSmoldot] Smoldot initialized, waiting for chain sync...')
        
        // Phase 2: Wait for sync by polling System.Number
        // Set timeout for sync
        syncTimeoutRef.current = setTimeout(() => {
          if (mountedRef.current) {
            console.error('[useSmoldot] Sync timeout')
            setStatus('error')
            setErrorMessage('同期がタイムアウトしました (60秒)')
          }
        }, SYNC_TIMEOUT_MS)
        
        // Poll for block number - this is more reliable for light clients
        // that may take time to load metadata
        const api = clientInstance.getUnsafeApi()
        
        const pollForSync = async () => {
          const pollInterval = 2000 // 2 seconds between polls
          let retries = 0
          const maxRetries = SYNC_TIMEOUT_MS / pollInterval
          
          while (mountedRef.current && retries < maxRetries) {
            try {
              console.log(`[useSmoldot] Polling for sync (attempt ${retries + 1})...`)
              const currentBlock = await api.query.System.Number.getValue()
              
              if (!mountedRef.current) return
              
              // Successfully got block number - we're synced
              if (syncTimeoutRef.current) {
                clearTimeout(syncTimeoutRef.current)
                syncTimeoutRef.current = null
              }
              
              console.log(`[useSmoldot] Connected - Block #${currentBlock}`)
              setUnsafeApi(api)
              setStatus('connected')
              setBlockNumber(currentBlock)
              
              // Start periodic block updates
              startBlockUpdates(api)
              return
            } catch (err) {
              // Not ready yet, wait and retry
              console.log('[useSmoldot] Not synced yet, waiting...')
              retries++
              await new Promise(resolve => setTimeout(resolve, pollInterval))
            }
          }
        }
        
        // Function to periodically update block number after connected
        const startBlockUpdates = (api: any) => {
          const updateInterval = setInterval(async () => {
            if (!mountedRef.current) {
              clearInterval(updateInterval)
              return
            }
            try {
              const currentBlock = await api.query.System.Number.getValue()
              setBlockNumber(currentBlock)
            } catch (err) {
              console.warn('[useSmoldot] Failed to update block number:', err)
            }
          }, 6000) // Update every 6 seconds (block time)
          
          // Store interval for cleanup
          subscription = { unsubscribe: () => clearInterval(updateInterval) }
        }
        
        pollForSync()
        
      } catch (err) {
        if (!mountedRef.current) return
        console.error('[useSmoldot] Initialization failed:', err)
        setStatus('error')
        setErrorMessage(
          err instanceof Error 
            ? err.message 
            : 'smoldot初期化に失敗しました'
        )
      }
    }

    init()

    return () => {
      mountedRef.current = false
      
      if (subscription) {
        subscription.unsubscribe()
      }
      
      if (syncTimeoutRef.current) {
        clearTimeout(syncTimeoutRef.current)
        syncTimeoutRef.current = null
      }
      
      // Note: We don't destroy smoldot on unmount because it's a singleton
      // that may be used by multiple components. It will be cleaned up
      // when the page unloads.
    }
  }, [])

  // Build ConnectionState object
  const connectionState: ConnectionState = {
    status,
    blockNumber: status === 'connected' ? (blockNumber ?? undefined) : undefined,
    errorMessage: status === 'error' ? errorMessage : undefined,
  }

  return {
    client,
    unsafeApi,
    connectionState,
    blockNumber,
  }
}

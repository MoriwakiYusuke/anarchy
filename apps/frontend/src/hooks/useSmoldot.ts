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
  isSmoldotInitialized,
} from '@/lib/smoldot-provider'
import type { ConnectionState, ConnectionStatus } from '@/types/connection'

/** Spec-defined timeout for initial sync in milliseconds (60 seconds) */
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
  const blockUpdateIntervalRef = useRef<NodeJS.Timeout | null>(null)
  const hasInitializedRef = useRef(false)

  useEffect(() => {
    mountedRef.current = true
    
    // Prevent double initialization (React StrictMode / hot reload)
    if (hasInitializedRef.current) {
      return
    }
    hasInitializedRef.current = true

    const init = async () => {
      // Function to periodically update block number after connected
      // (defined at top to allow hoisting for early use)
      const startBlockUpdates = (apiInstance: any) => {
        // Clear any existing interval
        if (blockUpdateIntervalRef.current) {
          clearInterval(blockUpdateIntervalRef.current)
        }
        
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
            console.warn('[useSmoldot] Failed to update block number:', err)
          }
        }, 6000) // Update every 6 seconds (block time)
      }
      
      try {
        // Check if already connected (singleton pattern)
        const alreadyInitialized = isSmoldotInitialized()
        
        const clientInstance = await initSmoldotClient()
        if (!mountedRef.current) return
        
        setClient(clientInstance)
        const api = clientInstance.getUnsafeApi()
        
        // If already initialized, skip sync timeout - just verify connection
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
            // Existing connection check failed, falling back to sync
          }
        }
        
        setStatus('syncing')
        
        // Set timeout for sync (only for fresh connections)
        // The timeout is cleared when polling succeeds
        syncTimeoutRef.current = setTimeout(() => {
          if (mountedRef.current) {
            console.error('[useSmoldot] Sync timeout')
            setStatus('error')
            setErrorMessage('同期がタイムアウトしました (60秒)')
          }
        }, SYNC_TIMEOUT_MS)
        
        // Poll for block number - this is more reliable for light clients
        // that may take time to load metadata
        const pollForSync = async () => {
          const pollInterval = 2000 // 2 seconds between polls
          let retries = 0
          const maxRetries = SYNC_TIMEOUT_MS / pollInterval
          
          while (mountedRef.current && retries < maxRetries) {
            try {
              const currentBlock = await api.query.System.Number.getValue()
              
              if (!mountedRef.current) return
              
              // Successfully got block number - we're synced
              if (syncTimeoutRef.current) {
                clearTimeout(syncTimeoutRef.current)
                syncTimeoutRef.current = null
              }
              
              setUnsafeApi(api)
              setStatus('connected')
              setBlockNumber(currentBlock)
              
              // Start periodic block updates
              startBlockUpdates(api)
              return
            } catch {
              // Not ready yet, wait and retry
              retries++
              await new Promise(resolve => setTimeout(resolve, pollInterval))
            }
          }
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
      
      if (blockUpdateIntervalRef.current) {
        clearInterval(blockUpdateIntervalRef.current)
        blockUpdateIntervalRef.current = null
      }
      
      if (syncTimeoutRef.current) {
        clearTimeout(syncTimeoutRef.current)
        syncTimeoutRef.current = null
      }
      
      // Note: We don't destroy smoldot on unmount because it's a singleton
      // that may be used by multiple components. It will be cleaned up
      // when the page unloads.
      // Also, we don't reset hasInitializedRef to allow reconnection reuse.
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

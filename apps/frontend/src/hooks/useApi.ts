'use client'

import { useCallback } from 'react'
import { PolkadotClient } from 'polkadot-api'
import { getPolkadotSigner, PolkadotSigner } from 'polkadot-api/signer'
import { DEV_PHRASE } from '@polkadot-labs/hdkd-helpers'
import { useSmoldot } from './useSmoldot'
import type { ConnectionState } from '@/types/connection'

// Re-export connection helpers for backward compatibility
export { isConnected, isSyncing, canPerformOperations } from '@/types/connection'

export interface UseApiResult {
  client: PolkadotClient | null
  unsafeApi: any
  /** Connection state with status, blockNumber, and errorMessage */
  connectionState: ConnectionState
  /** @deprecated Use connectionState.status === 'connected' instead */
  isConnected: boolean
  error: string | null
  createSigner: (seedPhrase: string) => Promise<PolkadotSigner | null>
}

/**
 * React hook for blockchain API access via smoldot light client
 * 
 * This hook uses smoldot to connect directly to the P2P network
 * without requiring a WebSocket RPC endpoint.
 */
export function useApi(): UseApiResult {
  // Use smoldot for connection
  const { client, unsafeApi, connectionState } = useSmoldot()

  // Create signer from seed phrase or derivation path
  // If seedPhrase starts with //, treat it as a derivation path from DEV_PHRASE
  // Otherwise, use it as a mnemonic seed phrase directly
  const createSigner = useCallback(async (seedPhrase: string): Promise<PolkadotSigner | null> => {
    try {
      // Use @polkadot/keyring for all cases to match WalletConnect's address derivation
      // Dynamic import to avoid SSR issues with octal escape sequences in the package
      const { Keyring } = await import('@polkadot/keyring')
      const keyring = new Keyring({ type: 'sr25519' })
      let pair
      
      if (seedPhrase.startsWith('//')) {
        // Development derivation path (e.g., //Alice, //Bob)
        // These use DEV_PHRASE as the base
        pair = keyring.addFromUri(`${DEV_PHRASE}${seedPhrase}`)
      } else {
        // Real seed phrase (12/24 words mnemonic)
        pair = keyring.addFromUri(seedPhrase)
      }
      
      return getPolkadotSigner(
        pair.publicKey,
        'Sr25519',
        (input: Uint8Array) => pair.sign(input)
      )
    } catch (err) {
      console.error('Failed to create signer:', err)
      return null
    }
  }, [])

  // Derive backward-compatible values
  const isConnectedFlag = connectionState.status === 'connected'
  const error = connectionState.status === 'error' 
    ? (connectionState.errorMessage ?? 'エラーが発生しました') 
    : null

  return { 
    client, 
    unsafeApi, 
    connectionState,
    isConnected: isConnectedFlag, 
    error, 
    createSigner 
  }
}

'use client'

import { useCallback } from 'react'
import { PolkadotClient } from 'polkadot-api'
import { getPolkadotSigner, PolkadotSigner } from 'polkadot-api/signer'
import { DEV_PHRASE } from '@polkadot-labs/hdkd-helpers'
import { useChain } from './useChain'
import type { ConnectionState } from '@/types/connection'

export { isConnected, isSyncing, canPerformOperations } from '@/types/connection'

export interface UseApiResult {
  client: PolkadotClient | null
  unsafeApi: any
  /** Connection state with status, blockNumber, and errorMessage */
  connectionState: ConnectionState
  error: string | null
  createSigner: (seedPhrase: string) => Promise<PolkadotSigner | null>
}

/**
 * React hook for blockchain API access via WebSocket-backed PAPI client.
 *
 * Phase B (PoW migration) で smoldot から WebSocket に切替済み (`useChain`)。
 * 詳細: lib/chain-client.ts のヘッダコメント参照。
 */
export function useApi(): UseApiResult {
  const { client, unsafeApi, connectionState } = useChain()

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

  const error = connectionState.status === 'error'
    ? (connectionState.errorMessage ?? 'エラーが発生しました')
    : null

  return {
    client,
    unsafeApi,
    connectionState,
    error,
    createSigner,
  }
}

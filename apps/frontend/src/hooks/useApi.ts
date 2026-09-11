'use client'

import { useCallback } from 'react'
import { PolkadotClient } from 'polkadot-api'
import { getPolkadotSigner, PolkadotSigner } from 'polkadot-api/signer'
import { resolveSeedUri } from '@/lib/devAccounts'
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
  // dev ビルドでは `//Alice` 形式を DEV_PHRASE からの派生パスとして扱う (lib/devAccounts)
  // それ以外はニーモニックとしてそのまま使う
  const createSigner = useCallback(async (seedPhrase: string): Promise<PolkadotSigner | null> => {
    try {
      // Use @polkadot/keyring for all cases to match WalletConnect's address derivation
      // Dynamic import to avoid SSR issues with octal escape sequences in the package
      const { Keyring } = await import('@polkadot/keyring')
      const keyring = new Keyring({ type: 'sr25519' })
      const pair = keyring.addFromUri(resolveSeedUri(seedPhrase))

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

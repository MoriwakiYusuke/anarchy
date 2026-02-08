'use client'

import { useState, useEffect, useCallback } from 'react'
import { createClient, PolkadotClient, Binary } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/web'
import { getPolkadotSigner, PolkadotSigner } from 'polkadot-api/signer'
import { DEV_PHRASE } from '@polkadot-labs/hdkd-helpers'
// Dynamic import for @polkadot/keyring to avoid SSR issues with octal escape sequences
// import { Keyring } from '@polkadot/keyring'

const WS_ENDPOINT = process.env.NEXT_PUBLIC_WS_ENDPOINT || 'ws://127.0.0.1:9944'

export interface UseApiResult {
  client: PolkadotClient | null
  unsafeApi: any
  isConnected: boolean
  error: string | null
  createSigner: (seedPhrase: string) => Promise<PolkadotSigner | null>
}

export function useApi(): UseApiResult {
  const [client, setClient] = useState<PolkadotClient | null>(null)
  const [unsafeApi, setUnsafeApi] = useState<any>(null)
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let mounted = true
    let clientInstance: PolkadotClient | null = null
    let healthCheckInterval: NodeJS.Timeout | null = null

    const connect = async () => {
      try {
        // Create PAPI client
        const provider = getWsProvider(WS_ENDPOINT)
        clientInstance = createClient(provider)
        const api = clientInstance.getUnsafeApi()

        if (mounted) {
          setClient(clientInstance)
          setUnsafeApi(api)
        }

        // Verify actual connection by calling RPC
        const checkConnection = async () => {
          if (!mounted) return
          try {
            // Use System.Number query to verify connection
            await api.query.System.Number.getValue()
            if (mounted) {
              setIsConnected(true)
              setError(null)
            }
          } catch {
            if (mounted) {
              setIsConnected(false)
            }
          }
        }

        // Initial check
        await checkConnection()

        // Periodic health check (every 5 seconds)
        healthCheckInterval = setInterval(checkConnection, 5000)

      } catch (err) {
        if (mounted) {
          setError(err instanceof Error ? err.message : 'API接続に失敗しました')
          setIsConnected(false)
        }
      }
    }

    connect()

    return () => {
      mounted = false
      if (healthCheckInterval) {
        clearInterval(healthCheckInterval)
      }
      if (clientInstance) {
        clientInstance.destroy()
      }
    }
  }, [])

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

  return { client, unsafeApi, isConnected, error, createSigner }
}

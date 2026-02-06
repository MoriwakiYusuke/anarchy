'use client'

import { useState, useEffect, useCallback } from 'react'
import { createClient, PolkadotClient, Binary } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/web'
import { getPolkadotSigner, PolkadotSigner } from 'polkadot-api/signer'
import { sr25519CreateDerive } from '@polkadot-labs/hdkd'
import { DEV_PHRASE, entropyToMiniSecret, mnemonicToEntropy } from '@polkadot-labs/hdkd-helpers'

const WS_ENDPOINT = process.env.NEXT_PUBLIC_WS_ENDPOINT || 'ws://127.0.0.1:9944'

export interface UseApiResult {
  client: PolkadotClient | null
  unsafeApi: any
  isConnected: boolean
  error: string | null
  createSigner: (derivePath: string) => PolkadotSigner | null
}

export function useApi(): UseApiResult {
  const [client, setClient] = useState<PolkadotClient | null>(null)
  const [unsafeApi, setUnsafeApi] = useState<any>(null)
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let mounted = true
    let clientInstance: PolkadotClient | null = null

    const connect = async () => {
      try {
        // Create PAPI client
        const provider = getWsProvider(WS_ENDPOINT)
        clientInstance = createClient(provider)

        if (mounted) {
          const api = clientInstance.getUnsafeApi()
          setClient(clientInstance)
          setUnsafeApi(api)
          setIsConnected(true)
          setError(null)
        }
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
      if (clientInstance) {
        clientInstance.destroy()
      }
    }
  }, [])

  // Create signer from derivation path (e.g., "//Alice" or "//Bob")
  const createSigner = useCallback((derivePath: string): PolkadotSigner | null => {
    try {
      const entropy = mnemonicToEntropy(DEV_PHRASE)
      const miniSecret = entropyToMiniSecret(entropy)
      const derive = sr25519CreateDerive(miniSecret)
      const keyPair = derive(derivePath)
      
      return getPolkadotSigner(
        keyPair.publicKey,
        'Sr25519',
        (input: Uint8Array) => keyPair.sign(input)
      )
    } catch (err) {
      console.error('Failed to create signer:', err)
      return null
    }
  }, [])

  return { client, unsafeApi, isConnected, error, createSigner }
}

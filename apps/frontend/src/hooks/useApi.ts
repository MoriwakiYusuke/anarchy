'use client'

import { useState, useEffect } from 'react'
import { ApiPromise, WsProvider } from '@polkadot/api'

const WS_ENDPOINT = process.env.NEXT_PUBLIC_WS_ENDPOINT || 'ws://127.0.0.1:9944'

export function useApi() {
  const [api, setApi] = useState<ApiPromise | null>(null)
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let mounted = true
    let apiInstance: ApiPromise | null = null

    const connect = async () => {
      try {
        const provider = new WsProvider(WS_ENDPOINT)
        apiInstance = await ApiPromise.create({ provider })

        if (mounted) {
          setApi(apiInstance)
          setIsConnected(true)
          setError(null)
        }

        apiInstance.on('disconnected', () => {
          if (mounted) {
            setIsConnected(false)
          }
        })

        apiInstance.on('connected', () => {
          if (mounted) {
            setIsConnected(true)
          }
        })
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
      if (apiInstance) {
        apiInstance.disconnect()
      }
    }
  }, [])

  return { api, isConnected, error }
}

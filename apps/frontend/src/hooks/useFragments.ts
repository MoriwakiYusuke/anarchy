'use client'

// useFragments: Fragment retrieval and content recovery (T063)

import { useCallback, useState } from 'react'
import { getSharedWorkerPool } from '@/workers/WorkerPool'

const RPC_ENDPOINT = process.env.NEXT_PUBLIC_WS_ENDPOINT?.replace('ws://', 'http://').replace('wss://', 'https://') || 'http://127.0.0.1:9944'
const MAX_RETRIES = 3
const RETRY_DELAY_MS = 1000

export interface HybridMetadata {
  originalLen: number
  ciphertextLen: number
  shardSize: number
  compressed: boolean
  threshold: number
  totalShards: number
}

export interface RecoverResult {
  data: Uint8Array
}

export interface UseFragmentsResult {
  recoverContent: (merkleRoot: Uint8Array, metadata: HybridMetadata) => Promise<RecoverResult>
  progress: number
  error: string | null
  isProcessing: boolean
  isReady: boolean
}

/**
 * RPC呼び出しユーティリティ
 */
async function rpcCall<T>(method: string, params: unknown[]): Promise<T> {
  const response = await fetch(RPC_ENDPOINT, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method,
      params,
    }),
  })

  const json = await response.json()
  if (json.error) {
    throw new Error(json.error.message || 'RPC error')
  }
  return json.result
}

async function rpcCallWithRetry<T>(
  method: string,
  params: unknown[],
  retries = MAX_RETRIES,
  delayMs = RETRY_DELAY_MS
): Promise<T> {
  let lastError: Error | null = null

  for (let attempt = 1; attempt <= retries; attempt++) {
    try {
      return await rpcCall<T>(method, params)
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(String(err))
      if (attempt < retries) {
        await new Promise(r => setTimeout(r, delayMs * attempt))
      }
    }
  }

  throw lastError || new Error('RPC call failed')
}

/** Fragment retrieval + hybrid recovery hook */
export function useFragments(): UseFragmentsResult {
  const [progress, setProgress] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [isProcessing, setIsProcessing] = useState(false)

  const pool = typeof window !== 'undefined' ? getSharedWorkerPool() : null
  const isReady = pool?.isReady ?? false

  const sendToWorker = useCallback(<T,>(type: string, payload: unknown): Promise<T> => {
    if (!pool) {
      return Promise.reject(new Error('WorkerPool not available'))
    }
    return pool.execute<T>(type, payload)
  }, [pool])

  const recoverContent = useCallback(async (
    merkleRoot: Uint8Array,
    metadata: HybridMetadata
  ): Promise<RecoverResult> => {
    const { threshold: k, totalShards: n, originalLen, ciphertextLen, shardSize, compressed } = metadata
    setIsProcessing(true)
    setError(null)
    setProgress(0)

    try {
      // 1. k個以上の断片を取得
      const shardBytes: Uint8Array[] = []
      const progressPerFragment = 70 / k

      // Try to fetch fragments from storage nodes
      for (let index = 0; index < n; index++) {
        if (shardBytes.length >= k) break
        
        try {
          const params = {
            merkle_root: Array.from(merkleRoot),
            index,
          }
          
          const result = await rpcCallWithRetry<{ data: string; hash: number[] }>(
            'storage_getFragment',
            [params]
          )

          // Base64デコード
          const data = Uint8Array.from(atob(result.data), c => c.charCodeAt(0))
          shardBytes.push(data)
          setProgress(prev => Math.min(prev + progressPerFragment, 70))
        } catch {
          // Failed to get fragment, try next
        }
      }

      if (shardBytes.length < k) {
        throw new Error(`Insufficient fragments: got ${shardBytes.length}, need ${k}`)
      }

      // 2. ハイブリッド復元 (Wasm Worker)
      setProgress(80)
      const recovered = await sendToWorker<Uint8Array>('hybrid_recover', {
        shardBytes,
        k,
        n,
        originalLen,
        ciphertextLen,
        shardSize,
        compressed,
      })

      setProgress(100)
      return { data: recovered }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setError(message)
      throw err
    } finally {
      setIsProcessing(false)
    }
  }, [sendToWorker])

  return {
    recoverContent,
    progress,
    error,
    isProcessing,
    isReady,
  }
}

export default useFragments

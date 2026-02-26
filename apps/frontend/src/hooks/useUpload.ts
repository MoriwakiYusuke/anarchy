'use client'

import { useState, useCallback, useEffect } from 'react'
import { blake2b } from 'blakejs'
import { getSharedWorkerPool } from '@/workers/WorkerPool'
import { uint8ArrayToBase64 } from '@/lib/postCodec'

const RPC_ENDPOINT = process.env.NEXT_PUBLIC_WS_ENDPOINT?.replace('ws://', 'http://').replace('wss://', 'https://') || 'http://127.0.0.1:9944'
const SSS_K = 3, SSS_N = 5, MAX_RETRIES = 3, RETRY_DELAY_MS = 1000

export interface SignedAuth {
  account_id: string
  timestamp: number
  nonce: string
  payload_hash: string
  signature: string
}

export interface StorageSigner {
  publicKey: Uint8Array
  sign: (message: Uint8Array) => Uint8Array
}

export async function createStorageSigner(derivePath: string): Promise<StorageSigner> {
  const { Keyring } = await import('@polkadot/keyring')
  const { DEV_PHRASE } = await import('@polkadot/keyring/defaults')
  
  const keyring = new Keyring({ type: 'sr25519' })
  const pair = derivePath.startsWith('//')
    ? keyring.addFromUri(`${DEV_PHRASE}${derivePath}`)
    : keyring.addFromUri(derivePath)
  
  return {
    publicKey: pair.publicKey,
    sign: (message: Uint8Array) => pair.sign(message)
  }
}

export interface HybridMetadata {
  originalLen: number
  ciphertextLen: number
  shardSize: number
  compressed: boolean
  threshold: number
  totalShards: number
}

export interface UploadResult {
  merkleRoot: Uint8Array
  fragmentHashes: Uint8Array[]
  shardHashes: Uint8Array[]
  totalSize: number
  metadata: HybridMetadata
}

export interface UseUploadOptions {
  signer?: StorageSigner
}

export interface UseUploadResult {
  uploadContent: (content: Uint8Array) => Promise<UploadResult>
  progress: number
  error: string | null
  isProcessing: boolean
  isReady: boolean
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('')
}

function generateAuth(signer: StorageSigner, params: Record<string, unknown>): SignedAuth {
  const timestamp = Math.floor(Date.now() / 1000)
  const nonceBytes = new Uint8Array(16)
  crypto.getRandomValues(nonceBytes)
  
  const sortedParams = Object.keys(params).sort().reduce((acc, key) => {
    acc[key] = params[key]
    return acc
  }, {} as Record<string, unknown>)
  const payload = JSON.stringify(sortedParams)
  const payloadHash = blake2b(new TextEncoder().encode(payload), undefined, 32)
  
  const message = new Uint8Array(32 + 8 + 16 + 32)
  message.set(signer.publicKey, 0)
  new DataView(message.buffer).setBigUint64(32, BigInt(timestamp), true)
  message.set(nonceBytes, 40)
  message.set(payloadHash, 56)
  
  return {
    account_id: toHex(signer.publicKey),
    timestamp,
    nonce: toHex(nonceBytes),
    payload_hash: toHex(payloadHash),
    signature: toHex(signer.sign(message)),
  }
}

export function useUpload(options: UseUploadOptions = {}): UseUploadResult {
  const { signer } = options
  const [progress, setProgress] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [isProcessing, setIsProcessing] = useState(false)
  const [isReady, setIsReady] = useState(false)

  const pool = typeof window !== 'undefined' ? getSharedWorkerPool() : null

  useEffect(() => {
    if (typeof window === 'undefined' || !pool) return
    pool.waitUntilReady().then(() => setIsReady(true)).catch((err) => {
      setError(`WorkerPool error: ${err.message}`)
    })
  }, [pool])

  const sendToWorker = useCallback(<T,>(type: string, payload: unknown): Promise<T> => {
    if (!pool) return Promise.reject(new Error('WorkerPool not available'))
    return pool.execute<T>(type, payload)
  }, [pool])

  const sendToSpecificWorker = useCallback(<T,>(workerIndex: number, type: string, payload: unknown): Promise<T> => {
    if (!pool) return Promise.reject(new Error('WorkerPool not available'))
    return pool.executeOnWorker<T>(workerIndex, type, payload)
  }, [pool])

  const rpcCall = useCallback(async <T,>(method: string, params: unknown[]): Promise<T> => {
    const response = await fetch(RPC_ENDPOINT, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
    })
    const json = await response.json()
    if (json.error) throw new Error(json.error.message || 'RPC error')
    return json.result
  }, [])

  const uploadContent = useCallback(async (content: Uint8Array): Promise<UploadResult> => {
    setIsProcessing(true)
    setError(null)
    setProgress(0)

    try {
      setProgress(10)
      interface HybridSplitResult {
        shards: Uint8Array[]
        shardHashes: Uint8Array[]
        originalLen: number
        ciphertextLen: number
        shardSize: number
        compressed: boolean
        threshold: number
        totalShards: number
      }
      const splitResult = await sendToWorker<HybridSplitResult>('hybrid_split', {
        data: content, k: SSS_K, n: SSS_N,
      })
      const { shards, shardHashes, originalLen, ciphertextLen, shardSize, compressed, threshold, totalShards } = splitResult

      setProgress(20)
      // Merkle操作には同じワーカーを使用（キャッシュ依存のため）
      const merkleWorkerIndex = pool?.acquireWorker() ?? 0
      const merkleResult = await sendToSpecificWorker<{ root: Uint8Array; rootHex: string }>(
        merkleWorkerIndex, 'merkle_build', { fragments: shards }
      )
      const { root: merkleRoot, rootHex: merkleRootHex } = merkleResult

      const fragmentHashes: Uint8Array[] = []
      const progressPerFragment = 70 / SSS_N

      const uploadPromises = shards.map(async (share, index) => {
        // 同じワーカーでProof生成（merkleキャッシュが必要）
        const proof = await sendToSpecificWorker<Uint8Array>(
          merkleWorkerIndex, 'merkle_generate_proof', { merkleRootHex, index }
        )
        const dataB64 = uint8ArrayToBase64(share)
        const proofB64 = uint8ArrayToBase64(proof)
        const baseParams = { merkle_root: Array.from(merkleRoot), index, data: dataB64, proof: proofB64, total_leaves: SSS_N }

        for (let attempt = 1; attempt <= MAX_RETRIES; attempt++) {
          try {
            const requestParams: Record<string, unknown> = { ...baseParams }
            if (signer) requestParams.auth = generateAuth(signer, baseParams)
            const result = await rpcCall<{ success: boolean; fragment_hash: number[] }>('storage_uploadFragment', [requestParams])
            setProgress(prev => Math.min(prev + progressPerFragment, 90))
            return new Uint8Array(result.fragment_hash)
          } catch (err) {
            if (attempt === MAX_RETRIES) throw err
            await new Promise(r => setTimeout(r, RETRY_DELAY_MS * attempt))
          }
        }
        throw new Error('Unreachable')
      })

      const results = await Promise.all(uploadPromises)
      fragmentHashes.push(...results)

      setProgress(100)
      return { merkleRoot, fragmentHashes, shardHashes, totalSize: content.length, metadata: { originalLen, ciphertextLen, shardSize, compressed, threshold, totalShards } }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      throw err
    } finally {
      setIsProcessing(false)
    }
  }, [sendToWorker, rpcCall, signer])

  return { uploadContent, progress, error, isProcessing, isReady }
}

export default useUpload

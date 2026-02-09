'use client'

import { useState, useCallback, useRef, useEffect } from 'react'

/**
 * Storage RPC エンドポイント
 */
const RPC_ENDPOINT = process.env.NEXT_PUBLIC_WS_ENDPOINT?.replace('ws://', 'http://').replace('wss://', 'https://') || 'http://127.0.0.1:9944'

/**
 * SSS/Merkle設定（システム固定値）
 */
const SSS_K = 3  // 復元に必要な最小断片数
const SSS_N = 5  // 総断片数

/**
 * リトライ設定
 */
const MAX_RETRIES = 3
const RETRY_DELAY_MS = 1000

/**
 * 断片アップロード結果
 */
export interface UploadResult {
  merkleRoot: Uint8Array
  fragmentHashes: Uint8Array[]
  totalSize: number
}

/**
 * 復元結果
 */
export interface RecoverResult {
  data: Uint8Array
}

/**
 * useStorage hookの戻り値
 */
export interface UseStorageResult {
  /** コンテンツをSSS分割してアップロード */
  uploadContent: (content: Uint8Array) => Promise<UploadResult>
  /** MerkleRootから断片を取得して復元 */
  recoverContent: (merkleRoot: Uint8Array, k: number, n: number) => Promise<RecoverResult>
  /** 進捗状態 (0-100) */
  progress: number
  /** エラーメッセージ */
  error: string | null
  /** 処理中フラグ */
  isProcessing: boolean
  /** Worker準備完了フラグ */
  isReady: boolean
}

/**
 * Worker通信用のPromise管理
 */
interface PendingRequest {
  resolve: (value: unknown) => void
  reject: (reason: unknown) => void
}

/**
 * 分散ストレージ操作Hook
 *
 * - SSS分割/復元（Wasm: Web Worker）
 * - MerkleTree構築/検証（Wasm: Web Worker）
 * - 断片アップロード/取得（RPC: storage_uploadFragment, storage_getFragment）
 */
export function useStorage(): UseStorageResult {
  const [progress, setProgress] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [isProcessing, setIsProcessing] = useState(false)
  const [isReady, setIsReady] = useState(false)

  // Web Workerインスタンス
  const workerRef = useRef<Worker | null>(null)
  // リクエストID → Promise マップ
  const pendingRef = useRef<Map<string, PendingRequest>>(new Map())
  // リクエストIDカウンタ
  const idCounterRef = useRef(0)

  /**
   * Web Worker初期化
   */
  useEffect(() => {
    // SSR/SSG環境ではWorkerを作成しない
    if (typeof window === 'undefined') {
      return
    }

    // Web Workerを作成
    const worker = new Worker(new URL('../workers/crypto.ts', import.meta.url), { type: 'module' })
    workerRef.current = worker

    worker.onmessage = (event) => {
      const data = event.data
      if (data.type === 'ready') {
        setIsReady(true)
        return
      }

      const { id, success, result, error: errMsg } = data
      const pending = pendingRef.current.get(id)
      if (pending) {
        pendingRef.current.delete(id)
        if (success) {
          pending.resolve(result)
        } else {
          pending.reject(new Error(errMsg))
        }
      }
    }

    worker.onerror = (err) => {
      console.error('[useStorage] Worker error:', err)
      setError(`Worker error: ${err.message}`)
    }

    return () => {
      worker.terminate()
      workerRef.current = null
    }
  }, [])

  /**
   * Workerにメッセージを送信してPromiseで結果を待つ
   */
  const sendToWorker = useCallback(<T,>(type: string, payload: unknown): Promise<T> => {
    return new Promise((resolve, reject) => {
      if (!workerRef.current) {
        reject(new Error('Worker not initialized'))
        return
      }

      const id = `req-${++idCounterRef.current}`
      pendingRef.current.set(id, { resolve: resolve as (v: unknown) => void, reject })
      workerRef.current.postMessage({ id, type, payload })
    })
  }, [])

  /**
   * JSON-RPC呼び出し
   */
  const rpcCall = useCallback(async <T,>(method: string, params: unknown[]): Promise<T> => {
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
  }, [])

  /**
   * リトライ付きRPC呼び出し
   */
  const rpcCallWithRetry = useCallback(async <T,>(
    method: string,
    params: unknown[],
    retries = MAX_RETRIES
  ): Promise<T> => {
    for (let attempt = 1; attempt <= retries; attempt++) {
      try {
        return await rpcCall<T>(method, params)
      } catch (err) {
        if (attempt === retries) throw err
        console.warn(`[useStorage] RPC retry ${attempt}/${retries}:`, err)
        await new Promise(r => setTimeout(r, RETRY_DELAY_MS * attempt))
      }
    }
    throw new Error('Unreachable')
  }, [rpcCall])

  /**
   * コンテンツをSSS分割してアップロード
   */
  const uploadContent = useCallback(async (content: Uint8Array): Promise<UploadResult> => {
    setIsProcessing(true)
    setError(null)
    setProgress(0)

    try {
      // 1. SSS分割 (Wasm Worker)
      setProgress(10)
      const shares = await sendToWorker<Uint8Array[]>('sss_split', {
        data: content,
        k: SSS_K,
        n: SSS_N,
      })

      // 2. MerkleTree構築 (Wasm Worker)
      setProgress(20)
      const merkleResult = await sendToWorker<{ root: Uint8Array; rootHex: string; leafCount: number }>('merkle_build', {
        fragments: shares,
      })
      const merkleRoot = merkleResult.root
      const merkleRootHex = merkleResult.rootHex

      // 3. 各断片のMerkleProofを生成してアップロード
      const fragmentHashes: Uint8Array[] = []
      const progressPerFragment = 70 / SSS_N

      // 並列アップロード（全n個）
      const uploadPromises = shares.map(async (share, index) => {
        // MerkleProof生成
        const proof = await sendToWorker<Uint8Array>('merkle_generate_proof', {
          merkleRootHex,
          index,
        })

        // Base64エンコード
        const dataB64 = btoa(String.fromCharCode.apply(null, Array.from(share)))
        const proofB64 = btoa(String.fromCharCode.apply(null, Array.from(proof)))

        // RPC呼び出し（リトライ付き）
        const result = await rpcCallWithRetry<{ success: boolean; fragment_hash: number[] }>(
          'storage_uploadFragment',
          [{
            merkle_root: Array.from(merkleRoot),
            index,
            data: dataB64,
            proof: proofB64,
            total_leaves: SSS_N,
          }]
        )

        setProgress(prev => Math.min(prev + progressPerFragment, 90))
        return new Uint8Array(result.fragment_hash)
      })

      const results = await Promise.all(uploadPromises)
      fragmentHashes.push(...results)

      setProgress(100)
      return {
        merkleRoot,
        fragmentHashes,
        totalSize: content.length,
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setError(message)
      throw err
    } finally {
      setIsProcessing(false)
    }
  }, [sendToWorker, rpcCallWithRetry])

  /**
   * MerkleRootから断片を取得して復元
   */
  const recoverContent = useCallback(async (
    merkleRoot: Uint8Array,
    k: number = SSS_K,
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    n: number = SSS_N
  ): Promise<RecoverResult> => {
    setIsProcessing(true)
    setError(null)
    setProgress(0)

    try {
      // 1. k個以上の断片を取得
      const shares: Uint8Array[] = []
      const progressPerFragment = 70 / k

      for (let index = 0; index < k; index++) {
        try {
          const result = await rpcCallWithRetry<{ data: string; hash: number[] }>(
            'storage_getFragment',
            [{
              merkle_root: Array.from(merkleRoot),
              index,
            }]
          )

          // Base64デコード
          const data = Uint8Array.from(atob(result.data), c => c.charCodeAt(0))
          shares.push(data)
          setProgress(prev => Math.min(prev + progressPerFragment, 70))
        } catch (err) {
          console.warn(`[useStorage] Failed to get fragment ${index}:`, err)
          // 次の断片を試す（可用性向上）
        }
      }

      if (shares.length < k) {
        throw new Error(`Insufficient fragments: got ${shares.length}, need ${k}`)
      }

      // 2. SSS復元 (Wasm Worker)
      setProgress(80)
      const recovered = await sendToWorker<Uint8Array>('sss_recover', {
        shares,
        k,
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
  }, [sendToWorker, rpcCallWithRetry])

  return {
    uploadContent,
    recoverContent,
    progress,
    error,
    isProcessing,
    isReady,
  }
}

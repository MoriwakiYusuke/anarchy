'use client'

import { useState, useCallback, useRef, useEffect } from 'react'
import { blake2b } from 'blakejs'

/**
 * Storage RPC エンドポイント
 * blockchain nodeがstorage RPCをプロキシする
 */
const RPC_ENDPOINT = process.env.NEXT_PUBLIC_WS_ENDPOINT?.replace('ws://', 'http://').replace('wss://', 'https://') || 'http://127.0.0.1:9944'

/**
 * ハイブリッド分割設定（システム固定値）
 * - AES-256-GCM暗号化 + Reed-Solomon符号化 + キーSSS分割
 */
const SSS_K = 3  // 復元に必要な最小断片数（閾値）
const SSS_N = 5  // 総断片数

/**
 * リトライ設定
 */
const MAX_RETRIES = 3
const RETRY_DELAY_MS = 1000

/**
 * 署名付きリクエスト（Storage Node認証用）
 * X-Anarchy-Auth ヘッダーに含まれるJSON構造体
 */
export interface SignedAuth {
  /** Sr25519公開鍵（hex 32バイト） */
  account_id: string
  /** Unixタイムスタンプ（秒） */
  timestamp: number
  /** ランダムnonce（hex 16バイト） */
  nonce: string
  /** リクエストボディのBlake2bハッシュ（hex 32バイト） */
  payload_hash: string
  /** Sr25519署名（hex 64バイト） */
  signature: string
}

/**
 * 署名用キーペア（Sr25519）
 */
export interface StorageSigner {
  /** 公開鍵（32バイト） */
  publicKey: Uint8Array
  /** 署名関数 */
  sign: (message: Uint8Array) => Uint8Array
}

/**
 * derivation path（例：'//Alice'）からStorageSignerを作成
 * @param derivePath シードフレーズまたはderivation path
 * @returns StorageSignerを返すPromise
 */
export async function createStorageSigner(derivePath: string): Promise<StorageSigner> {
  const { Keyring } = await import('@polkadot/keyring')
  const { DEV_PHRASE } = await import('@polkadot/keyring/defaults')
  
  const keyring = new Keyring({ type: 'sr25519' })
  let pair
  
  if (derivePath.startsWith('//')) {
    // Development derivation path (e.g., //Alice, //Bob)
    pair = keyring.addFromUri(`${DEV_PHRASE}${derivePath}`)
  } else {
    // Real seed phrase (12/24 words mnemonic)
    pair = keyring.addFromUri(derivePath)
  }
  
  return {
    publicKey: pair.publicKey,
    sign: (message: Uint8Array) => pair.sign(message)
  }
}

/**
 * 断片アップロード結果
 */
export interface UploadResult {
  merkleRoot: Uint8Array
  fragmentHashes: Uint8Array[]
  shardHashes: Uint8Array[]
  totalSize: number
  /** ハイブリッド復元用メタデータ */
  metadata: HybridMetadata
}

/**
 * ハイブリッド分割メタデータ（復元に必要）
 */
export interface HybridMetadata {
  originalLen: number
  ciphertextLen: number
  shardSize: number
  compressed: boolean
  threshold: number
  totalShards: number
}

/**
 * 復元結果
 */
export interface RecoverResult {
  data: Uint8Array
}

/**
 * useStorage hookのオプション
 */
export interface UseStorageOptions {
  /** 署名用キーペア（認証が必要な場合） */
  signer?: StorageSigner
}

/**
 * useStorage hookの戻り値
 */
export interface UseStorageResult {
  /** コンテンツをハイブリッド分割してアップロード */
  uploadContent: (content: Uint8Array) => Promise<UploadResult>
  /** MerkleRootから断片を取得して復元（メタデータ必須） */
  recoverContent: (merkleRoot: Uint8Array, metadata: HybridMetadata) => Promise<RecoverResult>
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
 * バイト配列をhex文字列に変換
 */
function toHex(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('')
}

/**
 * 認証用のSignedAuthを生成
 * @param signer 署名用キーペア
 * @param params リクエストパラメータオブジェクト
 */
function generateAuth(signer: StorageSigner, params: Record<string, unknown>): SignedAuth {
  // タイムスタンプ（秒）
  const timestamp = Math.floor(Date.now() / 1000)
  
  // ランダムnonce（16バイト）
  const nonceBytes = new Uint8Array(16)
  crypto.getRandomValues(nonceBytes)
  
  // ペイロードハッシュ（Blake2b-256）
  // キーをアルファベット順にソートしてJSON.stringify（serde_jsonと同じ順序）
  const sortedParams = Object.keys(params).sort().reduce((acc, key) => {
    acc[key] = params[key]
    return acc
  }, {} as Record<string, unknown>)
  const payload = JSON.stringify(sortedParams)
  const payloadBytes = new TextEncoder().encode(payload)
  const payloadHash = blake2b(payloadBytes, undefined, 32)
  
  // 署名対象メッセージ: account_id || timestamp || nonce || payload_hash
  const message = new Uint8Array(32 + 8 + 16 + 32)
  message.set(signer.publicKey, 0)
  const view = new DataView(message.buffer)
  view.setBigUint64(32, BigInt(timestamp), true) // little-endian
  message.set(nonceBytes, 40)
  message.set(payloadHash, 56)
  
  // Sr25519署名
  const signature = signer.sign(message)
  
  return {
    account_id: toHex(signer.publicKey),
    timestamp,
    nonce: toHex(nonceBytes),
    payload_hash: toHex(payloadHash),
    signature: toHex(signature),
  }
}

/**
 * 分散ストレージ操作Hook
 *
 * - SSS分割/復元（Wasm: Web Worker）
 * - MerkleTree構築/検証（Wasm: Web Worker）
 * - 断片アップロード/取得（RPC: storage_uploadFragment, storage_getFragment）
 * 
 * @param options オプション（署名用キーペア等）
 */
export function useStorage(options: UseStorageOptions = {}): UseStorageResult {
  const { signer } = options
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
   * コンテンツをハイブリッド分割してアップロード
   */
  const uploadContent = useCallback(async (content: Uint8Array): Promise<UploadResult> => {
    setIsProcessing(true)
    setError(null)
    setProgress(0)

    try {
      // 1. ハイブリッド分割 (Wasm Worker)
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
        data: content,
        k: SSS_K,
        n: SSS_N,
      })
      const { shards, shardHashes, originalLen, ciphertextLen, shardSize, compressed, threshold, totalShards } = splitResult

      // 2. MerkleTree構築 (Wasm Worker)
      setProgress(20)
      const merkleResult = await sendToWorker<{ root: Uint8Array; rootHex: string; leafCount: number }>('merkle_build', {
        fragments: shards,
      })
      const merkleRoot = merkleResult.root
      const merkleRootHex = merkleResult.rootHex

      // 3. 各断片のMerkleProofを生成してアップロード
      const fragmentHashes: Uint8Array[] = []
      const progressPerFragment = 70 / SSS_N

      // 並列アップロード（全n個）
      const uploadPromises = shards.map(async (share, index) => {
        // MerkleProof生成
        const proof = await sendToWorker<Uint8Array>('merkle_generate_proof', {
          merkleRootHex,
          index,
        })

        // Base64エンコード
        const dataB64 = btoa(String.fromCharCode.apply(null, Array.from(share)))
        const proofB64 = btoa(String.fromCharCode.apply(null, Array.from(proof)))

        // ベースリクエストパラメータ（authなし）
        const baseParams = {
          merkle_root: Array.from(merkleRoot),
          index,
          data: dataB64,
          proof: proofB64,
          total_leaves: SSS_N,
        }

        // リトライ付きアップロード（各リトライでauthを再生成）
        for (let attempt = 1; attempt <= MAX_RETRIES; attempt++) {
          try {
            // リクエストパラメータを毎回作成
            const requestParams: Record<string, unknown> = { ...baseParams }
            
            // 認証情報を追加（signerが提供されている場合、毎回新しいnonceで生成）
            if (signer) {
              requestParams.auth = generateAuth(signer, baseParams)
            }

            const result = await rpcCall<{ success: boolean; fragment_hash: number[] }>(
              'storage_uploadFragment',
              [requestParams]
            )
            
            setProgress(prev => Math.min(prev + progressPerFragment, 90))
            return new Uint8Array(result.fragment_hash)
          } catch (err) {
            if (attempt === MAX_RETRIES) throw err
            console.warn(`[useStorage] Upload retry ${attempt}/${MAX_RETRIES}:`, err)
            await new Promise(r => setTimeout(r, RETRY_DELAY_MS * attempt))
          }
        }
        throw new Error('Unreachable')
      })

      const results = await Promise.all(uploadPromises)
      fragmentHashes.push(...results)

      setProgress(100)
      return {
        merkleRoot,
        fragmentHashes,
        shardHashes,
        totalSize: content.length,
        metadata: {
          originalLen,
          ciphertextLen,
          shardSize,
          compressed,
          threshold,
          totalShards,
        },
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setError(message)
      throw err
    } finally {
      setIsProcessing(false)
    }
  }, [sendToWorker, rpcCall, signer])

  /**
   * MerkleRootから断片を取得して復元（メタデータ必須）
   */
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

      console.log(`[useStorage] Recovering content: merkleRoot=${Array.from(merkleRoot).map(b => b.toString(16).padStart(2, '0')).join('')}, k=${k}, n=${n}`)

      for (let index = 0; index < n && shardBytes.length < k; index++) {
        try {
          const params = {
            merkle_root: Array.from(merkleRoot),
            index,
          }
          console.log(`[useStorage] Requesting fragment ${index}:`, JSON.stringify(params))
          
          const result = await rpcCallWithRetry<{ data: string; hash: number[] }>(
            'storage_getFragment',
            [params]
          )

          // Base64デコード
          const data = Uint8Array.from(atob(result.data), c => c.charCodeAt(0))
          shardBytes.push(data)
          setProgress(prev => Math.min(prev + progressPerFragment, 70))
        } catch (err) {
          console.warn(`[useStorage] Failed to get fragment ${index}:`, err)
          // 次の断片を試す（可用性向上）
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

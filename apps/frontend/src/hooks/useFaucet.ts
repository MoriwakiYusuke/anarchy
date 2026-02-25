'use client'

import { useState, useCallback, useRef, useEffect } from 'react'
import { PolkadotSigner } from 'polkadot-api/signer'
import { computeChallenge, hexToBytes } from '@/lib/faucet/challenge'
import type { WorkerMessage, MineRequest } from '@/lib/faucet/worker'

/** Timeout for RPC calls in milliseconds (30 seconds) */
const RPC_TIMEOUT_MS = 30_000

/**
 * Wrap a promise with timeout
 */
function withTimeout<T>(promise: Promise<T>, ms: number, message: string): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) => 
      setTimeout(() => reject(new Error(`Timeout: ${message}`)), ms)
    )
  ])
}

export type FaucetStatus = 'idle' | 'mining' | 'submitting' | 'success' | 'error'

export interface FaucetError {
  code: 'AlreadyClaimed' | 'ChallengeExpired' | 'InvalidProof' | 'BlockNotFound' | 'NetworkError' | 'InsufficientBalance'
  message: string
}

export interface FaucetProgress {
  hashRate: number
  elapsed: number
  currentNonce: bigint
}

export interface UseFaucetResult {
  status: FaucetStatus
  error: FaucetError | null
  progress: FaucetProgress | null
  startMining: () => Promise<void>
  cancel: () => void
}

interface UseFaucetOptions {
  client: any
  unsafeApi: any
  account: string | null
  signer: PolkadotSigner | null
  onSuccess?: () => void
}

/**
 * Faucetの状態管理とPoW計算を行うhook
 */
export function useFaucet({ client, unsafeApi, account, signer, onSuccess }: UseFaucetOptions): UseFaucetResult {
  const [status, setStatus] = useState<FaucetStatus>('idle')
  const [error, setError] = useState<FaucetError | null>(null)
  const [progress, setProgress] = useState<FaucetProgress | null>(null)
  
  const workerRef = useRef<Worker | null>(null)
  const blockNumberRef = useRef<number | null>(null)
  
  // Workerクリーンアップ
  useEffect(() => {
    return () => {
      if (workerRef.current) {
        workerRef.current.terminate()
        workerRef.current = null
      }
    }
  }, [])

  // エラーコードをマップ
  const mapPalletError = (errorType: string): FaucetError['code'] => {
    const errorMap: Record<string, FaucetError['code']> = {
      'AlreadyClaimed': 'AlreadyClaimed',
      'ChallengeExpired': 'ChallengeExpired',
      'InvalidProof': 'InvalidProof',
      'BlockNotFound': 'BlockNotFound',
      'InsufficientBalance': 'InsufficientBalance',
    }
    return errorMap[errorType] || 'NetworkError'
  }

  const cancel = useCallback(() => {
    if (workerRef.current) {
      workerRef.current.terminate()
      workerRef.current = null
    }
    setStatus('idle')
    setProgress(null)
  }, [])

  const startMining = useCallback(async () => {
    if (!client || !unsafeApi || !account || !signer) {
      setError({ code: 'NetworkError', message: 'API or account not available' })
      setStatus('error')
      return
    }

    if (status === 'mining' || status === 'submitting') {
      return // Already in progress
    }

    setStatus('mining')
    setError(null)
    setProgress(null)

    try {
      // 1. 最新のファイナライズドブロック情報を取得
      // smoldotライトクライアント対応: chain_getBlockHashがnullを返すため、
      // chain_getFinalizedHead + chain_getHeader を使用
      const blockHash = await withTimeout(
        client._request('chain_getFinalizedHead', []) as Promise<string>,
        RPC_TIMEOUT_MS,
        'chain_getFinalizedHead'
      )
      if (!blockHash) {
        throw new Error('Failed to get finalized head')
      }
      
      const header = await withTimeout(
        client._request('chain_getHeader', [blockHash]) as Promise<{ number: string } | null>,
        RPC_TIMEOUT_MS,
        'chain_getHeader'
      )
      if (!header?.number) {
        throw new Error('Failed to get block header')
      }
      
      // ブロック番号は16進数形式で返されるので変換
      const blockNumber = parseInt(header.number, 16)
      blockNumberRef.current = blockNumber

      // 2. 現在の難易度を計算
      const totalClaims = await withTimeout(
        unsafeApi.query.Faucet.TotalClaims.getValue() as Promise<bigint>,
        RPC_TIMEOUT_MS,
        'Faucet.TotalClaims query'
      ) ?? BigInt(0)
      const baseDifficulty = await unsafeApi.constants.Faucet.BaseDifficulty() as number ?? 18
      const scalingFactor = await unsafeApi.constants.Faucet.DifficultyScalingFactor() as bigint ?? BigInt(1000)
      const maxDifficulty = await unsafeApi.constants.Faucet.MaxDifficulty() as number ?? 28
      
      const scaledClaims = Number(totalClaims) / Number(scalingFactor)
      const difficultyIncrease = scaledClaims > 0 ? Math.floor(Math.log2(1 + scaledClaims)) : 0
      const currentDifficulty = Math.min(baseDifficulty + difficultyIncrease, maxDifficulty)

      // 3. チャレンジを計算
      const blockHashBytes = hexToBytes(blockHash)
      
      // AccountIdをバイト列に変換（SS58デコード）
      const { decodeAddress } = await import('@polkadot/util-crypto')
      const accountIdBytes = decodeAddress(account)
      
      const challenge = computeChallenge(blockHashBytes, accountIdBytes)

      // 4. Web Workerでマイニング開始
      const worker = new Worker(new URL('@/lib/faucet/worker.ts', import.meta.url))
      workerRef.current = worker

      const noncePromise = new Promise<bigint>((resolve, reject) => {
        worker.onmessage = (event: MessageEvent<WorkerMessage | { type: 'ready' }>) => {
          const message = event.data
          
          if (message.type === 'ready') {
            // Worker準備完了、マイニング開始
            const request: MineRequest = {
              type: 'start',
              challenge,
              difficulty: currentDifficulty,
              startNonce: BigInt(0),
            }
            worker.postMessage(request)
            return
          }
          
          if (message.type === 'progress') {
            setProgress({
              hashRate: message.hashRate,
              elapsed: message.elapsed,
              currentNonce: message.nonce,
            })
            return
          }
          
          if (message.type === 'solution') {
            resolve(message.nonce)
            return
          }
          
          if (message.type === 'error') {
            reject(new Error(message.message))
            return
          }
        }

        worker.onerror = (err) => {
          reject(new Error(`Worker error: ${err.message}`))
        }
      })

      // Workerから解が返ってくるまで待機
      const nonce = await noncePromise
      
      // Workerを終了
      worker.terminate()
      workerRef.current = null

      // 5. トランザクション送信 (unsigned transaction)
      setStatus('submitting')
      setProgress(null)

      const blockNum = blockNumberRef.current!
      
      // Faucet.claim extrinsic呼び出し (unsigned - account is a parameter)
      const tx = unsafeApi.tx.Faucet.claim({
        account: account,
        block_number: blockNum,
        nonce: nonce,
      })

      // Use getBareTx() which returns SCALE-encoded unsigned transaction
      const bareTx = await tx.getBareTx()
      
      // Convert to hex if needed
      let hexExtrinsic: string
      if (typeof bareTx === 'string') {
        hexExtrinsic = bareTx.startsWith('0x') ? bareTx : '0x' + bareTx
      } else if (typeof bareTx?.asHex === 'function') {
        hexExtrinsic = bareTx.asHex()
      } else if (bareTx instanceof Uint8Array) {
        hexExtrinsic = '0x' + Array.from(bareTx).map(b => b.toString(16).padStart(2, '0')).join('')
      } else if (ArrayBuffer.isView(bareTx)) {
        const bytes = new Uint8Array((bareTx as ArrayBufferView).buffer, (bareTx as ArrayBufferView).byteOffset, (bareTx as ArrayBufferView).byteLength)
        hexExtrinsic = '0x' + Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('')
      } else if (bareTx && typeof bareTx.length === 'number') {
        // Array-like with numeric indices
        const bytes: number[] = []
        for (let i = 0; i < bareTx.length; i++) {
          bytes.push(bareTx[i])
        }
        hexExtrinsic = '0x' + bytes.map(b => b.toString(16).padStart(2, '0')).join('')
      } else {
        throw new Error(`Unknown bare tx format: ${typeof bareTx}`)
      }
      
      // Check if already claimed BEFORE submitting
      // This provides immediate feedback for second claim attempts
      try {
        const alreadyClaimed = await withTimeout(
          unsafeApi.query.Faucet.Claims.getValue(account),
          RPC_TIMEOUT_MS,
          'Faucet.Claims pre-check'
        )
        if (alreadyClaimed) {
          throw new Error('AlreadyClaimed: This account has already claimed from the faucet')
        }
      } catch (preCheckErr: any) {
        // If it's our AlreadyClaimed error, rethrow
        if (preCheckErr?.message?.includes('AlreadyClaimed')) {
          throw preCheckErr
        }
        // Otherwise ignore pre-check errors (might not have Claims storage yet)
      }
      
      // Submit transaction
      // Note: smoldot's author_submitExtrinsic returns tx hash immediately,
      // validation happens asynchronously. We verify success by checking storage after.
      let result: any
      try {
        result = await withTimeout(
          client._request('author_submitExtrinsic', [hexExtrinsic]) as Promise<any>,
          RPC_TIMEOUT_MS,
          'author_submitExtrinsic'
        )
      } catch (submitError: any) {
        const message = submitError?.message || submitError?.toString() || 'Submit failed'
        if (
          message.includes('Invalid') || 
          message.includes('Custom(1)') || 
          message.includes('1010') ||
          message.includes('already claimed') ||
          message.includes('dispatch')
        ) {
          throw new Error('AlreadyClaimed: This account has already claimed from the faucet')
        }
        throw submitError
      }
      
      // Check if result indicates an error
      if (result && typeof result === 'object' && 'error' in result) {
        throw new Error('AlreadyClaimed: This account has already claimed from the faucet')
      }

      // smoldot returns tx hash immediately, need to wait and verify claim was recorded
      // Wait for transaction to be included in block (max ~12 seconds = 2 blocks)
      const maxWaitMs = 12000
      const pollInterval = 2000
      let elapsed = 0
      let claimVerified = false
      
      while (elapsed < maxWaitMs && !claimVerified) {
        await new Promise(resolve => setTimeout(resolve, pollInterval))
        elapsed += pollInterval
        
        try {
          const claimed = await withTimeout(
            unsafeApi.query.Faucet.Claims.getValue(account),
            RPC_TIMEOUT_MS,
            'Faucet.Claims verification'
          )
          if (claimed) {
            claimVerified = true
          }
        } catch {
          // Verify error, continue waiting
        }
      }
      
      if (!claimVerified) {
        throw new Error('AlreadyClaimed: This account has already claimed from the faucet')
      }

      // 成功
      setStatus('success')
      onSuccess?.()
      
      // 数秒後にidleに戻る
      setTimeout(() => {
        setStatus('idle')
      }, 3000)

    } catch (err) {
      // Workerクリーンアップ
      if (workerRef.current) {
        workerRef.current.terminate()
        workerRef.current = null
      }

      // エラーメッセージからパレットエラーを抽出
      let errorCode: FaucetError['code'] = 'NetworkError'
      let errorMessage = 'Unknown error'

      if (err instanceof Error) {
        errorMessage = err.message
        
        // パレットエラーを検出
        const palletErrors = ['AlreadyClaimed', 'ChallengeExpired', 'InvalidProof', 'BlockNotFound', 'InsufficientBalance']
        for (const palletError of palletErrors) {
          if (errorMessage.includes(palletError)) {
            errorCode = mapPalletError(palletError)
            break
          }
        }
        
        // Invalid Transaction はValidateUnsignedで拒否された場合
        // 2回目以降の請求 = AlreadyClaimed の可能性が高い
        if (errorMessage.includes('Invalid Transaction') || errorMessage.includes('InvalidTransaction')) {
          errorCode = 'AlreadyClaimed'
          errorMessage = 'This account has already claimed from the faucet'
        }
      }

      setError({ code: errorCode, message: errorMessage })
      setStatus('error')
      
      // 数秒後にidleに戻る
      setTimeout(() => {
        setStatus('idle')
        setError(null)
      }, 5000)
    }
  }, [client, unsafeApi, account, status, onSuccess])

  return {
    status,
    error,
    progress,
    startMining,
    cancel,
  }
}

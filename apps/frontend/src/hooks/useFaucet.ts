'use client'

import { useState, useCallback, useRef, useEffect } from 'react'
import { PolkadotSigner } from 'polkadot-api/signer'
import { computeChallenge, hexToBytes } from '@/lib/faucet/challenge'
import { debugLog, debugWarn, debugError } from '@/lib/debugLog'
import { withTimeout } from '@/lib/withTimeout'
import { minePow, PowCancelledError, type MinePowHandle } from '@/lib/pow/pausableMiner'

/** Timeout for RPC calls in milliseconds.
 *  PoW 移行で block time が 30s に伸びたため、signAndSubmit (finalize 待ち) は
 *  最低 1 ブロック + GRANDPA finalize で 60s 超える。余裕を見て 240s に設定。
 */
const RPC_TIMEOUT_MS = 240_000

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
  
  // minePow ハンドル (Foreground PoW: タブ隠蔽で worker を terminate し、
  // 復帰時に保存済み nonce から自動再開する。lib/pow/pausableMiner 参照)。
  const minerRef = useRef<MinePowHandle | null>(null)
  const blockNumberRef = useRef<number | null>(null)

  // マイナークリーンアップ
  useEffect(() => {
    return () => {
      minerRef.current?.cancel()
      minerRef.current = null
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
    minerRef.current?.cancel()
    minerRef.current = null
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

      // 4. Web Worker でマイニング開始 (Foreground PoW only)。
      // minePow がタブ隠蔽時の terminate / 復帰時の startNonce 再開を扱う。
      const miner = minePow({
        // bundler の静的解析のため new Worker(new URL(...)) はここに直書きする
        createWorker: () => new Worker(new URL('@/lib/faucet/worker.ts', import.meta.url)),
        challenge,
        difficulty: currentDifficulty,
        autoResume: true,
        onProgress: (p) => {
          setProgress({
            hashRate: p.hashRate,
            elapsed: p.elapsed,
            currentNonce: p.nonce,
          })
        },
      })
      minerRef.current = miner

      // 解が返ってくるまで待機 (worker は minePow 側で terminate 済み)
      const { nonce } = await miner.promise
      minerRef.current = null

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
          unsafeApi.query.Faucet.FaucetClaims.getValue(account),
          RPC_TIMEOUT_MS,
          'Faucet.FaucetClaims pre-check'
        )
        if (alreadyClaimed) {
          debugLog('[useFaucet] Account already claimed (pre-check)')
          throw new Error('AlreadyClaimed: This account has already claimed from the faucet')
        }
      } catch (preCheckErr: any) {
        // If it's our AlreadyClaimed error, rethrow
        if (preCheckErr?.message?.includes('AlreadyClaimed')) {
          throw preCheckErr
        }
        // Otherwise ignore pre-check errors (might not have Claims storage yet)
        debugWarn('[useFaucet] Pre-check error (ignoring):', preCheckErr)
      }
      
      // Submit transaction and wait for finalization
      // client.submit() waits until the transaction is finalized or rejected
      try {
        const finalizedResult = await withTimeout(
          client.submit(hexExtrinsic) as Promise<any>,
          RPC_TIMEOUT_MS, // PoW 30s blocktime + finalization lag があるため長め (60s では不足しタイムアウトすることがあった)
          'client.submit'
        )
        debugLog('[useFaucet] Transaction finalized:', finalizedResult)
        
        // Check if the transaction was successful
        if (finalizedResult?.ok === false || finalizedResult?.dispatchError) {
          debugError('[useFaucet] Transaction failed:', finalizedResult)
          throw new Error('AlreadyClaimed: This account has already claimed from the faucet')
        }
      } catch (submitError: any) {
        debugError('[useFaucet] Submit error:', submitError)
        const message = submitError?.message || submitError?.toString() || 'Submit failed'
        if (
          message.includes('Invalid') || 
          message.includes('Custom(1)') || 
          message.includes('1010') ||
          message.includes('already claimed') ||
          message.includes('dispatch') ||
          message.includes('AlreadyClaimed')
        ) {
          throw new Error('AlreadyClaimed: This account has already claimed from the faucet')
        }
        throw submitError
      }

      // 成功
      setStatus('success')
      onSuccess?.()
      
      // 数秒後にidleに戻る
      setTimeout(() => {
        setStatus('idle')
      }, 3000)

    } catch (err) {
      // cancel() による中断は正常系 (cancel 側で idle に戻している)
      if (err instanceof PowCancelledError) {
        return
      }

      debugError('[useFaucet] Faucet error:', err)

      // マイナークリーンアップ
      minerRef.current?.cancel()
      minerRef.current = null

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
    // (#40-MEDIUM-1) include `signer` so that switching account regenerates startMining
  }, [client, unsafeApi, account, signer, status, onSuccess])

  return {
    status,
    error,
    progress,
    startMining,
    cancel,
  }
}

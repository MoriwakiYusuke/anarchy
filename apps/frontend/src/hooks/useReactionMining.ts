/**
 * useReactionMining Hook
 *
 * Manages the reaction mining workflow including:
 * - PoW mining in WebWorker (via lib/pow/pausableMiner)
 * - Page Visibility API monitoring (foreground enforcement)
 *   タブが隠れたら worker を terminate して計算を完全停止し、直近 nonce を保存。
 *   resume() で同じ nonce から再開する (CLAUDE.md Security Principle #4)。
 * - Mining progress tracking
 * - Transaction submission
 *
 * Feature: 017-reaction-mining
 */

'use client'

import { useState, useCallback, useRef, useEffect } from 'react'
import type { PolkadotSigner } from 'polkadot-api/signer'
import {
  getReactionChallenge,
  submitReaction,
  ReactionType,
  ReactionResult,
} from '@/services/reactionService'
import {
  minePow,
  PowPausedError,
  PowCancelledError,
  type MinePowHandle,
} from '@/lib/pow/pausableMiner'
import { debugError, debugLog } from '@/lib/debugLog'

/** Mining status states */
export type MiningStatus = 'idle' | 'mining' | 'paused' | 'submitting' | 'success' | 'error'

/** Mining progress information */
export interface MiningProgress {
  /** Hashes computed per second */
  hashRate: number
  /** Elapsed mining time in milliseconds */
  elapsedMs: number
  /** Current nonce being tested */
  currentNonce: bigint
  /** Required difficulty in leading zero bits */
  difficulty: number
}

/** Mining error information */
export interface MiningError {
  code: 'AlreadyReacted' | 'InvalidPoW' | 'ChallengeExpired' | 'VisibilityPaused' | 'NetworkError'
  message: string
}

/** Hook options */
export interface UseReactionMiningOptions {
  /** PAPI client for RPC calls */
  client: any
  /** PAPI unsafe API for pallet queries */
  unsafeApi: any
  /** User's account address */
  account: string | null
  /** Polkadot signer for transaction signing */
  signer: PolkadotSigner | null
  /** Callback on successful reaction */
  onSuccess?: (result: ReactionResult) => void
  /** Callback on error */
  onError?: (error: MiningError) => void
}

/** Hook return value */
export interface UseReactionMiningResult {
  /** Current mining status */
  status: MiningStatus
  /** Error information if status is 'error' */
  error: MiningError | null
  /** Mining progress if status is 'mining' */
  progress: MiningProgress | null
  /** Result after successful submission */
  result: ReactionResult | null
  /** Start mining for a reaction */
  startMining: (postId: bigint, reactionType: ReactionType) => Promise<void>
  /** Cancel mining */
  cancel: () => void
  /** Resume paused mining */
  resume: () => void
}

interface MiningParams {
  postId: bigint
  reactionType: ReactionType
  challengeBlock: number
  challenge: Uint8Array
  difficulty: number
  /** 中断時に保存した nonce。resume でここから再開する。 */
  startNonce: bigint
}

/**
 * Hook for managing PoW-based reaction mining
 */
export function useReactionMining({
  client,
  unsafeApi,
  account,
  signer,
  onSuccess,
  onError,
}: UseReactionMiningOptions): UseReactionMiningResult {
  const [status, setStatus] = useState<MiningStatus>('idle')
  const [error, setError] = useState<MiningError | null>(null)
  const [progress, setProgress] = useState<MiningProgress | null>(null)
  const [result, setResult] = useState<ReactionResult | null>(null)

  // onmessage closure から最新 status を参照するための ref
  // (solution handler を status === 'mining' でゲートする)。
  const statusRef = useRef<MiningStatus>('idle')
  const updateStatus = useCallback((next: MiningStatus) => {
    statusRef.current = next
    setStatus(next)
  }, [])

  const handleRef = useRef<MinePowHandle | null>(null)
  const miningParamsRef = useRef<MiningParams | null>(null)

  // Cleanup miner on unmount
  useEffect(() => {
    return () => {
      handleRef.current?.cancel()
      handleRef.current = null
    }
  }, [])

  /**
   * Cancel ongoing mining
   */
  const cancel = useCallback(() => {
    miningParamsRef.current = null
    // minePow handle 経由で worker を terminate (即時停止)
    handleRef.current?.cancel()
    handleRef.current = null
    updateStatus('idle')
    setProgress(null)
    setError(null)
  }, [updateStatus])

  /**
   * Handle mining success - submit transaction
   */
  const handleMiningSuccess = useCallback(async (
    nonce: bigint,
    hashRate: number,
    elapsed: number,
    postId: bigint,
    reactionType: ReactionType,
    challengeBlock: number,
    difficulty: number
  ) => {
    // Validate all required params
    if (nonce === undefined || postId === undefined || challengeBlock === undefined) {
      const err: MiningError = {
        code: 'NetworkError',
        message: `Invalid params: nonce=${nonce}, postId=${postId}, challengeBlock=${challengeBlock}`,
      }
      setError(err)
      updateStatus('error')
      onError?.(err)
      return
    }

    updateStatus('submitting')
    setProgress({
      hashRate,
      elapsedMs: elapsed,
      currentNonce: nonce,
      difficulty,
    })

    try {
      const txResult = await submitReaction(unsafeApi, signer!, {
        postId,
        reactionType,
        nonce,
        challengeBlock,
      })

      if (txResult.success) {
        setResult(txResult)
        updateStatus('success')
        onSuccess?.(txResult)
      } else {
        const err: MiningError = {
          code: txResult.error?.includes('AlreadyReacted') ? 'AlreadyReacted' :
                txResult.error?.includes('InvalidPoW') ? 'InvalidPoW' :
                txResult.error?.includes('Expired') ? 'ChallengeExpired' : 'NetworkError',
          message: txResult.error || 'Transaction failed',
        }
        setError(err)
        updateStatus('error')
        onError?.(err)
      }
    } catch (txErr) {
      const errMessage = txErr instanceof Error ? txErr.message : 'Transaction submission failed'
      // faucet-style: detect AlreadyReacted from exception message
      const isAlreadyReacted =
        errMessage.includes('AlreadyReacted') ||
        errMessage.includes('Invalid Transaction') ||
        errMessage.includes('InvalidTransaction') ||
        errMessage.includes('Custom(1)') ||
        errMessage.includes('1010')
      const err: MiningError = {
        code: isAlreadyReacted ? 'AlreadyReacted' : 'NetworkError',
        message: errMessage,
      }
      setError(err)
      updateStatus('error')
      onError?.(err)
    }

    miningParamsRef.current = null
  }, [unsafeApi, signer, onSuccess, onError, updateStatus])

  /**
   * minePow でマイニングを 1 セッション実行する。
   * - progress ごとに最新 nonce を miningParamsRef に保存 (pause/resume 用)。
   * - タブが隠れたら PowPausedError → 'paused' に遷移 (worker は terminate 済み)。
   */
  const runMining = useCallback((params: MiningParams) => {
    // 既存セッションがあれば止める (PowCancelledError は下の catch で無視される)
    handleRef.current?.cancel()
    miningParamsRef.current = params

    const handle = minePow({
      // bundler の静的解析のため new Worker(new URL(...)) はここに直書きする
      createWorker: () =>
        new Worker(new URL('@/lib/reaction/miningWorker.ts', import.meta.url)),
      challenge: params.challenge,
      difficulty: params.difficulty,
      startNonce: params.startNonce,
      autoResume: false,
      onProgress: (p) => {
        // 最新 nonce を保存 (hide 時の resume 用)
        if (miningParamsRef.current) {
          miningParamsRef.current = { ...miningParamsRef.current, startNonce: p.nonce }
        }
        setProgress({
          hashRate: p.hashRate,
          elapsedMs: p.elapsed,
          currentNonce: p.nonce,
          difficulty: params.difficulty,
        })
      },
    })
    handleRef.current = handle

    handle.promise
      .then((solution) => {
        // Foreground PoW gate: mining 中でなければ solution を提出しない
        if (statusRef.current !== 'mining') {
          debugLog('[useReactionMining] solution ignored (status != mining)')
          return
        }
        debugLog('[useReactionMining] solution received')
        return handleMiningSuccess(
          solution.nonce,
          solution.hashRate,
          solution.elapsed,
          params.postId,
          params.reactionType,
          params.challengeBlock,
          params.difficulty,
        )
      })
      .catch((err) => {
        if (err instanceof PowCancelledError) {
          return // cancel() 側で状態は処理済み
        }
        if (err instanceof PowPausedError) {
          // タブ隠蔽による中断: worker は terminate 済み。nonce を保存して paused へ。
          if (miningParamsRef.current) {
            miningParamsRef.current = {
              ...miningParamsRef.current,
              startNonce: err.lastNonce,
            }
          }
          updateStatus('paused')
          setError({
            code: 'VisibilityPaused',
            message: 'Mining paused - return to this tab to continue',
          })
          return
        }
        const miningError: MiningError = {
          code: 'NetworkError',
          message: err instanceof Error ? err.message : 'Mining failed',
        }
        setError(miningError)
        updateStatus('error')
        onError?.(miningError)
      })
  }, [handleMiningSuccess, onError, updateStatus])

  /**
   * Resume paused mining
   */
  const resume = useCallback(async () => {
    if (statusRef.current !== 'paused' || !miningParamsRef.current) {
      return
    }

    // Re-get challenge (may have expired)
    const params = miningParamsRef.current
    try {
      const { challenge, blockNumber, difficulty } = await getReactionChallenge(
        client,
        unsafeApi,
        params.postId,
        account!
      )

      // Update params with new challenge (保存済み nonce から再開)
      const updated: MiningParams = {
        ...params,
        challenge,
        challengeBlock: blockNumber,
        difficulty,
      }

      // Restart mining
      updateStatus('mining')
      setError(null)
      runMining(updated)
    } catch (err) {
      setError({
        code: 'NetworkError',
        message: err instanceof Error ? err.message : 'Failed to resume mining',
      })
    }
  }, [client, unsafeApi, account, runMining, updateStatus])

  /**
   * Start mining for a reaction
   */
  const startMining = useCallback(async (
    postId: bigint,
    reactionType: ReactionType
  ) => {
    if (!client || !unsafeApi || !account || !signer) {
      const err: MiningError = { code: 'NetworkError', message: 'API or account not available' }
      setError(err)
      updateStatus('error')
      onError?.(err)
      return
    }

    if (statusRef.current === 'mining' || statusRef.current === 'submitting') {
      return // Already in progress
    }

    updateStatus('mining')
    setError(null)
    setProgress(null)
    setResult(null)

    try {
      // Get challenge from chain
      const { challenge, blockNumber, difficulty } = await getReactionChallenge(
        client,
        unsafeApi,
        postId,
        account
      )

      // Initialize progress
      setProgress({
        hashRate: 0,
        elapsedMs: 0,
        currentNonce: BigInt(0),
        difficulty,
      })

      runMining({
        postId,
        reactionType,
        challengeBlock: blockNumber,
        challenge,
        difficulty,
        startNonce: 0n,
      })
    } catch (err) {
      debugError('[useReactionMining] startMining error:', err)
      const miningError: MiningError = {
        code: 'NetworkError',
        message: err instanceof Error ? err.message : 'Failed to start mining',
      }
      setError(miningError)
      updateStatus('error')
      onError?.(miningError)
    }
  }, [client, unsafeApi, account, signer, runMining, onError, updateStatus])

  return {
    status,
    error,
    progress,
    result,
    startMining,
    cancel,
    resume,
  }
}

// Re-export ReactionType for convenience
export { ReactionType }

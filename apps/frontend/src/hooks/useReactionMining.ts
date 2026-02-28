/**
 * useReactionMining Hook
 * 
 * Manages the reaction mining workflow including:
 * - PoW mining in WebWorker
 * - Page Visibility API monitoring (foreground enforcement)
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
  
  const workerRef = useRef<Worker | null>(null)
  const pendingRequestIdRef = useRef<string | null>(null)
  const miningParamsRef = useRef<{
    postId: bigint
    reactionType: ReactionType
    challengeBlock: number
    challenge: Uint8Array
    difficulty: number
  } | null>(null)
  
  // Cleanup worker on unmount
  useEffect(() => {
    return () => {
      if (workerRef.current) {
        workerRef.current.terminate()
        workerRef.current = null
      }
    }
  }, [])
  
  // Page Visibility API monitoring for foreground enforcement (FR-202)
  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.hidden && status === 'mining') {
        // Pause mining when page is hidden
        setStatus('paused')
        setError({
          code: 'VisibilityPaused',
          message: 'Mining paused - return to this tab to continue',
        })
      }
    }
    
    document.addEventListener('visibilitychange', handleVisibilityChange)
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange)
    }
  }, [status])
  
  /**
   * Create a new mining worker (faucet-style, new instance each time)
   */
  const createWorker = useCallback((): Worker => {
    // Terminate existing worker if any
    if (workerRef.current) {
      workerRef.current.terminate()
      workerRef.current = null
    }
    // Create new dedicated worker (use @ alias like faucet, no type: 'module')
    const worker = new Worker(
      new URL('@/lib/reaction/miningWorker.ts', import.meta.url)
    )
    workerRef.current = worker
    return worker
  }, [])
  
  /**
   * Cancel ongoing mining
   */
  const cancel = useCallback(() => {
    pendingRequestIdRef.current = null
    miningParamsRef.current = null
    // Terminate worker to stop mining immediately
    if (workerRef.current) {
      workerRef.current.terminate()
      workerRef.current = null
    }
    setStatus('idle')
    setProgress(null)
    setError(null)
  }, [])
  
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
    console.log('[handleMiningSuccess] called with:', {
      nonce: nonce?.toString(),
      hashRate,
      elapsed,
      postId: postId?.toString(),
      reactionType,
      challengeBlock,
      difficulty,
    })
    
    // Validate all required params
    if (nonce === undefined || postId === undefined || challengeBlock === undefined) {
      const err: MiningError = {
        code: 'NetworkError',
        message: `Invalid params: nonce=${nonce}, postId=${postId}, challengeBlock=${challengeBlock}`,
      }
      setError(err)
      setStatus('error')
      onError?.(err)
      return
    }
    
    setStatus('submitting')
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
        setStatus('success')
        onSuccess?.(txResult)
      } else {
        const err: MiningError = {
          code: txResult.error?.includes('AlreadyReacted') ? 'AlreadyReacted' :
                txResult.error?.includes('InvalidPoW') ? 'InvalidPoW' :
                txResult.error?.includes('Expired') ? 'ChallengeExpired' : 'NetworkError',
          message: txResult.error || 'Transaction failed',
        }
        setError(err)
        setStatus('error')
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
      setStatus('error')
      onError?.(err)
    }
    
    miningParamsRef.current = null
    pendingRequestIdRef.current = null
  }, [unsafeApi, signer, onSuccess, onError])
  
  /**
   * Resume paused mining
   */
  const resume = useCallback(async () => {
    if (status !== 'paused' || !miningParamsRef.current) {
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
      
      // Update params with new challenge
      miningParamsRef.current = {
        ...params,
        challenge,
        challengeBlock: blockNumber,
        difficulty,
      }
      
      // Restart mining
      setStatus('mining')
      setError(null)
      
      const worker = createWorker()
      
      // Set up message handler (faucet-style with ready signal)
      worker.onmessage = (event: MessageEvent) => {
        const message = event.data
        
        if (message.type === 'ready') {
          // Worker準備完了、マイニング開始 (send startNonce as string)
          worker.postMessage({
            type: 'start',
            challenge,
            difficulty,
            startNonce: '0',
          })
          return
        }
        
        if (message.type === 'progress') {
          setProgress({
            hashRate: message.hashRate,
            elapsedMs: message.elapsed,
            currentNonce: BigInt(message.nonce), // Parse from string
            difficulty,
          })
          return
        }
        
        if (message.type === 'error') {
          const err: MiningError = { code: 'NetworkError', message: message.message }
          setError(err)
          setStatus('error')
          onError?.(err)
          return
        }
        
        if (message.type === 'solution') {
          // Mining succeeded - submit transaction
          const nonce = BigInt(message.nonce) // Parse from string
          handleMiningSuccess(nonce, message.hashRate, message.elapsed, params.postId, params.reactionType, blockNumber, difficulty)
        }
      }
    } catch (err) {
      setError({
        code: 'NetworkError',
        message: err instanceof Error ? err.message : 'Failed to resume mining',
      })
    }
  }, [status, client, unsafeApi, account, createWorker, handleMiningSuccess, onError])
  
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
      setStatus('error')
      onError?.(err)
      return
    }
    
    if (status === 'mining' || status === 'submitting') {
      return // Already in progress
    }
    
    setStatus('mining')
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
      
      // Store params for potential resume
      miningParamsRef.current = {
        postId,
        reactionType,
        challengeBlock: blockNumber,
        challenge,
        difficulty,
      }
      
      // Initialize progress
      setProgress({
        hashRate: 0,
        elapsedMs: 0,
        currentNonce: BigInt(0),
        difficulty,
      })
      
      // Create new worker (faucet-style)
      const worker = createWorker()
      
      // Set up message handler (faucet-style with ready signal)
      worker.onmessage = (event: MessageEvent) => {
        const message = event.data
        
        if (message.type === 'ready') {
          // Worker準備完了、マイニング開始 (send startNonce as string)
          worker.postMessage({
            type: 'start',
            challenge,
            difficulty,
            startNonce: '0',
          })
          return
        }
        
        if (message.type === 'progress') {
          setProgress({
            hashRate: message.hashRate,
            elapsedMs: message.elapsed,
            currentNonce: BigInt(message.nonce), // Parse from string
            difficulty,
          })
          return
        }
        
        if (message.type === 'error') {
          const err: MiningError = { code: 'NetworkError', message: message.message }
          setError(err)
          setStatus('error')
          onError?.(err)
          return
        }
        
        if (message.type === 'solution') {
          // Mining succeeded - submit transaction
          console.log('[useReactionMining] solution received:', {
            messageNonce: message.nonce,
            messageHashRate: message.hashRate,
            messageElapsed: message.elapsed,
            closurePostId: postId?.toString(),
            closureReactionType: reactionType,
            closureBlockNumber: blockNumber,
            closureDifficulty: difficulty,
          })
          const nonce = BigInt(message.nonce) // Parse from string
          handleMiningSuccess(
            nonce,
            message.hashRate,
            message.elapsed,
            postId,
            reactionType,
            blockNumber,
            difficulty
          )
        }
      }
      
    } catch (err) {
      console.error('[useReactionMining] startMining error:', err)
      const miningError: MiningError = {
        code: 'NetworkError',
        message: err instanceof Error ? err.message : 'Failed to start mining',
      }
      setError(miningError)
      setStatus('error')
      onError?.(miningError)
    }
  }, [client, unsafeApi, account, signer, status, createWorker, handleMiningSuccess, onError])
  
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

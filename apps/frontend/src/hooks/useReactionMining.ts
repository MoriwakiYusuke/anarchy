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
   * Initialize or get the crypto worker
   */
  const getWorker = useCallback((): Worker => {
    if (!workerRef.current) {
      workerRef.current = new Worker(
        new URL('../workers/crypto.ts', import.meta.url),
        { type: 'module' }
      )
    }
    return workerRef.current
  }, [])
  
  /**
   * Cancel ongoing mining
   */
  const cancel = useCallback(() => {
    pendingRequestIdRef.current = null
    miningParamsRef.current = null
    setStatus('idle')
    setProgress(null)
    setError(null)
  }, [])
  
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
      
      const worker = getWorker()
      const requestId = `mine_${Date.now()}`
      pendingRequestIdRef.current = requestId
      
      worker.postMessage({
        id: requestId,
        type: 'mine_reaction',
        payload: {
          challenge,
          difficulty,
          maxIterations: 0, // No limit
        },
      })
    } catch (err) {
      setError({
        code: 'NetworkError',
        message: err instanceof Error ? err.message : 'Failed to resume mining',
      })
    }
  }, [status, client, unsafeApi, account, getWorker])
  
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
      
      // Start mining in worker
      const worker = getWorker()
      const requestId = `mine_${Date.now()}`
      pendingRequestIdRef.current = requestId
      
      // Set up message handler
      worker.onmessage = async (event: MessageEvent) => {
        const { id, success, result: miningResult, error: miningError } = event.data
        
        // Ignore responses for cancelled requests
        if (id !== pendingRequestIdRef.current) {
          return
        }
        
        if (!success) {
          const err: MiningError = { code: 'NetworkError', message: miningError || 'Mining failed' }
          setError(err)
          setStatus('error')
          onError?.(err)
          return
        }
        
        // Mining succeeded - submit transaction
        setStatus('submitting')
        
        const { nonce, hashRate, elapsedMs } = miningResult as {
          nonce: bigint
          iterations: number
          hashRate: number
          elapsedMs: number
        }
        
        setProgress({
          hashRate,
          elapsedMs,
          currentNonce: nonce,
          difficulty,
        })
        
        try {
          const txResult = await submitReaction(unsafeApi, signer!, {
            postId,
            reactionType,
            nonce,
            challengeBlock: blockNumber,
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
          const err: MiningError = {
            code: 'NetworkError',
            message: txErr instanceof Error ? txErr.message : 'Transaction submission failed',
          }
          setError(err)
          setStatus('error')
          onError?.(err)
        }
        
        miningParamsRef.current = null
        pendingRequestIdRef.current = null
      }
      
      // Send mining request
      worker.postMessage({
        id: requestId,
        type: 'mine_reaction',
        payload: {
          challenge,
          difficulty,
          maxIterations: 0, // No limit
        },
      })
      
    } catch (err) {
      const miningError: MiningError = {
        code: 'NetworkError',
        message: err instanceof Error ? err.message : 'Failed to start mining',
      }
      setError(miningError)
      setStatus('error')
      onError?.(miningError)
    }
  }, [client, unsafeApi, account, signer, status, getWorker, onSuccess, onError])
  
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

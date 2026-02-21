'use client'

// useStorageStatus: Storage operation status management (T065)
// Extracted from useStorage for Issue 13 decomposition

import { useState, useCallback } from 'react'
import { getSharedWorkerPool } from '@/workers/WorkerPool'

export interface StorageStatus {
  /** Progress percentage (0-100) */
  progress: number
  /** Error message if any */
  error: string | null
  /** Operation in progress */
  isProcessing: boolean
  /** Worker pool ready */
  isReady: boolean
}

export interface UseStorageStatusResult extends StorageStatus {
  /** Set progress value */
  setProgress: (value: number) => void
  /** Set error message */
  setError: (message: string | null) => void
  /** Set processing state */
  setIsProcessing: (value: boolean) => void
  /** Reset all status to initial values */
  reset: () => void
}

/**
 * Storage operation status management hook.
 * Tracks progress, errors, and processing state.
 */
export function useStorageStatus(): UseStorageStatusResult {
  const [progress, setProgress] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [isProcessing, setIsProcessing] = useState(false)

  // Check WorkerPool readiness
  const pool = typeof window !== 'undefined' ? getSharedWorkerPool() : null
  const isReady = pool?.isReady ?? false

  const reset = useCallback(() => {
    setProgress(0)
    setError(null)
    setIsProcessing(false)
  }, [])

  return {
    progress,
    error,
    isProcessing,
    isReady,
    setProgress,
    setError,
    setIsProcessing,
    reset,
  }
}

export default useStorageStatus

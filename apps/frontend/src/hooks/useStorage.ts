'use client'

// useStorage: Facade for split storage hooks (T066)
// Re-exports upload and recovery functionality while maintaining backward compatibility

import { useUpload, type UseUploadOptions, type UseUploadResult, type UploadResult, type HybridMetadata, type StorageSigner, type SignedAuth, createStorageSigner } from './useUpload'
import { useFragments, type UseFragmentsResult, type RecoverResult } from './useFragments'
export { useProofSubmission, type UseProofSubmissionResult, type ProofSubmissionResult } from './useProofSubmission'
export { useStorageStatus, type UseStorageStatusResult, type StorageStatus } from './useStorageStatus'

// Re-export types from split hooks
export type { UploadResult, HybridMetadata, StorageSigner, SignedAuth, RecoverResult }
export { createStorageSigner }

/** useStorage options (backward compatible) */
export interface UseStorageOptions {
  signer?: StorageSigner
}

/** useStorage result (backward compatible) */
export interface UseStorageResult {
  uploadContent: (content: Uint8Array) => Promise<UploadResult>
  recoverContent: (merkleRoot: Uint8Array, metadata: HybridMetadata) => Promise<RecoverResult>
  progress: number
  error: string | null
  isProcessing: boolean
  isReady: boolean
}

/**
 * Unified storage hook combining upload and recovery.
 * Facade over useUpload and useFragments for backward compatibility.
 */
export function useStorage(options: UseStorageOptions = {}): UseStorageResult {
  const upload = useUpload(options as UseUploadOptions)
  const fragments = useFragments()

  // Use the higher progress value (upload or fragments)
  // This ensures progress remains at 100 after upload completes
  const progress = Math.max(upload.progress, fragments.progress)
  const error = upload.error || fragments.error
  const isProcessing = upload.isProcessing || fragments.isProcessing
  const isReady = upload.isReady && fragments.isReady

  return {
    uploadContent: upload.uploadContent,
    recoverContent: fragments.recoverContent,
    progress,
    error,
    isProcessing,
    isReady,
  }
}

export default useStorage

// Named exports for individual hooks
export { useUpload, useFragments }
export type { UseUploadOptions, UseUploadResult, UseFragmentsResult }

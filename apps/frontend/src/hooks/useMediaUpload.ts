/**
 * useMediaUpload Hook
 * 
 * T-051: Media upload hook with hybrid_split and storage node upload
 * 
 * Features:
 * - File validation
 * - EXIF stripping
 * - Hybrid split (KZG-VSS)
 * - Storage node shard upload
 * - Progress tracking
 * - Atomic upload mode (all-or-nothing)
 */

'use client'

import { useState, useCallback, useRef } from 'react'
import type {
  MediaFile,
  MediaFileStatus,
  MediaUploadResult,
  UploadState,
} from '@/types/media'
import { MAX_FILES_PER_POST, detectMediaType } from '@/types/media'
import { validateFile, validateFiles } from '@/lib/mediaValidator'
import { processMediaFile } from '@/lib/mediaProcessor'
// Import wasm-engine at top level - this hook is client-only
import { hybrid_split, merkle_build } from 'anarchy-wasm-engine'

// SSS/Reed-Solomon parameters
const DEFAULT_THRESHOLD = 3  // k: minimum shards to reconstruct
const DEFAULT_SHARD_COUNT = 5  // n: total shards

/** Default storage node URL */
const DEFAULT_STORAGE_NODE_URL = process.env.NEXT_PUBLIC_STORAGE_NODE_URL || 'http://localhost:3030'

/** Hook options */
export interface UseMediaUploadOptions {
  /** Storage node RPC URL (default: http://localhost:3030) */
  storageNodeUrl?: string
  /** Strip EXIF data from images */
  stripExif?: boolean
  /** All files must succeed or all fail */
  atomicUpload?: boolean
  /** Maximum files allowed */
  maxFiles?: number
  /** Progress callback */
  onProgress?: (fileId: string, progress: number) => void
  /** Upload complete callback */
  onUploadComplete?: (results: MediaUploadResult[]) => void
  /** Error callback */
  onError?: (error: string) => void
}

/** Hook return type */
export interface UseMediaUploadReturn {
  /** Current files */
  files: MediaFile[]
  /** Overall state */
  state: UploadState
  /** Error message i18n key */
  error: string | null
  /** Add files to upload queue */
  addFiles: (files: File[]) => Promise<void>
  /** Remove a file by ID */
  removeFile: (fileId: string) => void
  /** Clear all files */
  clearAll: () => void
  /** Start uploading all pending files */
  uploadAll: () => Promise<MediaUploadResult[]>
  /** Retry failed uploads */
  retryFailed: () => Promise<void>
}

/**
 * Convert Uint8Array to hex string
 */
function toHexString(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map(b => b.toString(16).padStart(2, '0'))
    .join('')
}

/**
 * useMediaUpload hook for handling media file uploads
 */
export function useMediaUpload(options: UseMediaUploadOptions = {}): UseMediaUploadReturn {
  const {
    storageNodeUrl = DEFAULT_STORAGE_NODE_URL,
    stripExif = true,
    atomicUpload = false,
    maxFiles = MAX_FILES_PER_POST,
    onProgress,
    onUploadComplete,
    onError,
  } = options

  const [files, setFiles] = useState<MediaFile[]>([])
  const [state, setState] = useState<UploadState>('idle')
  const [error, setError] = useState<string | null>(null)
  
  // Track if upload is in progress
  const uploadingRef = useRef(false)

  /**
   * Update a single file's state
   */
  const updateFile = useCallback((fileId: string, updates: Partial<MediaFile>) => {
    setFiles(prev => prev.map(f => 
      f.id === fileId ? { ...f, ...updates } : f
    ))
  }, [])

  /**
   * Add files to upload queue
   */
  const addFiles = useCallback(async (newFiles: File[]) => {
    setError(null)
    
    // Validate files
    const { validFiles, errors, overLimitCount } = validateFiles(
      Array.from(newFiles),
      files.length
    )

    // Report file limit error
    if (overLimitCount > 0) {
      setError('error.tooManyFiles')
      onError?.('error.tooManyFiles')
    }

    // Report validation errors (use first error)
    if (errors.length > 0 && !error) {
      setError(errors[0].error)
      onError?.(errors[0].error)
    }

    // Process and add valid files
    const processedFiles: MediaFile[] = []
    
    for (const file of validFiles) {
      const id = crypto.randomUUID()
      const mediaType = detectMediaType(file.type)
      
      if (!mediaType) continue

      // Create preview URL
      const preview = URL.createObjectURL(file)

      // Get dimensions for images
      let width: number | undefined
      let height: number | undefined

      if (mediaType === 'image') {
        try {
          const bitmap = await createImageBitmap(file)
          width = bitmap.width
          height = bitmap.height
          bitmap.close()
        } catch {
          // Ignore dimension errors
        }
      }

      processedFiles.push({
        id,
        file,
        type: mediaType,
        size: file.size,
        preview,
        uploadProgress: 0,
        status: 'pending',
        width,
        height,
      })
    }

    setFiles(prev => [...prev, ...processedFiles])
  }, [files.length, error, onError])

  /**
   * Remove a file by ID
   */
  const removeFile = useCallback((fileId: string) => {
    setFiles(prev => {
      const file = prev.find(f => f.id === fileId)
      if (file?.preview) {
        URL.revokeObjectURL(file.preview)
      }
      return prev.filter(f => f.id !== fileId)
    })
  }, [])

  /**
   * Clear all files
   */
  const clearAll = useCallback(() => {
    // Revoke all preview URLs
    files.forEach(file => {
      if (file.preview) {
        URL.revokeObjectURL(file.preview)
      }
    })
    setFiles([])
    setState('idle')
    setError(null)
  }, [files])

  /**
   * Upload a single file
   */
  const uploadFile = useCallback(async (file: MediaFile): Promise<MediaUploadResult | null> => {
    const fileId = file.id

    try {
      // Update status to splitting
      updateFile(fileId, { status: 'splitting', uploadProgress: 0 })

      // Process file (strip EXIF if enabled)
      let processedFile = file.file
      let width = file.width
      let height = file.height

      if (stripExif && file.type === 'image') {
        try {
          const result = await processMediaFile(file.file, { stripExif: true })
          processedFile = result.file
          width = result.width ?? width
          height = result.height ?? height
        } catch {
          // Use original if processing fails
        }
      }

      // Read file data
      const buffer = await processedFile.arrayBuffer()
      const data = new Uint8Array(buffer)

      // Split data using hybrid scheme (threshold k, total_shards n)
      updateFile(fileId, { uploadProgress: 10 })
      const splitResult = hybrid_split(data, DEFAULT_THRESHOLD, DEFAULT_SHARD_COUNT)

      // Collect all shard chunk hashes for merkle tree
      const shardHashes: Uint8Array[] = []
      for (let i = 0; i < splitResult.shard_count; i++) {
        const shard = splitResult.get_shard(i)
        if (shard) {
          shardHashes.push(shard.chunk_hash)
        }
      }
      
      // Generate merkle tree from shard hashes
      const merkleResult = merkle_build(shardHashes)
      const merkleRoot = merkleResult.root

      // Update status to uploading
      updateFile(fileId, { status: 'uploading', uploadProgress: 20 })

      // Upload each shard to storage node
      const totalShards = splitResult.shard_count
      for (let i = 0; i < totalShards; i++) {
        const shard = splitResult.get_shard(i)
        if (!shard) continue
        
        const response = await fetch(`${storageNodeUrl}/rpc`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            jsonrpc: '2.0',
            id: 1,
            method: 'store_fragment',
            params: {
              merkle_root: toHexString(merkleRoot),
              index: shard.index,
              data: Array.from(shard.to_bytes()),
            },
          }),
        })

        if (!response.ok) {
          throw new Error(`Shard upload failed: ${response.status}`)
        }

        // Update progress
        const progress = 20 + Math.floor(((i + 1) / totalShards) * 80)
        updateFile(fileId, { uploadProgress: progress })
        onProgress?.(fileId, progress)
      }

      // Convert merkle root to hex string
      const merkleRootHex = toHexString(merkleRoot)

      // Mark as complete
      updateFile(fileId, { 
        status: 'complete', 
        uploadProgress: 100,
        merkleRoot: merkleRootHex,
      })
      onProgress?.(fileId, 100)

      return {
        fileId,
        merkleRoot: merkleRootHex,
        mediaType: file.type,
        sizeBytes: file.size,
        width,
        height,
        threshold: splitResult.threshold,
        totalShards: splitResult.total_shards,
      }
    } catch (err) {
      console.error('[useMediaUpload] Upload failed:', err)
      updateFile(fileId, { 
        status: 'error',
        error: err instanceof Error ? err.message : 'Unknown error',
      })
      return null
    }
  }, [storageNodeUrl, stripExif, updateFile, onProgress])

  /**
   * Upload all pending files
   */
  const uploadAll = useCallback(async (): Promise<MediaUploadResult[]> => {
    if (uploadingRef.current) {
      return []
    }

    const pendingFiles = files.filter(f => f.status === 'pending' || f.status === 'error')
    if (pendingFiles.length === 0) {
      return []
    }

    uploadingRef.current = true
    setState('processing')
    setError(null)

    const results: MediaUploadResult[] = []
    let hasError = false

    for (const file of pendingFiles) {
      const result = await uploadFile(file)
      
      if (result) {
        results.push(result)
      } else {
        hasError = true
        
        // In atomic mode, fail all on first error
        if (atomicUpload) {
          // Mark all pending as error
          setFiles(prev => prev.map(f => 
            f.status === 'pending' ? { ...f, status: 'error' as MediaFileStatus } : f
          ))
          break
        }
      }
    }

    uploadingRef.current = false

    if (hasError) {
      setState('error')
      setError('error.uploadFailed')
      onError?.('error.uploadFailed')
    } else {
      setState('complete')
      onUploadComplete?.(results)
    }

    return results
  }, [files, uploadFile, atomicUpload, onUploadComplete, onError])

  /**
   * Retry failed uploads
   */
  const retryFailed = useCallback(async () => {
    // Reset error status on failed files
    setFiles(prev => prev.map(f => 
      f.status === 'error' ? { ...f, status: 'pending' as MediaFileStatus, uploadProgress: 0 } : f
    ))
    
    await uploadAll()
  }, [uploadAll])

  return {
    files,
    state,
    error,
    addFiles,
    removeFile,
    clearAll,
    uploadAll,
    retryFailed,
  }
}

export default useMediaUpload

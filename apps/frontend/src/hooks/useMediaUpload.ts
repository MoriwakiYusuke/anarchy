/**
 * useMediaUpload Hook
 * 
 * T-051: Media upload hook with hybrid_split and storage node upload
 * T-065: Added video support with thumbnail extraction
 * 
 * Features:
 * - File validation
 * - EXIF stripping
 * - Video thumbnail extraction
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
import { MAX_FILES_PER_POST, detectMediaType, MAX_VIDEO_SIZE } from '@/types/media'
import { validateFile, validateFiles } from '@/lib/mediaValidator'
import { processMediaFile } from '@/lib/mediaProcessor'
import { extractVideoThumbnail } from '@/lib/videoThumbnail'
// wasm-engine の重い処理 (hybrid_split / merkle_build) はメインスレッドで叩くと
// ブラウザが「応答なし」になるので、必ず WorkerPool 経由で逃がす。
// 参照: useUpload.ts の同パターン。
import { getSharedWorkerPool } from '@/workers/WorkerPool'
// (CLAUDE.md §5) アップロードは chain-node の `storage_uploadFragment` 経由のみ。
// endpoint 解決 / auth 署名 / json.error 検査は lib/chainRpc.ts に集約。
import {
  authenticatedRpcCall,
  resolveChainRpcEndpoint,
  uint8ArrayToBase64,
  type StorageSigner,
} from '@/lib/chainRpc'
import { debugError } from '@/lib/debugLog'

// SSS/Reed-Solomon parameters
const DEFAULT_THRESHOLD = 3  // k: minimum shards to reconstruct
const DEFAULT_SHARD_COUNT = 5  // n: total shards
const MAX_RETRIES = 3
const RETRY_DELAY_MS = 1000

/** Hook options */
export interface UseMediaUploadOptions {
  /** RPC endpoint URL (default: blockchain node at 9944) */
  rpcEndpoint?: string
  /** Strip EXIF data from images */
  stripExif?: boolean
  /** All files must succeed or all fail */
  atomicUpload?: boolean
  /** Maximum files allowed */
  maxFiles?: number
  /** Allow video files (default: false for backward compatibility) */
  includeVideo?: boolean
  /** upload auth 署名用 signer (useUpload.ts と同じ。省略時は無認証 =
   *  auth disabled な dev storage-node でのみ動作) */
  signer?: StorageSigner
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
 * useMediaUpload hook for handling media file uploads
 */
export function useMediaUpload(options: UseMediaUploadOptions = {}): UseMediaUploadReturn {
  const {
    rpcEndpoint,
    stripExif = true,
    atomicUpload = false,
    maxFiles = MAX_FILES_PER_POST,
    includeVideo = false,
    signer,
    onProgress,
    onUploadComplete,
    onError,
  } = options

  const [files, setFiles] = useState<MediaFile[]>([])
  const [state, setState] = useState<UploadState>('idle')
  const [error, setError] = useState<string | null>(null)

  // Track if upload is in progress
  const uploadingRef = useRef(false)

  // SSR セーフ。worker は client only。
  const pool = typeof window !== 'undefined' ? getSharedWorkerPool() : null

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
    
    // Filter out video files if includeVideo is false
    const filteredInputFiles = includeVideo 
      ? newFiles 
      : newFiles.filter(f => !f.type.startsWith('video/'))
    
    // Report if video files were filtered out
    if (!includeVideo && filteredInputFiles.length < newFiles.length) {
      setError('error.videoNotSupported')
      onError?.('error.videoNotSupported')
      if (filteredInputFiles.length === 0) {
        return
      }
    }
    
    // Validate files
    const { validFiles, errors, overLimitCount } = validateFiles(
      Array.from(filteredInputFiles),
      files.length,
      includeVideo
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
      let duration: number | undefined
      let thumbnail: string | undefined

      if (mediaType === 'image') {
        try {
          const bitmap = await createImageBitmap(file)
          width = bitmap.width
          height = bitmap.height
          bitmap.close()
        } catch {
          // Ignore dimension errors
        }
      } else if (mediaType === 'video') {
        try {
          const videoMeta = await extractVideoThumbnail(file)
          width = videoMeta.width
          height = videoMeta.height
          duration = videoMeta.duration
          thumbnail = videoMeta.thumbnail
        } catch {
          // Ignore video metadata errors - use file directly
        }
      }

      processedFiles.push({
        id,
        file,
        type: mediaType,
        size: file.size,
        preview,
        thumbnail,
        duration,
        uploadProgress: 0,
        status: 'pending',
        width,
        height,
      })
    }

    setFiles(prev => [...prev, ...processedFiles])
  }, [files.length, error, includeVideo, onError])

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

      // Split data using hybrid scheme (threshold k, total_shards n) — Worker 経由。
      // メインスレッドで `hybrid_split` を直叩きすると数 MB のメディアで UI が
      // 数秒〜十数秒固まり、ブラウザが「応答なし」を出す原因になる。
      if (!pool) throw new Error('WorkerPool not available')
      updateFile(fileId, { uploadProgress: 10 })
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
      const splitResult = await pool.execute<HybridSplitResult>('hybrid_split', {
        data,
        k: DEFAULT_THRESHOLD,
        n: DEFAULT_SHARD_COUNT,
      })
      const { shards, threshold, totalShards } = splitResult

      // Merkle tree をシャード本体から構築 — Worker 経由。
      // chain-node の `storage_uploadFragment` は「アップロードした data が
      // merkle proof で root に対して leaf として検証できる」ことを要求する
      // ため、useUpload.ts と同じく shards 自体を leaf にする (chunk hash ではなく)。
      // proof 生成は merkle キャッシュに依存するので同一 worker を使う。
      const merkleWorkerIndex = pool.acquireWorker()
      const merkleResult = await pool.executeOnWorker<{ root: Uint8Array; rootHex: string }>(
        merkleWorkerIndex,
        'merkle_build',
        { fragments: shards },
      )
      const merkleRoot = merkleResult.root
      const merkleRootHex = merkleResult.rootHex

      // Update status to uploading
      updateFile(fileId, { status: 'uploading', uploadProgress: 20 })

      // Upload each shard via chain-node `storage_uploadFragment`
      // (CLAUDE.md §5: storage-node 直叩き禁止。useUpload.ts と同じフロー)
      const endpoints = [resolveChainRpcEndpoint(rpcEndpoint)]
      for (let i = 0; i < totalShards; i++) {
        const shardBytes = shards[i]
        if (!shardBytes) continue

        // 同じ worker で proof 生成 (merkle キャッシュが必要)
        const proof = await pool.executeOnWorker<Uint8Array>(
          merkleWorkerIndex,
          'merkle_generate_proof',
          { merkleRootHex, index: i },
        )

        const baseParams = {
          merkle_root: Array.from(merkleRoot),
          index: i,
          data: uint8ArrayToBase64(shardBytes),
          proof: uint8ArrayToBase64(proof),
          total_leaves: totalShards,
        }

        // リトライ付きアップロード。JSON-RPC error (HTTP 200) も
        // authenticatedRpcCall が検査して throw する。
        let uploaded = false
        for (let attempt = 1; attempt <= MAX_RETRIES && !uploaded; attempt++) {
          try {
            await authenticatedRpcCall<{ success: boolean; fragment_hash: number[] }>(
              'storage_uploadFragment',
              baseParams,
              { signer, endpoints },
            )
            uploaded = true
          } catch (err) {
            if (attempt === MAX_RETRIES) throw err
            await new Promise(r => setTimeout(r, RETRY_DELAY_MS * attempt))
          }
        }

        // Update progress
        const progress = 20 + Math.floor(((i + 1) / totalShards) * 80)
        updateFile(fileId, { uploadProgress: progress })
        onProgress?.(fileId, progress)
      }

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
        duration: file.duration,
        thumbnail: file.thumbnail,
        threshold,
        totalShards,
      }
    } catch (err) {
      debugError('[useMediaUpload] Upload failed:', err)
      updateFile(fileId, {
        status: 'error',
        error: err instanceof Error ? err.message : 'Unknown error',
      })
      return null
    }
  }, [rpcEndpoint, stripExif, updateFile, onProgress, pool, signer])

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

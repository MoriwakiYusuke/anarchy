/**
 * Media Types for file upload and display
 * @module types/media
 */

/**
 * Supported media types
 */
export type MediaType = 'image' | 'video'

/**
 * Media file status during upload process
 */
export type MediaFileStatus = 'pending' | 'splitting' | 'uploading' | 'complete' | 'error'

/**
 * Media file representation (client-side)
 */
export interface MediaFile {
  /** Unique identifier (crypto.randomUUID()) */
  id: string
  /** Original File object */
  file: File
  /** Detected media type */
  type: MediaType
  /** File size in bytes */
  size: number
  /** Preview URL (blob: URL for images, thumbnail for videos) */
  previewUrl?: string
  /** Upload progress percentage (0-100) */
  uploadProgress: number
  /** Current status */
  status: MediaFileStatus
  /** Error message if status is 'error' */
  error?: string
  /** Upload result if status is 'complete' */
  result?: MediaUploadResult
}

/**
 * Result of successful media upload
 */
export interface MediaUploadResult {
  /** Original file ID */
  fileId: string
  /** Merkle root of uploaded shards (hex encoded) */
  merkleRoot: string
  /** Media type */
  mediaType: MediaType
  /** Original file size in bytes */
  sizeBytes: number
  /** Image/video width (if available) */
  width?: number
  /** Image/video height (if available) */
  height?: number
  /** Reed-Solomon threshold (k) */
  threshold: number
  /** Total shards (n) */
  totalShards: number
}

/**
 * Reference to uploaded media (stored on-chain or in post metadata)
 */
export interface MediaRef {
  /** Merkle root (32 bytes, hex encoded) */
  merkleRoot: string
  /** Media type */
  type: MediaType
  /** Original size in bytes */
  size: number
  /** Width in pixels */
  width?: number
  /** Height in pixels */
  height?: number
  /** Reed-Solomon k */
  k: number
  /** Reed-Solomon n */
  n: number
}

/**
 * Upload progress event
 */
export interface UploadProgress {
  /** File ID being processed */
  fileId: string
  /** Current phase */
  phase: 'splitting' | 'uploading'
  /** Current item (shard index for uploading) */
  current: number
  /** Total items */
  total: number
  /** Percentage (0-100) */
  percent: number
}

/**
 * File validation result
 */
export interface FileValidation {
  /** Whether file is valid */
  valid: boolean
  /** i18n error key if invalid */
  error?: string
}

/**
 * Overall upload state
 */
export type UploadState = 'idle' | 'processing' | 'complete' | 'error'

// Constants

/**
 * Maximum number of media files per post
 */
export const MAX_FILES_PER_POST = 4

/**
 * Maximum image file size (100MB)
 */
export const MAX_IMAGE_SIZE = 100 * 1024 * 1024

/**
 * Maximum video file size (1GB)
 */
export const MAX_VIDEO_SIZE = 1024 * 1024 * 1024

/**
 * Allowed image MIME types
 */
export const ALLOWED_IMAGE_TYPES = [
  'image/jpeg',
  'image/png',
  'image/gif',
  'image/webp',
] as const

/**
 * Allowed video MIME types
 */
export const ALLOWED_VIDEO_TYPES = [
  'video/mp4',
  'video/webm',
  'video/quicktime',
] as const

/**
 * Detect media type from MIME type
 * @param mimeType - File MIME type
 * @returns MediaType or null if unsupported
 */
export function detectMediaType(mimeType: string): MediaType | null {
  if (ALLOWED_IMAGE_TYPES.includes(mimeType as typeof ALLOWED_IMAGE_TYPES[number])) {
    return 'image'
  }
  if (ALLOWED_VIDEO_TYPES.includes(mimeType as typeof ALLOWED_VIDEO_TYPES[number])) {
    return 'video'
  }
  return null
}

/**
 * Get maximum file size for media type
 * @param type - Media type
 * @returns Maximum size in bytes
 */
export function getMaxFileSize(type: MediaType): number {
  return type === 'image' ? MAX_IMAGE_SIZE : MAX_VIDEO_SIZE
}

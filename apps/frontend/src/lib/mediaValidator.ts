/**
 * Media Validator
 * 
 * T-049: File validation helper for media uploads
 */

import {
  MediaType,
  FileValidation,
  MAX_FILES_PER_POST,
  MAX_IMAGE_SIZE,
  MAX_VIDEO_SIZE,
  ALLOWED_IMAGE_TYPES,
  ALLOWED_VIDEO_TYPES,
  detectMediaType,
} from '@/types/media'

/**
 * Validate a single file for upload
 * 
 * @param file - File to validate
 * @returns Validation result with error key if invalid
 */
export function validateFile(file: File): FileValidation {
  // Check MIME type
  const mediaType = detectMediaType(file.type)
  if (!mediaType) {
    return {
      valid: false,
      error: 'error.unsupportedFileType',
    }
  }

  // Check file size based on type
  const maxSize = mediaType === 'image' ? MAX_IMAGE_SIZE : MAX_VIDEO_SIZE
  if (file.size > maxSize) {
    return {
      valid: false,
      error: 'error.fileTooLarge',
    }
  }

  // Check for zero-size files
  if (file.size === 0) {
    return {
      valid: false,
      error: 'error.emptyFile',
    }
  }

  return { valid: true }
}

/**
 * Validate multiple files for a single post
 * 
 * @param files - Files to validate
 * @param existingCount - Number of files already added
 * @returns Validation result with valid files and errors
 */
export function validateFiles(
  files: File[],
  existingCount: number = 0
): {
  validFiles: File[]
  errors: Array<{ file: File; error: string }>
  overLimitCount: number
} {
  const validFiles: File[] = []
  const errors: Array<{ file: File; error: string }> = []
  let overLimitCount = 0

  const remainingSlots = MAX_FILES_PER_POST - existingCount

  for (let i = 0; i < files.length; i++) {
    const file = files[i]

    // Check if we're over the file limit
    if (validFiles.length >= remainingSlots) {
      overLimitCount++
      continue
    }

    const validation = validateFile(file)
    if (validation.valid) {
      validFiles.push(file)
    } else {
      errors.push({ file, error: validation.error! })
    }
  }

  return { validFiles, errors, overLimitCount }
}

/**
 * Get human-readable file size
 */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  
  const units = ['B', 'KB', 'MB', 'GB']
  const k = 1024
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  
  return `${(bytes / Math.pow(k, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`
}

/**
 * Get accept string for file input
 */
export function getAcceptString(includeVideo: boolean = false): string {
  const types: string[] = [...ALLOWED_IMAGE_TYPES]
  if (includeVideo) {
    types.push(...ALLOWED_VIDEO_TYPES)
  }
  return types.join(',')
}

/**
 * Check if a MIME type is an image
 */
export function isImageType(mimeType: string): boolean {
  return ALLOWED_IMAGE_TYPES.includes(mimeType as typeof ALLOWED_IMAGE_TYPES[number])
}

/**
 * Check if a MIME type is a video
 */
export function isVideoType(mimeType: string): boolean {
  return ALLOWED_VIDEO_TYPES.includes(mimeType as typeof ALLOWED_VIDEO_TYPES[number])
}

/**
 * Get the media type from a file
 */
export function getMediaType(file: File): MediaType | null {
  return detectMediaType(file.type)
}

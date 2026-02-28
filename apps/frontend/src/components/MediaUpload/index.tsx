/**
 * MediaUpload Component
 * 
 * T-052: MediaUpload component for file selection and upload
 * 
 * Features:
 * - Drag and drop support
 * - File validation
 * - Preview display
 * - Progress tracking
 */

'use client'

import React, { useCallback, useRef, useState, useEffect } from 'react'
import { useLocale } from '@/i18n'
import { useMediaUpload } from '@/hooks/useMediaUpload'
import { getAcceptString } from '@/lib/mediaValidator'
import { MAX_FILES_PER_POST } from '@/types/media'
import type { MediaUploadResult, MediaFile } from '@/types/media'
import MediaPreview from './MediaPreview'
import ProgressBar from './ProgressBar'
import styles from './MediaUpload.module.css'

export { type MediaFile } from '@/types/media'

export interface MediaUploadProps {
  /** RPC endpoint URL (optional, uses default) */
  rpcEndpoint?: string
  /** Include video support */
  includeVideo?: boolean
  /** Maximum files allowed */
  maxFiles?: number
  /** Disable the component */
  disabled?: boolean
  /** Called when all uploads complete */
  onUploadComplete?: (results: MediaUploadResult[]) => void
  /** Called when file list changes (for external handling) */
  onFilesChange?: (files: MediaFile[]) => void
  /** Called on error */
  onError?: (error: string) => void
  /** Custom class name */
  className?: string
}

export default function MediaUpload({
  rpcEndpoint,
  includeVideo = false,
  maxFiles = MAX_FILES_PER_POST,
  disabled = false,
  onUploadComplete,
  onFilesChange,
  onError,
  className,
}: MediaUploadProps): React.ReactElement {
  const { t } = useLocale()
  const fileInputRef = useRef<HTMLInputElement>(null)
  const [isDragActive, setIsDragActive] = useState(false)

  const {
    files,
    state,
    error,
    addFiles,
    removeFile,
    clearAll,
    uploadAll,
    retryFailed,
  } = useMediaUpload({
    rpcEndpoint,
    includeVideo,
    onUploadComplete,
    onError,
  })

  const isUploading = state === 'processing'
  const isDisabled = disabled || isUploading
  const canAddMore = files.length < maxFiles

  // Notify parent when files change
  useEffect(() => {
    onFilesChange?.(files)
  }, [files, onFilesChange])

  // Handle file input change
  const handleFileChange = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFiles = e.target.files
    if (selectedFiles && selectedFiles.length > 0) {
      await addFiles(Array.from(selectedFiles))
    }
    // Reset input so same file can be selected again
    e.target.value = ''
  }, [addFiles])

  // Handle dropzone click
  const handleDropzoneClick = useCallback(() => {
    if (!isDisabled && canAddMore) {
      fileInputRef.current?.click()
    }
  }, [isDisabled, canAddMore])

  // Handle keyboard activation
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if ((e.key === 'Enter' || e.key === ' ') && !isDisabled && canAddMore) {
      e.preventDefault()
      fileInputRef.current?.click()
    }
  }, [isDisabled, canAddMore])

  // Drag and drop handlers
  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    if (!isDisabled) {
      setIsDragActive(true)
    }
  }, [isDisabled])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setIsDragActive(false)
  }, [])

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
  }, [])

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setIsDragActive(false)

    if (isDisabled) return

    const droppedFiles = Array.from(e.dataTransfer.files)
    if (droppedFiles.length > 0) {
      await addFiles(droppedFiles)
    }
  }, [isDisabled, addFiles])

  // Compute container classes
  const containerClasses = [
    styles.container,
    className,
  ].filter(Boolean).join(' ')

  const dropzoneClasses = [
    styles.dropzone,
    isDragActive && styles.dragActive,
    isDisabled && styles.disabled,
  ].filter(Boolean).join(' ')

  return (
    <div className={containerClasses}>
      {/* Hidden file input */}
      <input
        ref={fileInputRef}
        type="file"
        accept={getAcceptString(includeVideo)}
        multiple
        onChange={handleFileChange}
        className={styles.hiddenInput}
        disabled={isDisabled}
      />

      {/* File previews */}
      {files.length > 0 && (
        <div className={styles.previews}>
          {files.map(file => (
            <MediaPreview
              key={file.id}
              file={file}
              onRemove={() => removeFile(file.id)}
              onRetry={() => retryFailed()}
              disabled={isUploading}
            />
          ))}
        </div>
      )}

      {/* Dropzone */}
      {canAddMore && (
        <div
          role="button"
          tabIndex={isDisabled ? -1 : 0}
          className={dropzoneClasses}
          onClick={handleDropzoneClick}
          onKeyDown={handleKeyDown}
          onDragEnter={handleDragEnter}
          onDragLeave={handleDragLeave}
          onDragOver={handleDragOver}
          onDrop={handleDrop}
        >
          <span className={styles.dropzoneText}>
            {t('media.dropzone')}
          </span>
          {files.length > 0 && (
            <span className={styles.fileCount}>
              {files.length}/{maxFiles}
            </span>
          )}
        </div>
      )}

      {/* Error message */}
      {error && (
        <p className={styles.error}>{error}</p>
      )}
    </div>
  )
}

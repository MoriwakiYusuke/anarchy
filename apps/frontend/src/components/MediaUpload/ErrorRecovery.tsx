/**
 * ErrorRecovery Component
 * 
 * T-071: Error recovery UI for media upload failures
 * 
 * Features:
 * - Display failed uploads list
 * - Retry individual or all failed uploads
 * - Clear failed uploads
 */

'use client'

import React from 'react'
import type { MediaFile } from '@/types/media'
import { useLocale } from '@/i18n'
import styles from './ErrorRecovery.module.css'

export interface ErrorRecoveryProps {
  /** Failed media files */
  failedFiles: MediaFile[]
  /** Retry a single file */
  onRetryFile: (fileId: string) => void
  /** Retry all failed files */
  onRetryAll: () => void
  /** Remove a failed file */
  onRemoveFile: (fileId: string) => void
  /** Clear all failed files */
  onClearAll: () => void
  /** Whether retry is in progress */
  isRetrying?: boolean
}

export default function ErrorRecovery({
  failedFiles,
  onRetryFile,
  onRetryAll,
  onRemoveFile,
  onClearAll,
  isRetrying = false,
}: ErrorRecoveryProps): React.ReactElement | null {
  const { t } = useLocale()

  if (failedFiles.length === 0) {
    return null
  }

  return (
    <div className={styles.container} role="alert" aria-live="polite">
      <div className={styles.header}>
        <span className={styles.errorIcon}>⚠️</span>
        <h3 className={styles.title}>
          {t('media.uploadFailed')} ({failedFiles.length})
        </h3>
      </div>

      <ul className={styles.fileList}>
        {failedFiles.map(file => (
          <li key={file.id} className={styles.fileItem}>
            <span className={styles.fileName}>{file.file.name}</span>
            {file.error && (
              <span className={styles.errorMessage}>{file.error}</span>
            )}
            <div className={styles.actions}>
              <button
                type="button"
                onClick={() => onRetryFile(file.id)}
                disabled={isRetrying}
                className={styles.retryButton}
                aria-label={`Retry ${file.file.name}`}
              >
                {t('media.retry')}
              </button>
              <button
                type="button"
                onClick={() => onRemoveFile(file.id)}
                disabled={isRetrying}
                className={styles.removeButton}
                aria-label={`Remove ${file.file.name}`}
              >
                ×
              </button>
            </div>
          </li>
        ))}
      </ul>

      <div className={styles.footer}>
        <button
          type="button"
          onClick={onRetryAll}
          disabled={isRetrying}
          className={styles.retryAllButton}
        >
          {isRetrying ? t('media.retrying') : t('media.retryAll')}
        </button>
        <button
          type="button"
          onClick={onClearAll}
          disabled={isRetrying}
          className={styles.clearButton}
        >
          {t('media.clearAll')}
        </button>
      </div>
    </div>
  )
}

/**
 * MediaPreview Component
 * 
 * T-054: Preview subcomponent for media files
 */

'use client'

import React from 'react'
import type { MediaFile } from '@/types/media'
import { useLocale } from '@/i18n'
import ProgressBar from './ProgressBar'
import styles from './MediaPreview.module.css'

export interface MediaPreviewProps {
  /** Media file to preview */
  file: MediaFile
  /** Called when remove button is clicked */
  onRemove: () => void
  /** Called when retry button is clicked */
  onRetry?: () => void
  /** Disable interactions */
  disabled?: boolean
}

export default function MediaPreview({
  file,
  onRemove,
  onRetry,
  disabled = false,
}: MediaPreviewProps): React.ReactElement {
  const { t } = useLocale()
  const { status, uploadProgress, preview } = file

  const isUploading = status === 'splitting' || status === 'uploading'
  const isComplete = status === 'complete'
  const isError = status === 'error'

  return (
    <div className={`${styles.preview} ${isError ? styles.error : ''}`}>
      {/* Image preview */}
      {file.type === 'image' && preview && (
        <img 
          src={preview} 
          alt={file.file.name}
          className={styles.image}
        />
      )}

      {/* Video preview (thumbnail or placeholder) */}
      {file.type === 'video' && (
        <div className={styles.videoPlaceholder}>
          <span className={styles.videoIcon}>🎬</span>
        </div>
      )}

      {/* Status overlay */}
      <div className={styles.overlay}>
        {/* Progress */}
        {isUploading && (
          <div className={styles.progressWrapper}>
            {status === 'splitting' && (
              <span className={styles.statusText}>{t('media.processing')}</span>
            )}
            <ProgressBar progress={uploadProgress} />
          </div>
        )}

        {/* Complete checkmark */}
        {isComplete && (
          <span className={styles.completeIcon}>✓</span>
        )}

        {/* Error state */}
        {isError && (
          <div className={styles.errorOverlay}>
            <span className={styles.errorIcon}>!</span>
            {onRetry && (
              <button
                type="button"
                onClick={onRetry}
                className={styles.retryButton}
                disabled={disabled}
                aria-label={t('media.retry')}
              >
                {t('media.retry')}
              </button>
            )}
          </div>
        )}
      </div>

      {/* Remove button */}
      {!isUploading && (
        <button
          type="button"
          onClick={onRemove}
          className={styles.removeButton}
          disabled={disabled}
          aria-label={t('media.remove')}
        >
          ×
        </button>
      )}

      {/* Complete badge */}
      {isComplete && (
        <span className={styles.completeBadge}>{t('media.complete')}</span>
      )}
    </div>
  )
}

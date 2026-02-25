/**
 * MediaPlaceholder Component
 * 
 * T-072: Placeholder component for failed or loading media
 * 
 * Features:
 * - Shows error state for failed loads
 * - Shows loading state with spinner
 * - Retry button for failed loads
 */

'use client'

import React from 'react'
import { useLocale } from '@/i18n'
import styles from './MediaPlaceholder.module.css'

export type PlaceholderState = 'loading' | 'error' | 'empty'

export interface MediaPlaceholderProps {
  /** Current state */
  state: PlaceholderState
  /** Error message (for error state) */
  message?: string
  /** Retry callback (for error state) */
  onRetry?: () => void
  /** Custom class name */
  className?: string
  /** Width */
  width?: number | string
  /** Height */
  height?: number | string
}

export default function MediaPlaceholder({
  state,
  message,
  onRetry,
  className,
  width,
  height,
}: MediaPlaceholderProps): React.ReactElement {
  const { t } = useLocale()

  const containerClasses = [
    styles.container,
    styles[state],
    className,
  ].filter(Boolean).join(' ')

  const containerStyle: React.CSSProperties = {
    ...(width && { width: typeof width === 'number' ? `${width}px` : width }),
    ...(height && { height: typeof height === 'number' ? `${height}px` : height }),
  }

  return (
    <div 
      className={containerClasses}
      style={containerStyle}
      role={state === 'loading' ? 'status' : undefined}
      aria-live={state === 'loading' ? 'polite' : undefined}
    >
      {state === 'loading' && (
        <>
          <div className={styles.spinner} aria-hidden="true" />
          <span className={styles.loadingText}>{t('media.processing')}</span>
        </>
      )}

      {state === 'error' && (
        <>
          <span className={styles.errorIcon}>⚠️</span>
          <span className={styles.errorText}>
            {message || t('media.loadError')}
          </span>
          {onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className={styles.retryButton}
            >
              {t('media.retry')}
            </button>
          )}
        </>
      )}

      {state === 'empty' && (
        <span className={styles.emptyIcon}>🖼️</span>
      )}
    </div>
  )
}

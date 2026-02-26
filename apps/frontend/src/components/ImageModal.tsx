'use client'

import { useEffect, useCallback } from 'react'
import styles from './ImageModal.module.css'

interface ImageModalProps {
  src: string
  alt?: string
  onClose: () => void
  downloadFilename?: string
}

export function ImageModal({ src, alt = 'Image', onClose, downloadFilename }: ImageModalProps) {
  // Close on Escape key
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      onClose()
    }
  }, [onClose])

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown)
    // Prevent body scroll when modal is open, compensate for scrollbar width
    const scrollbarWidth = window.innerWidth - document.documentElement.clientWidth
    document.body.style.overflow = 'hidden'
    document.body.style.paddingRight = `${scrollbarWidth}px`
    return () => {
      document.removeEventListener('keydown', handleKeyDown)
      document.body.style.overflow = ''
      document.body.style.paddingRight = ''
    }
  }, [handleKeyDown])

  // Close when clicking overlay (not the image)
  const handleOverlayClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose()
    }
  }

  return (
    <div className={styles.overlay} onClick={handleOverlayClick}>
      <div className={styles.modal}>
        <button
          type="button"
          className={styles.closeButton}
          onClick={onClose}
          aria-label="閉じる"
        >
          ×
        </button>
        <img
          src={src}
          alt={alt}
          className={styles.image}
        />
        <div className={styles.actions}>
          {downloadFilename && (
            <a
              href={src}
              download={downloadFilename}
              className={styles.actionButton}
            >
              ↓ ダウンロード
            </a>
          )}
        </div>
      </div>
    </div>
  )
}

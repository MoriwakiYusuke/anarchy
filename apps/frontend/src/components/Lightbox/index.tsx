/**
 * Lightbox Component
 * 
 * T-059: Fullsize image viewer with navigation
 * 
 * Features:
 * - Fullscreen overlay
 * - Keyboard navigation (arrows, escape)
 * - Touch/swipe support
 * - Image counter
 */

'use client'

import React, { useEffect, useCallback, useRef } from 'react'
import Image from 'next/image'
import styles from './Lightbox.module.css'

export interface LightboxImage {
  /** Image source URL */
  src: string
  /** Original width */
  width?: number
  /** Original height */
  height?: number
  /** Alt text */
  alt?: string
}

export interface LightboxProps {
  /** Array of images to display */
  images: LightboxImage[]
  /** Current image index */
  currentIndex: number
  /** Called when lightbox should close */
  onClose: () => void
  /** Called when navigating to previous image */
  onPrev: () => void
  /** Called when navigating to next image */
  onNext: () => void
}

export default function Lightbox({
  images,
  currentIndex,
  onClose,
  onPrev,
  onNext,
}: LightboxProps): React.ReactElement {
  const containerRef = useRef<HTMLDivElement>(null)
  const touchStartX = useRef<number | null>(null)

  const currentImage = images[currentIndex]
  const hasMultiple = images.length > 1

  // Handle keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'Escape':
          onClose()
          break
        case 'ArrowLeft':
          if (hasMultiple) onPrev()
          break
        case 'ArrowRight':
          if (hasMultiple) onNext()
          break
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onClose, onPrev, onNext, hasMultiple])

  // Prevent body scroll when lightbox is open
  useEffect(() => {
    const originalOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    return () => {
      document.body.style.overflow = originalOverflow
    }
  }, [])

  // Focus trap
  useEffect(() => {
    containerRef.current?.focus()
  }, [])

  // Handle backdrop click
  const handleBackdropClick = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose()
    }
  }, [onClose])

  // Touch handlers for swipe
  const handleTouchStart = useCallback((e: React.TouchEvent) => {
    touchStartX.current = e.touches[0].clientX
  }, [])

  const handleTouchEnd = useCallback((e: React.TouchEvent) => {
    if (touchStartX.current === null || !hasMultiple) return

    const touchEndX = e.changedTouches[0].clientX
    const diff = touchStartX.current - touchEndX

    if (Math.abs(diff) > 50) {
      if (diff > 0) {
        onNext()
      } else {
        onPrev()
      }
    }

    touchStartX.current = null
  }, [hasMultiple, onNext, onPrev])

  return (
    <div
      ref={containerRef}
      className={styles.overlay}
      onClick={handleBackdropClick}
      onTouchStart={handleTouchStart}
      onTouchEnd={handleTouchEnd}
      role="dialog"
      aria-modal="true"
      aria-label="Image lightbox"
      tabIndex={-1}
    >
      {/* Close button */}
      <button
        type="button"
        className={styles.closeButton}
        onClick={onClose}
        aria-label="Close lightbox"
      >
        <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor">
          <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z" />
        </svg>
      </button>

      {/* Navigation buttons */}
      {hasMultiple && (
        <>
          <button
            type="button"
            className={`${styles.navButton} ${styles.prevButton}`}
            onClick={onPrev}
            aria-label="Previous image"
          >
            <svg viewBox="0 0 24 24" width="32" height="32" fill="currentColor">
              <path d="M15.41 7.41L14 6l-6 6 6 6 1.41-1.41L10.83 12z" />
            </svg>
          </button>
          <button
            type="button"
            className={`${styles.navButton} ${styles.nextButton}`}
            onClick={onNext}
            aria-label="Next image"
          >
            <svg viewBox="0 0 24 24" width="32" height="32" fill="currentColor">
              <path d="M10 6L8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z" />
            </svg>
          </button>
        </>
      )}

      {/* Image */}
      <div className={styles.imageContainer}>
        {currentImage && (
          <Image
            src={currentImage.src}
            alt={currentImage.alt || `Image ${currentIndex + 1}`}
            fill
            sizes="100vw"
            className={styles.image}
            priority
            // blob:/data: URL は next/image オプティマイザを通せないので
            // unoptimized で素のまま読ませる (DM 添付の復号 blob 用)。
            unoptimized={currentImage.src.startsWith('blob:') || currentImage.src.startsWith('data:')}
          />
        )}
      </div>

      {/* Image counter */}
      {hasMultiple && (
        <div className={styles.counter}>
          {currentIndex + 1} / {images.length}
        </div>
      )}
    </div>
  )
}

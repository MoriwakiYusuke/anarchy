/**
 * MediaDisplay Component
 * 
 * T-057: Component for displaying media attachments in timeline posts
 * 
 * Features:
 * - Fixed max-width with aspect ratio preservation
 * - Grid layout for multiple images
 * - Click to open fullsize (Lightbox)
 * - Lazy loading for performance
 */

'use client'

import React, { useState, useCallback } from 'react'
import Image from 'next/image'
import Lightbox from '@/components/Lightbox'
import styles from './MediaDisplay.module.css'

export interface MediaItem {
  /** Merkle root identifying the media in storage */
  merkleRoot: string
  /** Media type */
  type: 'image' | 'video'
  /** Width in pixels */
  width?: number
  /** Height in pixels */
  height?: number
}

export interface MediaDisplayProps {
  /** Array of media items to display */
  media: MediaItem[]
  /** Storage node URL for fetching media */
  storageNodeUrl?: string
  /** Custom class name */
  className?: string
  /** Alt text prefix */
  altPrefix?: string
}

/**
 * Get media URL from storage node
 */
function getMediaUrl(merkleRoot: string, storageNodeUrl: string): string {
  return `${storageNodeUrl}/media/${merkleRoot}`
}

/**
 * Calculate grid layout class based on media count
 */
function getGridClass(count: number): string {
  switch (count) {
    case 1:
      return styles.gridSingle
    case 2:
      return styles.gridTwo
    case 3:
      return styles.gridThree
    case 4:
    default:
      return styles.gridFour
  }
}

export default function MediaDisplay({
  media,
  storageNodeUrl = process.env.NEXT_PUBLIC_STORAGE_NODE_URL || 'http://localhost:3030',
  className,
  altPrefix = 'Post media',
}: MediaDisplayProps): React.ReactElement | null {
  const [lightboxIndex, setLightboxIndex] = useState<number | null>(null)
  const [loadError, setLoadError] = useState<Record<string, boolean>>({})

  // Filter to only images for now
  const images = media.filter(m => m.type === 'image')

  // Handle image click - open lightbox
  const handleImageClick = useCallback((index: number) => {
    setLightboxIndex(index)
  }, [])

  // Handle lightbox close
  const handleLightboxClose = useCallback(() => {
    setLightboxIndex(null)
  }, [])

  // Handle lightbox navigation
  const handleLightboxPrev = useCallback(() => {
    setLightboxIndex(prev => 
      prev !== null ? (prev > 0 ? prev - 1 : images.length - 1) : null
    )
  }, [images.length])

  const handleLightboxNext = useCallback(() => {
    setLightboxIndex(prev => 
      prev !== null ? (prev < images.length - 1 ? prev + 1 : 0) : null
    )
  }, [images.length])

  // Handle image load error
  const handleImageError = useCallback((merkleRoot: string) => {
    setLoadError(prev => ({ ...prev, [merkleRoot]: true }))
  }, [])

  // Early return if no media
  if (images.length === 0) {
    return null
  }

  const containerClasses = [
    styles.container,
    getGridClass(images.length),
    className,
  ].filter(Boolean).join(' ')

  return (
    <>
      <div className={containerClasses}>
        {images.map((item, index) => {
          const url = getMediaUrl(item.merkleRoot, storageNodeUrl)
          const hasError = loadError[item.merkleRoot]

          return (
            <button
              key={item.merkleRoot}
              type="button"
              className={styles.imageButton}
              onClick={() => handleImageClick(index)}
              aria-label={`${altPrefix} ${index + 1} of ${images.length}`}
            >
              {hasError ? (
                <div className={styles.errorPlaceholder}>
                  <span className={styles.errorIcon}>⚠️</span>
                  <span className={styles.errorText}>Failed to load</span>
                </div>
              ) : (
                <Image
                  src={url}
                  alt={`${altPrefix} ${index + 1}`}
                  fill
                  sizes="(max-width: 600px) 100vw, 400px"
                  className={styles.image}
                  loading="lazy"
                  onError={() => handleImageError(item.merkleRoot)}
                />
              )}
            </button>
          )
        })}
      </div>

      {/* Lightbox for fullsize view */}
      {lightboxIndex !== null && (
        <Lightbox
          images={images.map(m => ({
            src: getMediaUrl(m.merkleRoot, storageNodeUrl),
            width: m.width,
            height: m.height,
          }))}
          currentIndex={lightboxIndex}
          onClose={handleLightboxClose}
          onPrev={handleLightboxPrev}
          onNext={handleLightboxNext}
        />
      )}
    </>
  )
}

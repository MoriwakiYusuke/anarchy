/**
 * MediaDisplay Component
 * 
 * T-057: Component for displaying media attachments in timeline posts
 * T-069: Added video support with VideoPlayer
 * 
 * Features:
 * - Fixed max-width with aspect ratio preservation
 * - Grid layout for multiple images/videos
 * - Click to open fullsize (Lightbox for images)
 * - Inline video playback
 * - Lazy loading for performance
 */

'use client'

import React, { useState, useCallback } from 'react'
import Image from 'next/image'
import Lightbox from '@/components/Lightbox'
import VideoPlayer from '@/components/VideoPlayer'
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
  /** Duration in seconds (for video) */
  duration?: number
  /** Thumbnail URL (for video) */
  thumbnail?: string
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

  // Separate images and videos
  const images = media.filter(m => m.type === 'image')
  const videos = media.filter(m => m.type === 'video')
  
  // All displayable media
  const allMedia = media

  // Handle image click - open lightbox
  const handleImageClick = useCallback((index: number) => {
    // Only open lightbox for images
    const item = allMedia[index]
    if (item.type === 'image') {
      // Find the index within images array
      const imageIndex = images.findIndex(img => img.merkleRoot === item.merkleRoot)
      if (imageIndex !== -1) {
        setLightboxIndex(imageIndex)
      }
    }
  }, [allMedia, images])

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
  if (allMedia.length === 0) {
    return null
  }

  const containerClasses = [
    styles.container,
    getGridClass(allMedia.length),
    className,
  ].filter(Boolean).join(' ')

  return (
    <>
      <div className={containerClasses}>
        {allMedia.map((item, index) => {
          const url = getMediaUrl(item.merkleRoot, storageNodeUrl)
          const hasError = loadError[item.merkleRoot]

          // Render video
          if (item.type === 'video') {
            return (
              <div key={item.merkleRoot} className={styles.videoWrapper}>
                <VideoPlayer
                  src={url}
                  poster={item.thumbnail}
                  width={item.width}
                  height={item.height}
                  duration={item.duration}
                />
              </div>
            )
          }

          // Render image
          return (
            <button
              key={item.merkleRoot}
              type="button"
              className={styles.imageButton}
              onClick={() => handleImageClick(index)}
              aria-label={`${altPrefix} ${index + 1} of ${allMedia.length}`}
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

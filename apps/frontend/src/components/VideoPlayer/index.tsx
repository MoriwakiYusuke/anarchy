/**
 * VideoPlayer Component
 * 
 * T-068: Video player with controls overlay
 * 
 * Features:
 * - Play/pause toggle
 * - Poster display
 * - Duration overlay
 * - Keyboard controls
 */

'use client'

import React, { useRef, useState, useCallback } from 'react'
import { formatDuration } from '@/lib/videoThumbnail'
import styles from './VideoPlayer.module.css'

export interface VideoPlayerProps {
  /** Video source URL */
  src: string
  /** Poster image URL */
  poster?: string
  /** Video width */
  width?: number
  /** Video height */
  height?: number
  /** Video duration in seconds (pre-calculated) */
  duration?: number
  /** Autoplay video */
  autoPlay?: boolean
  /** Mute video */
  muted?: boolean
  /** Loop video */
  loop?: boolean
  /** Show native controls */
  controls?: boolean
  /** Additional CSS classes */
  className?: string
  /** Callback when video starts playing */
  onPlay?: () => void
  /** Callback when video pauses */
  onPause?: () => void
  /** Callback when video ends */
  onEnded?: () => void
  /** Callback on error */
  onError?: () => void
}

export default function VideoPlayer({
  src,
  poster,
  width,
  height,
  duration,
  autoPlay = false,
  muted = false,
  loop = false,
  controls = true,
  className,
  onPlay,
  onPause,
  onEnded,
  onError,
}: VideoPlayerProps): React.ReactElement {
  const videoRef = useRef<HTMLVideoElement>(null)
  const [isPlaying, setIsPlaying] = useState(false)
  const [showOverlay, setShowOverlay] = useState(!autoPlay)

  const handlePlayClick = useCallback(async () => {
    if (videoRef.current) {
      try {
        await videoRef.current.play()
        setIsPlaying(true)
        setShowOverlay(false)
      } catch (err) {
        console.error('[VideoPlayer] Play failed:', err)
      }
    }
  }, [])

  const handlePauseClick = useCallback(() => {
    if (videoRef.current) {
      videoRef.current.pause()
      setIsPlaying(false)
      setShowOverlay(true)
    }
  }, [])

  const handlePlay = useCallback(() => {
    setIsPlaying(true)
    setShowOverlay(false)
    onPlay?.()
  }, [onPlay])

  const handlePause = useCallback(() => {
    setIsPlaying(false)
    setShowOverlay(true)
    onPause?.()
  }, [onPause])

  const handleEnded = useCallback(() => {
    setIsPlaying(false)
    setShowOverlay(true)
    onEnded?.()
  }, [onEnded])

  const handleVideoClick = useCallback(() => {
    if (isPlaying) {
      handlePauseClick()
    } else {
      handlePlayClick()
    }
  }, [isPlaying, handlePlayClick, handlePauseClick])

  const handleKeyDown = useCallback((event: React.KeyboardEvent) => {
    if (event.key === ' ' || event.key === 'Enter') {
      event.preventDefault()
      handleVideoClick()
    }
  }, [handleVideoClick])

  return (
    <div 
      className={`${styles.container} ${className || ''}`}
      onClick={handleVideoClick}
      onKeyDown={handleKeyDown}
      role="button"
      tabIndex={0}
    >
      <video
        ref={videoRef}
        data-testid="video-player"
        src={src}
        poster={poster}
        width={width}
        height={height}
        autoPlay={autoPlay}
        muted={muted}
        loop={loop}
        controls={controls && isPlaying}
        playsInline
        className={styles.video}
        onPlay={handlePlay}
        onPause={handlePause}
        onEnded={handleEnded}
        onError={onError}
        // Stop propagation when native controls are shown to prevent double play/pause
        onClick={controls && isPlaying ? (e) => e.stopPropagation() : undefined}
      />

      {/* Play button overlay */}
      {showOverlay && (
        <div className={styles.overlay}>
          <button
            type="button"
            className={styles.playButton}
            onClick={(e) => {
              e.stopPropagation()
              handlePlayClick()
            }}
            aria-label="Play video"
          >
            <span className={styles.playIcon}>▶</span>
          </button>

          {/* Duration badge */}
          {duration !== undefined && (
            <span className={styles.durationBadge}>
              {formatDuration(duration)}
            </span>
          )}
        </div>
      )}
    </div>
  )
}

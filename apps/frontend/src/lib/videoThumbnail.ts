/**
 * Video Thumbnail Extraction
 * 
 * T-064: Extract thumbnails and metadata from video files
 * 
 * Features:
 * - Thumbnail extraction from video frames
 * - Duration detection
 * - Dimension extraction
 */

export interface VideoMetadata {
  /** Thumbnail as data URL */
  thumbnail: string
  /** Video width in pixels */
  width: number
  /** Video height in pixels */
  height: number
  /** Duration in seconds */
  duration: number
}

/**
 * Extract thumbnail and metadata from video file
 * 
 * @param file Video file
 * @param seekTime Time in seconds to capture thumbnail (default: 1)
 * @returns Video metadata including thumbnail
 */
export async function extractVideoThumbnail(
  file: File,
  seekTime: number = 1
): Promise<VideoMetadata> {
  return new Promise((resolve, reject) => {
    const video = document.createElement('video')
    const canvas = document.createElement('canvas')
    const ctx = canvas.getContext('2d')

    if (!ctx) {
      reject(new Error('Failed to get canvas context'))
      return
    }

    const cleanup = () => {
      video.removeEventListener('loadedmetadata', handleMetadata)
      video.removeEventListener('seeked', handleSeeked)
      video.removeEventListener('error', handleError)
      URL.revokeObjectURL(video.src)
    }

    const handleError = () => {
      cleanup()
      reject(new Error('Failed to load video'))
    }

    const handleSeeked = () => {
      try {
        // Set canvas dimensions
        canvas.width = video.videoWidth
        canvas.height = video.videoHeight

        // Draw video frame to canvas
        ctx.drawImage(video, 0, 0, canvas.width, canvas.height)

        // Convert to data URL
        const thumbnail = canvas.toDataURL('image/jpeg', 0.7)

        cleanup()
        resolve({
          thumbnail,
          width: video.videoWidth,
          height: video.videoHeight,
          duration: video.duration,
        })
      } catch (err) {
        cleanup()
        reject(err)
      }
    }

    const handleMetadata = () => {
      // Seek to specified time (or 10% if seekTime is 0)
      const targetTime = seekTime > 0 ? Math.min(seekTime, video.duration) : video.duration * 0.1
      video.currentTime = targetTime
    }

    video.addEventListener('loadedmetadata', handleMetadata)
    video.addEventListener('seeked', handleSeeked)
    video.addEventListener('error', handleError)

    // Enable CORS for blob URLs
    video.crossOrigin = 'anonymous'
    video.preload = 'metadata'
    video.muted = true

    // Create object URL and load video
    video.src = URL.createObjectURL(file)
    video.load()
  })
}

/**
 * Get video duration without extracting thumbnail
 * 
 * @param file Video file
 * @returns Duration in seconds
 */
export async function getVideoDuration(file: File): Promise<number> {
  return new Promise((resolve, reject) => {
    const video = document.createElement('video')

    const cleanup = () => {
      video.removeEventListener('loadedmetadata', handleMetadata)
      video.removeEventListener('error', handleError)
      URL.revokeObjectURL(video.src)
    }

    const handleError = () => {
      cleanup()
      reject(new Error('Failed to load video'))
    }

    const handleMetadata = () => {
      const duration = video.duration
      cleanup()
      resolve(duration)
    }

    video.addEventListener('loadedmetadata', handleMetadata)
    video.addEventListener('error', handleError)

    video.preload = 'metadata'
    video.muted = true
    video.src = URL.createObjectURL(file)
    video.load()
  })
}

/**
 * Format duration as MM:SS or HH:MM:SS
 */
export function formatDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const secs = Math.floor(seconds % 60)

  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`
  }
  return `${minutes}:${secs.toString().padStart(2, '0')}`
}

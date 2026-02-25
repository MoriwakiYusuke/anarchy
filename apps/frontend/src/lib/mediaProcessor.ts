/**
 * Media Processor
 * 
 * T-048: EXIF stripping and image processing helper
 * Strips metadata from images before upload for privacy
 */

/**
 * Strip EXIF metadata from a JPEG image
 * 
 * JPEG structure:
 * - FFD8 (SOI - Start of Image)
 * - FFE0/FFE1... (APP markers containing EXIF)
 * - Other segments
 * - FFD9 (EOI - End of Image)
 * 
 * We remove APP1 (FFE1) which contains EXIF data
 */
export function stripExifFromJpeg(data: Uint8Array): Uint8Array {
  // Check for JPEG magic bytes
  if (data[0] !== 0xFF || data[1] !== 0xD8) {
    // Not a JPEG, return as-is
    return data
  }

  const result: number[] = []
  let i = 0

  // Copy SOI marker
  result.push(data[i++])
  result.push(data[i++])

  while (i < data.length) {
    // Check for marker
    if (data[i] !== 0xFF) {
      // Not a marker, copy and continue
      result.push(data[i++])
      continue
    }

    const marker = data[i + 1]

    // EOI (End of Image) - copy rest and finish
    if (marker === 0xD9) {
      while (i < data.length) {
        result.push(data[i++])
      }
      break
    }

    // SOS (Start of Scan) - copy rest (image data follows)
    if (marker === 0xDA) {
      while (i < data.length) {
        result.push(data[i++])
      }
      break
    }

    // APP1 (EXIF) - skip this segment
    if (marker === 0xE1) {
      const length = (data[i + 2] << 8) | data[i + 3]
      i += 2 + length // Skip marker + length
      continue
    }

    // Other segments - copy them
    if (marker >= 0xE0 && marker <= 0xEF) {
      // APP markers (except APP1 which we skip)
      const length = (data[i + 2] << 8) | data[i + 3]
      if (marker === 0xE0) {
        // Keep APP0 (JFIF)
        for (let j = 0; j < 2 + length; j++) {
          result.push(data[i + j])
        }
      }
      i += 2 + length
      continue
    }

    // Copy other markers with their data
    if ((marker >= 0xC0 && marker <= 0xCF) || marker === 0xDB || marker === 0xC4 || marker === 0xFE) {
      // SOF, DQT, DHT, COM - copy with length
      result.push(data[i++])
      result.push(data[i++])
      if (i + 1 < data.length) {
        const length = (data[i] << 8) | data[i + 1]
        for (let j = 0; j < length; j++) {
          if (i < data.length) {
            result.push(data[i++])
          }
        }
      }
      continue
    }

    // Unknown marker - copy byte by byte
    result.push(data[i++])
  }

  return new Uint8Array(result)
}

/**
 * Strip metadata from an image file
 * 
 * @param file - Image file to process
 * @returns Processed file without EXIF data
 */
export async function stripImageMetadata(file: File): Promise<File> {
  // Only process JPEG files
  if (file.type !== 'image/jpeg' && file.type !== 'image/jpg') {
    // For PNG/GIF/WebP, use canvas re-encoding which strips metadata
    return await reencodeImage(file)
  }

  const buffer = await file.arrayBuffer()
  const data = new Uint8Array(buffer)
  const stripped = stripExifFromJpeg(data)

  // Create a new Uint8Array from the stripped data to ensure it's ArrayBuffer-backed
  const cleanData = new Uint8Array(stripped)
  return new File([cleanData], file.name, { type: file.type })
}

/**
 * Re-encode image through canvas to strip all metadata
 * Works for PNG, GIF, WebP
 */
async function reencodeImage(file: File): Promise<File> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    const url = URL.createObjectURL(file)

    img.onload = () => {
      URL.revokeObjectURL(url)

      const canvas = document.createElement('canvas')
      canvas.width = img.width
      canvas.height = img.height

      const ctx = canvas.getContext('2d')
      if (!ctx) {
        reject(new Error('Failed to get canvas context'))
        return
      }

      ctx.drawImage(img, 0, 0)

      canvas.toBlob(
        (blob) => {
          if (!blob) {
            reject(new Error('Failed to create blob'))
            return
          }
          resolve(new File([blob], file.name, { type: file.type }))
        },
        file.type,
        0.95 // Quality for lossy formats
      )
    }

    img.onerror = () => {
      URL.revokeObjectURL(url)
      reject(new Error('Failed to load image'))
    }

    img.src = url
  })
}

/**
 * Get image dimensions
 */
export async function getImageDimensions(file: File): Promise<{ width: number; height: number }> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    const url = URL.createObjectURL(file)

    img.onload = () => {
      URL.revokeObjectURL(url)
      resolve({ width: img.width, height: img.height })
    }

    img.onerror = () => {
      URL.revokeObjectURL(url)
      reject(new Error('Failed to load image'))
    }

    img.src = url
  })
}

/**
 * Process media file before upload
 * - Strip EXIF data from images
 * - Get dimensions
 */
export async function processMediaFile(
  file: File,
  options: { stripExif?: boolean } = {}
): Promise<{ file: File; width?: number; height?: number }> {
  const { stripExif = true } = options

  let processedFile = file
  let width: number | undefined
  let height: number | undefined

  // Process images
  if (file.type.startsWith('image/')) {
    // Get dimensions first
    try {
      const dims = await getImageDimensions(file)
      width = dims.width
      height = dims.height
    } catch {
      // Ignore dimension errors
    }

    // Strip EXIF if requested
    if (stripExif) {
      try {
        processedFile = await stripImageMetadata(file)
      } catch {
        // If stripping fails, use original file
        processedFile = file
      }
    }
  }

  return { file: processedFile, width, height }
}

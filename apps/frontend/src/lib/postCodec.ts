/**
 * Post Content Binary Codec
 * 
 * Format:
 * [1 byte: version=1]
 * [4 bytes: text_length (big-endian)]
 * [text bytes (UTF-8)]
 * [1 byte: media_count]
 * for each media:
 *   [1 byte: media_type]  // 0=jpeg, 1=png, 2=gif, 3=webp
 *   [2 bytes: width (big-endian)]
 *   [2 bytes: height (big-endian)]
 *   [4 bytes: data_length (big-endian)]
 *   [data bytes]
 */

export const CODEC_VERSION = 1;

export type MediaType = 'image/jpeg' | 'image/png' | 'image/gif' | 'image/webp' | 'video/mp4' | 'video/webm' | 'video/quicktime';

export interface MediaItem {
  type: MediaType;
  width: number;
  height: number;
  data: Uint8Array;
}

export interface PostContent {
  text: string;
  media: MediaItem[];
}

const MEDIA_TYPE_MAP: Record<MediaType, number> = {
  'image/jpeg': 0,
  'image/png': 1,
  'image/gif': 2,
  'image/webp': 3,
  'video/mp4': 4,
  'video/webm': 5,
  'video/quicktime': 6,
};

const MEDIA_TYPE_REVERSE: Record<number, MediaType> = {
  0: 'image/jpeg',
  1: 'image/png',
  2: 'image/gif',
  3: 'image/webp',
  4: 'video/mp4',
  5: 'video/webm',
  6: 'video/quicktime',
};

/**
 * Encode post content (text + media) to binary format
 */
export function encodePostContent(content: PostContent): Uint8Array {
  const textBytes = new TextEncoder().encode(content.text);
  
  // Calculate total size
  let totalSize = 1 + 4 + textBytes.length + 1; // version + text_length + text + media_count
  for (const media of content.media) {
    totalSize += 1 + 2 + 2 + 4 + media.data.length; // type + width + height + data_length + data
  }
  
  const buffer = new Uint8Array(totalSize);
  const view = new DataView(buffer.buffer);
  let offset = 0;
  
  // Version
  buffer[offset++] = CODEC_VERSION;
  
  // Text length (big-endian)
  view.setUint32(offset, textBytes.length, false);
  offset += 4;
  
  // Text bytes
  buffer.set(textBytes, offset);
  offset += textBytes.length;
  
  // Media count
  buffer[offset++] = content.media.length;
  
  // Media items
  for (const media of content.media) {
    // Media type
    buffer[offset++] = MEDIA_TYPE_MAP[media.type] ?? 0;
    
    // Width (big-endian)
    view.setUint16(offset, media.width, false);
    offset += 2;
    
    // Height (big-endian)
    view.setUint16(offset, media.height, false);
    offset += 2;
    
    // Data length (big-endian)
    view.setUint32(offset, media.data.length, false);
    offset += 4;
    
    // Data bytes
    buffer.set(media.data, offset);
    offset += media.data.length;
  }
  
  return buffer;
}

/**
 * Decode binary format to post content
 */
export function decodePostContent(data: Uint8Array): PostContent {
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  let offset = 0;
  
  // Version check
  const version = data[offset++];
  if (version !== CODEC_VERSION) {
    // Fallback: treat as plain text (legacy posts)
    return {
      text: new TextDecoder().decode(data),
      media: [],
    };
  }
  
  // Text length
  const textLength = view.getUint32(offset, false);
  offset += 4;
  
  // Text bytes
  const textBytes = data.slice(offset, offset + textLength);
  const text = new TextDecoder().decode(textBytes);
  offset += textLength;
  
  // Media count
  const mediaCount = data[offset++];
  
  // Media items
  const media: MediaItem[] = [];
  for (let i = 0; i < mediaCount; i++) {
    // Media type
    const typeCode = data[offset++];
    const type = MEDIA_TYPE_REVERSE[typeCode] ?? 'image/jpeg';
    
    // Width
    const width = view.getUint16(offset, false);
    offset += 2;
    
    // Height
    const height = view.getUint16(offset, false);
    offset += 2;
    
    // Data length
    const dataLength = view.getUint32(offset, false);
    offset += 4;
    
    // Data bytes
    const mediaData = data.slice(offset, offset + dataLength);
    offset += dataLength;
    
    media.push({ type, width, height, data: mediaData });
  }
  
  return { text, media };
}

/**
 * Convert Uint8Array to base64 string (handles large files without stack overflow)
 * Uses chunked approach with incremental btoa calls
 */
export function uint8ArrayToBase64(data: Uint8Array): string {
  // For small data, use simple conversion
  if (data.length < 0x8000) {
    let binary = '';
    for (let i = 0; i < data.length; i++) {
      binary += String.fromCharCode(data[i]);
    }
    return btoa(binary);
  }
  
  // For large data, process in chunks that are multiples of 3
  // (base64 encodes 3 bytes into 4 characters)
  const CHUNK_SIZE = 0x6000; // 24KB (divisible by 3)
  const base64Chunks: string[] = [];
  
  for (let i = 0; i < data.length; i += CHUNK_SIZE) {
    const end = Math.min(i + CHUNK_SIZE, data.length);
    let binary = '';
    for (let j = i; j < end; j++) {
      binary += String.fromCharCode(data[j]);
    }
    base64Chunks.push(btoa(binary));
  }
  
  return base64Chunks.join('');
}

/**
 * Convert MediaItem to data URL for display (async for large files)
 */
export function mediaToDataUrl(media: MediaItem): string {
  const base64 = uint8ArrayToBase64(media.data);
  return `data:${media.type};base64,${base64}`;
}

/**
 * Load image file and get dimensions
 */
export async function loadImageFile(file: File): Promise<MediaItem> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const arrayBuffer = reader.result as ArrayBuffer;
      const data = new Uint8Array(arrayBuffer);
      
      // Get image dimensions
      const img = new Image();
      img.onload = () => {
        resolve({
          type: file.type as MediaType,
          width: img.width,
          height: img.height,
          data,
        });
        URL.revokeObjectURL(img.src);
      };
      img.onerror = () => {
        reject(new Error('Failed to load image'));
        URL.revokeObjectURL(img.src);
      };
      img.src = URL.createObjectURL(file);
    };
    reader.onerror = () => reject(new Error('Failed to read file'));
    reader.readAsArrayBuffer(file);
  });
}

/**
 * Load video file and get dimensions
 */
export async function loadVideoFile(file: File): Promise<MediaItem> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const arrayBuffer = reader.result as ArrayBuffer;
      const data = new Uint8Array(arrayBuffer);
      
      // Get video dimensions
      const video = document.createElement('video');
      video.preload = 'metadata';
      video.onloadedmetadata = () => {
        resolve({
          type: file.type as MediaType,
          width: video.videoWidth,
          height: video.videoHeight,
          data,
        });
        URL.revokeObjectURL(video.src);
      };
      video.onerror = () => {
        reject(new Error('Failed to load video'));
        URL.revokeObjectURL(video.src);
      };
      video.src = URL.createObjectURL(file);
    };
    reader.onerror = () => reject(new Error('Failed to read file'));
    reader.readAsArrayBuffer(file);
  });
}

/**
 * Load media file (image or video) and get dimensions
 */
export async function loadMediaFile(file: File): Promise<MediaItem> {
  if (file.type.startsWith('video/')) {
    return loadVideoFile(file);
  }
  return loadImageFile(file);
}

/**
 * Check if file type is supported
 */
export function isSupportedMediaType(type: string): type is MediaType {
  return type in MEDIA_TYPE_MAP;
}

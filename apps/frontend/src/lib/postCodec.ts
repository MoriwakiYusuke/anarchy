/**
 * Post Content Binary Codec
 * 
 * Format v2:
 * [1 byte: version=2]
 * [4 bytes: text_length (big-endian)]
 * [text bytes (UTF-8)]
 * [1 byte: media_count]
 * for each media:
 *   [1 byte: mime_type_length]
 *   [mime_type bytes (UTF-8)]
 *   [2 bytes: filename_length (big-endian)]
 *   [filename bytes (UTF-8)]
 *   [2 bytes: width (big-endian)]
 *   [2 bytes: height (big-endian)]
 *   [4 bytes: data_length (big-endian)]
 *   [data bytes]
 * 
 * Format v1 (legacy):
 * [1 byte: version=1]
 * [4 bytes: text_length]
 * [text bytes]
 * [1 byte: media_count]
 * for each media:
 *   [1 byte: media_type_code]
 *   [2 bytes: width]
 *   [2 bytes: height]
 *   [4 bytes: data_length]
 *   [data bytes]
 */

export const CODEC_VERSION = 2;
export const CODEC_VERSION_LEGACY = 1;

// Any MIME type is supported
export type MediaType = string;

export interface MediaItem {
  type: MediaType;
  filename: string;
  width: number;
  height: number;
  data: Uint8Array;
}

export interface PostContent {
  text: string;
  media: MediaItem[];
}

// Legacy type codes for v1 compatibility
const LEGACY_MEDIA_TYPE_REVERSE: Record<number, MediaType> = {
  0: 'image/jpeg',
  1: 'image/png',
  2: 'image/gif',
  3: 'image/webp',
  4: 'video/mp4',
  5: 'video/webm',
  6: 'video/quicktime',
};

/**
 * Encode post content (text + media) to binary format v2
 */
export function encodePostContent(content: PostContent): Uint8Array {
  const textEncoder = new TextEncoder();
  const textBytes = textEncoder.encode(content.text);
  
  // Pre-encode media strings
  const mediaEncodings = content.media.map(media => ({
    typeBytes: textEncoder.encode(media.type),
    filenameBytes: textEncoder.encode(media.filename || ''),
    media,
  }));
  
  // Calculate total size for v2 format
  let totalSize = 1 + 4 + textBytes.length + 1; // version + text_length + text + media_count
  for (const { typeBytes, filenameBytes, media } of mediaEncodings) {
    // type_length(1) + type + filename_length(2) + filename + width(2) + height(2) + data_length(4) + data
    totalSize += 1 + typeBytes.length + 2 + filenameBytes.length + 2 + 2 + 4 + media.data.length;
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
  for (const { typeBytes, filenameBytes, media } of mediaEncodings) {
    // MIME type length (1 byte) and bytes
    buffer[offset++] = typeBytes.length;
    buffer.set(typeBytes, offset);
    offset += typeBytes.length;
    
    // Filename length (2 bytes big-endian) and bytes
    view.setUint16(offset, filenameBytes.length, false);
    offset += 2;
    buffer.set(filenameBytes, offset);
    offset += filenameBytes.length;
    
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
 * Decode binary format to post content (supports v1 and v2)
 */
export function decodePostContent(data: Uint8Array): PostContent {
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const textDecoder = new TextDecoder();
  let offset = 0;
  
  // Version check
  const version = data[offset++];
  
  if (version === CODEC_VERSION_LEGACY) {
    // Decode v1 format
    return decodeV1(data, view, offset, textDecoder);
  }
  
  if (version !== CODEC_VERSION) {
    // Fallback: treat as plain text (very old posts)
    return {
      text: textDecoder.decode(data),
      media: [],
    };
  }
  
  // Decode v2 format
  // Text length
  const textLength = view.getUint32(offset, false);
  offset += 4;
  
  // Text bytes
  const textBytes = data.slice(offset, offset + textLength);
  const text = textDecoder.decode(textBytes);
  offset += textLength;
  
  // Media count
  const mediaCount = data[offset++];
  
  // Media items
  const media: MediaItem[] = [];
  for (let i = 0; i < mediaCount; i++) {
    // MIME type length and bytes
    const typeLength = data[offset++];
    const typeBytes = data.slice(offset, offset + typeLength);
    const type = textDecoder.decode(typeBytes);
    offset += typeLength;
    
    // Filename length and bytes
    const filenameLength = view.getUint16(offset, false);
    offset += 2;
    const filenameBytes = data.slice(offset, offset + filenameLength);
    const filename = textDecoder.decode(filenameBytes);
    offset += filenameLength;
    
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
    
    media.push({ type, filename, width, height, data: mediaData });
  }
  
  return { text, media };
}

/**
 * Decode legacy v1 format
 */
function decodeV1(data: Uint8Array, view: DataView, offset: number, textDecoder: TextDecoder): PostContent {
  // Text length
  const textLength = view.getUint32(offset, false);
  offset += 4;
  
  // Text bytes
  const textBytes = data.slice(offset, offset + textLength);
  const text = textDecoder.decode(textBytes);
  offset += textLength;
  
  // Media count
  const mediaCount = data[offset++];
  
  // Media items
  const media: MediaItem[] = [];
  for (let i = 0; i < mediaCount; i++) {
    // Media type code
    const typeCode = data[offset++];
    const type = LEGACY_MEDIA_TYPE_REVERSE[typeCode] ?? 'application/octet-stream';
    
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
    
    media.push({ type, filename: '', width, height, data: mediaData });
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
          filename: file.name,
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
          filename: file.name,
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
 * Load general file (no dimensions)
 */
export async function loadGeneralFile(file: File): Promise<MediaItem> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const arrayBuffer = reader.result as ArrayBuffer;
      const data = new Uint8Array(arrayBuffer);
      resolve({
        type: file.type || 'application/octet-stream',
        filename: file.name,
        width: 0,
        height: 0,
        data,
      });
    };
    reader.onerror = () => reject(new Error('Failed to read file'));
    reader.readAsArrayBuffer(file);
  });
}

/**
 * Load media file (image, video, or general file)
 */
export async function loadMediaFile(file: File): Promise<MediaItem> {
  if (file.type.startsWith('video/')) {
    return loadVideoFile(file);
  }
  if (file.type.startsWith('image/')) {
    return loadImageFile(file);
  }
  return loadGeneralFile(file);
}

/**
 * Check if file type is supported (all types are supported now)
 */
export function isSupportedMediaType(type: string): type is MediaType {
  return true;
}

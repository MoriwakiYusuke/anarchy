/**
 * Post Content Binary Codec Tests
 */

import {
  encodePostContent,
  decodePostContent,
  mediaToDataUrl,
  isSupportedMediaType,
  CODEC_VERSION,
  type MediaItem,
  type PostContent,
} from '@/lib/postCodec'

describe('PostCodec', () => {
  describe('isSupportedMediaType', () => {
    it('should return true for image/jpeg', () => {
      expect(isSupportedMediaType('image/jpeg')).toBe(true)
    })

    it('should return true for image/png', () => {
      expect(isSupportedMediaType('image/png')).toBe(true)
    })

    it('should return true for image/gif', () => {
      expect(isSupportedMediaType('image/gif')).toBe(true)
    })

    it('should return true for image/webp', () => {
      expect(isSupportedMediaType('image/webp')).toBe(true)
    })

    it('should return true for video/mp4', () => {
      expect(isSupportedMediaType('video/mp4')).toBe(true)
    })

    it('should return true for video/webm', () => {
      expect(isSupportedMediaType('video/webm')).toBe(true)
    })

    it('should return true for video/quicktime', () => {
      expect(isSupportedMediaType('video/quicktime')).toBe(true)
    })

    it('should return true for any type (all types supported)', () => {
      expect(isSupportedMediaType('text/plain')).toBe(true)
      expect(isSupportedMediaType('application/pdf')).toBe(true)
      expect(isSupportedMediaType('audio/mp3')).toBe(true)
      expect(isSupportedMediaType('video/avi')).toBe(true)
    })
  })

  describe('encodePostContent', () => {
    it('should encode text-only post', () => {
      const content: PostContent = {
        text: 'Hello, world!',
        media: [],
      }

      const encoded = encodePostContent(content)
      expect(encoded).toBeInstanceOf(Uint8Array)
      expect(encoded[0]).toBe(CODEC_VERSION)
    })

    it('should encode post with image', () => {
      const imageData = new Uint8Array([0xFF, 0xD8, 0xFF, 0xE0])
      const content: PostContent = {
        text: 'Test',
        media: [
          {
            type: 'image/jpeg',
            filename: 'test.jpg',
            width: 100,
            height: 200,
            data: imageData,
          },
        ],
      }

      const encoded = encodePostContent(content)
      expect(encoded).toBeInstanceOf(Uint8Array)
      expect(encoded[0]).toBe(CODEC_VERSION)
    })

    it('should encode post with video', () => {
      const videoData = new Uint8Array([0x00, 0x00, 0x00, 0x1C])
      const content: PostContent = {
        text: 'Video post',
        media: [
          {
            type: 'video/mp4',
            filename: 'video.mp4',
            width: 1920,
            height: 1080,
            data: videoData,
          },
        ],
      }

      const encoded = encodePostContent(content)
      expect(encoded).toBeInstanceOf(Uint8Array)
      expect(encoded[0]).toBe(CODEC_VERSION)
    })

    it('should encode post with multiple media', () => {
      const imageData = new Uint8Array([0xFF, 0xD8])
      const videoData = new Uint8Array([0x00, 0x00])
      const content: PostContent = {
        text: 'Mixed media',
        media: [
          { type: 'image/jpeg', filename: 'img.jpg', width: 100, height: 100, data: imageData },
          { type: 'video/mp4', filename: 'vid.mp4', width: 1920, height: 1080, data: videoData },
        ],
      }

      const encoded = encodePostContent(content)
      expect(encoded).toBeInstanceOf(Uint8Array)
    })
  })

  describe('decodePostContent', () => {
    it('should decode text-only post', () => {
      const original: PostContent = {
        text: 'Hello, world!',
        media: [],
      }

      const encoded = encodePostContent(original)
      const decoded = decodePostContent(encoded)

      expect(decoded.text).toBe(original.text)
      expect(decoded.media).toHaveLength(0)
    })

    it('should decode post with image', () => {
      const imageData = new Uint8Array([0xFF, 0xD8, 0xFF, 0xE0])
      const original: PostContent = {
        text: 'Test post',
        media: [
          {
            type: 'image/jpeg',
            filename: 'test.jpg',
            width: 100,
            height: 200,
            data: imageData,
          },
        ],
      }

      const encoded = encodePostContent(original)
      const decoded = decodePostContent(encoded)

      expect(decoded.text).toBe(original.text)
      expect(decoded.media).toHaveLength(1)
      expect(decoded.media[0].type).toBe('image/jpeg')
      expect(decoded.media[0].filename).toBe('test.jpg')
      expect(decoded.media[0].width).toBe(100)
      expect(decoded.media[0].height).toBe(200)
      expect(decoded.media[0].data).toEqual(imageData)
    })

    it('should decode post with video', () => {
      const videoData = new Uint8Array([0x00, 0x00, 0x00, 0x1C, 0x66, 0x74, 0x79, 0x70])
      const original: PostContent = {
        text: 'Video post!',
        media: [
          {
            type: 'video/mp4',
            filename: 'video.mp4',
            width: 1920,
            height: 1080,
            data: videoData,
          },
        ],
      }

      const encoded = encodePostContent(original)
      const decoded = decodePostContent(encoded)

      expect(decoded.text).toBe(original.text)
      expect(decoded.media).toHaveLength(1)
      expect(decoded.media[0].type).toBe('video/mp4')
      expect(decoded.media[0].filename).toBe('video.mp4')
      expect(decoded.media[0].width).toBe(1920)
      expect(decoded.media[0].height).toBe(1080)
      expect(decoded.media[0].data).toEqual(videoData)
    })

    it('should decode post with webm video', () => {
      const videoData = new Uint8Array([0x1A, 0x45, 0xDF, 0xA3])
      const original: PostContent = {
        text: 'WebM video',
        media: [
          {
            type: 'video/webm',
            filename: 'clip.webm',
            width: 1280,
            height: 720,
            data: videoData,
          },
        ],
      }

      const encoded = encodePostContent(original)
      const decoded = decodePostContent(encoded)

      expect(decoded.media[0].type).toBe('video/webm')
      expect(decoded.media[0].width).toBe(1280)
      expect(decoded.media[0].height).toBe(720)
    })

    it('should decode post with quicktime video', () => {
      const videoData = new Uint8Array([0x00, 0x00, 0x00, 0x14])
      const original: PostContent = {
        text: 'MOV video',
        media: [
          {
            type: 'video/quicktime',
            filename: 'movie.mov',
            width: 640,
            height: 480,
            data: videoData,
          },
        ],
      }

      const encoded = encodePostContent(original)
      const decoded = decodePostContent(encoded)

      expect(decoded.media[0].type).toBe('video/quicktime')
    })

    it('should decode post with mixed media', () => {
      const imageData = new Uint8Array([0xFF, 0xD8])
      const videoData = new Uint8Array([0x00, 0x00])
      const original: PostContent = {
        text: 'Mixed',
        media: [
          { type: 'image/png', filename: 'img.png', width: 50, height: 50, data: imageData },
          { type: 'video/mp4', filename: 'vid.mp4', width: 1920, height: 1080, data: videoData },
        ],
      }

      const encoded = encodePostContent(original)
      const decoded = decodePostContent(encoded)

      expect(decoded.media).toHaveLength(2)
      expect(decoded.media[0].type).toBe('image/png')
      expect(decoded.media[1].type).toBe('video/mp4')
    })

    it('should handle Japanese text', () => {
      const original: PostContent = {
        text: 'これは日本語のテストです。🎉',
        media: [],
      }

      const encoded = encodePostContent(original)
      const decoded = decodePostContent(encoded)

      expect(decoded.text).toBe(original.text)
    })

    it('should handle empty text', () => {
      const imageData = new Uint8Array([0xFF, 0xD8])
      const original: PostContent = {
        text: '',
        media: [{ type: 'image/jpeg', filename: 'img.jpg', width: 100, height: 100, data: imageData }],
      }

      const encoded = encodePostContent(original)
      const decoded = decodePostContent(encoded)

      expect(decoded.text).toBe('')
      expect(decoded.media).toHaveLength(1)
    })
  })

  describe('mediaToDataUrl', () => {
    it('should convert image media to data URL', () => {
      const media: MediaItem = {
        type: 'image/jpeg',
        filename: 'test.jpg',
        width: 100,
        height: 100,
        data: new Uint8Array([0xFF, 0xD8, 0xFF, 0xE0]),
      }

      const dataUrl = mediaToDataUrl(media)
      expect(dataUrl).toMatch(/^data:image\/jpeg;base64,/)
    })

    it('should convert video media to data URL', () => {
      const media: MediaItem = {
        type: 'video/mp4',
        filename: 'video.mp4',
        width: 1920,
        height: 1080,
        data: new Uint8Array([0x00, 0x00, 0x00, 0x1C]),
      }

      const dataUrl = mediaToDataUrl(media)
      expect(dataUrl).toMatch(/^data:video\/mp4;base64,/)
    })

    it('should convert webm video to data URL', () => {
      const media: MediaItem = {
        type: 'video/webm',
        filename: 'clip.webm',
        width: 1280,
        height: 720,
        data: new Uint8Array([0x1A, 0x45, 0xDF, 0xA3]),
      }

      const dataUrl = mediaToDataUrl(media)
      expect(dataUrl).toMatch(/^data:video\/webm;base64,/)
    })

    it('should handle empty data', () => {
      const media: MediaItem = {
        type: 'image/png',
        filename: '',
        width: 1,
        height: 1,
        data: new Uint8Array([]),
      }

      const dataUrl = mediaToDataUrl(media)
      expect(dataUrl).toBe('data:image/png;base64,')
    })

    it('should handle data at exact chunk boundary (64KB)', () => {
      const CHUNK_SIZE = 0x10000 // 64KB
      const data = new Uint8Array(CHUNK_SIZE)
      for (let i = 0; i < data.length; i++) {
        data[i] = i % 256
      }

      const media: MediaItem = {
        type: 'image/jpeg',
        filename: 'chunk-test.jpg',
        width: 256,
        height: 256,
        data,
      }

      const dataUrl = mediaToDataUrl(media)
      expect(dataUrl).toMatch(/^data:image\/jpeg;base64,/)
      expect(dataUrl.length).toBeGreaterThan(50000)
    })

    it('should handle 1MB data without stack overflow', () => {
      const SIZE_1MB = 1024 * 1024 // 1MB
      const data = new Uint8Array(SIZE_1MB)
      for (let i = 0; i < data.length; i++) {
        data[i] = i % 256
      }

      const media: MediaItem = {
        type: 'video/mp4',
        filename: 'large-1mb.mp4',
        width: 1920,
        height: 1080,
        data,
      }

      const dataUrl = mediaToDataUrl(media)
      expect(dataUrl).toMatch(/^data:video\/mp4;base64,/)
      // Base64 encoded 1MB should be ~1.37MB
      expect(dataUrl.length).toBeGreaterThan(1000000)
    })

    it('should handle 5MB data without stack overflow', () => {
      const SIZE_5MB = 5 * 1024 * 1024 // 5MB
      const data = new Uint8Array(SIZE_5MB)
      // Fill with repeating pattern
      for (let i = 0; i < data.length; i++) {
        data[i] = i % 256
      }

      const media: MediaItem = {
        type: 'video/mp4',
        filename: 'large-5mb.mp4',
        width: 1920,
        height: 1080,
        data,
      }

      const dataUrl = mediaToDataUrl(media)
      expect(dataUrl).toMatch(/^data:video\/mp4;base64,/)
      // Base64 encoded 5MB should be ~6.85MB
      expect(dataUrl.length).toBeGreaterThan(5000000)
    })

    it('should produce correct base64 output that can be decoded', () => {
      // Known test data
      const testData = new Uint8Array([72, 101, 108, 108, 111]) // "Hello"
      const media: MediaItem = {
        type: 'image/png',
        filename: 'hello.png',
        width: 1,
        height: 1,
        data: testData,
      }

      const dataUrl = mediaToDataUrl(media)
      // Extract base64 part and verify
      const base64Part = dataUrl.split(',')[1]
      expect(base64Part).toBe('SGVsbG8=') // "Hello" in base64
    })
  })

  describe('roundtrip encoding/decoding', () => {
    it('should preserve all data through encode/decode cycle', () => {
      const imageData = new Uint8Array(1024).fill(0xAB)
      const videoData = new Uint8Array(2048).fill(0xCD)
      
      const original: PostContent = {
        text: 'Complete roundtrip test with all types',
        media: [
          { type: 'image/jpeg', filename: 'photo.jpg', width: 640, height: 480, data: imageData },
          { type: 'video/mp4', filename: 'movie.mp4', width: 1920, height: 1080, data: videoData },
          { type: 'image/gif', filename: 'anim.gif', width: 256, height: 256, data: new Uint8Array([0x47, 0x49, 0x46]) },
          { type: 'video/webm', filename: 'clip.webm', width: 800, height: 600, data: new Uint8Array([0x1A, 0x45]) },
        ],
      }

      const encoded = encodePostContent(original)
      const decoded = decodePostContent(encoded)

      expect(decoded.text).toBe(original.text)
      expect(decoded.media).toHaveLength(original.media.length)

      for (let i = 0; i < original.media.length; i++) {
        expect(decoded.media[i].type).toBe(original.media[i].type)
        expect(decoded.media[i].width).toBe(original.media[i].width)
        expect(decoded.media[i].height).toBe(original.media[i].height)
        expect(decoded.media[i].data).toEqual(original.media[i].data)
      }
    })
  })
})

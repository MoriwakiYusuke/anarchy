/**
 * useMediaUpload Video Tests
 * 
 * T-063: Video handling tests for useMediaUpload hook
 * Test-First Development - tests written before implementation
 */

import { renderHook, act } from '@testing-library/react'
import '@testing-library/jest-dom'

// Mock browser APIs before importing hook
const mockBitmap = {
  width: 1920,
  height: 1080,
  close: jest.fn(),
}
global.createImageBitmap = jest.fn().mockResolvedValue(mockBitmap)
global.URL.createObjectURL = jest.fn().mockReturnValue('blob:test-url')
global.URL.revokeObjectURL = jest.fn()
Object.defineProperty(global, 'crypto', {
  value: {
    randomUUID: jest.fn().mockReturnValue('test-uuid-video-1234'),
  },
})

// Note: MOCK_MERKLE_ROOT and MOCK_SHARD must be defined inline in the mock factory
// because jest.mock is hoisted

jest.mock('anarchy-wasm-engine', () => {
  const MOCK_MERKLE_ROOT = new Uint8Array(32).fill(0)
  const MOCK_SHARD = {
    index: 0,
    chunk: new Uint8Array([1, 2, 3]),
    chunk_hash: new Uint8Array(32).fill(1),
    key_share_data: new Uint8Array([10, 11, 12]),
    key_share_index: 0,
    to_bytes: () => new Uint8Array([1, 2, 3, 4, 5]),
  }
  return {
    hybrid_split: jest.fn().mockReturnValue({
      get_shard: jest.fn((idx: number) => idx < 3 ? { ...MOCK_SHARD, index: idx } : undefined),
      shard_count: 3,
      original_len: 1024,
      compressed: true,
      ciphertext_len: 1024,
      shard_size: 100,
      threshold: 2,
      total_shards: 3,
    }),
    merkle_build: jest.fn().mockReturnValue({
      root: MOCK_MERKLE_ROOT,
      leaf_count: 3,
      generate_proof: jest.fn(() => new Uint8Array(32)),
    }),
  }
})

// Mock mediaProcessor
jest.mock('@/lib/mediaProcessor', () => ({
  processMediaFile: jest.fn().mockImplementation(async (file: File) => ({
    file,
    width: 1920,
    height: 1080,
  })),
}))

// Mock videoThumbnail
jest.mock('@/lib/videoThumbnail', () => ({
  extractVideoThumbnail: jest.fn().mockImplementation(async () => ({
    thumbnail: 'data:image/jpeg;base64,/9j/4AAQSkZJRgAB...',
    width: 1920,
    height: 1080,
    duration: 120,
  })),
  getVideoDuration: jest.fn().mockResolvedValue(120),
}))

// Mock fetch for storage node RPC
global.fetch = jest.fn().mockResolvedValue({
  ok: true,
  json: () => Promise.resolve({ success: true }),
})

// Import after mocks
import { useMediaUpload } from '@/hooks/useMediaUpload'

describe('useMediaUpload Video Support', () => {
  // Test constants
  const RPC_ENDPOINT = 'http://localhost:9944'
  const MAX_VIDEO_SIZE = 1000 * 1024 * 1024 // 1GB
  const MAX_FILES = 4

  // Mock file creators
  const createMockVideoFile = (sizeMB: number = 10, name: string = 'test.mp4'): File => {
    const content = new Uint8Array(sizeMB * 1024 * 1024).fill(0xff)
    const file = new File([content], name, { type: 'video/mp4' })
    ;(file as any).arrayBuffer = async () => content.buffer
    return file
  }

  const createMockImageFile = (sizeKB: number = 100, name: string = 'test.jpg'): File => {
    const content = new Uint8Array(sizeKB * 1024).fill(0xff)
    const file = new File([content], name, { type: 'image/jpeg' })
    ;(file as any).arrayBuffer = async () => content.buffer
    return file
  }

  beforeEach(() => {
    jest.clearAllMocks()
    ;(fetch as jest.Mock).mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ success: true }),
    })
  })

  // ============================================================================
  // T-063a: Video File Addition
  // ============================================================================

  describe('Video File Addition', () => {
    it('should add a valid video file when includeVideo is true', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      const file = createMockVideoFile(50) // 50MB video

      await act(async () => {
        await result.current.addFiles([file])
      })

      expect(result.current.files).toHaveLength(1)
      expect(result.current.files[0].type).toBe('video')
      expect(result.current.files[0].status).toBe('pending')
    })

    it('should reject video files when includeVideo is false', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: false,
      }))

      const file = createMockVideoFile(50)

      await act(async () => {
        await result.current.addFiles([file])
      })

      expect(result.current.files).toHaveLength(0)
      expect(result.current.error).toBe('error.videoNotSupported')
    })

    it('should accept mixed image and video files', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      const videoFile = createMockVideoFile(50)
      const imageFile = createMockImageFile(100)

      await act(async () => {
        await result.current.addFiles([videoFile, imageFile])
      })

      expect(result.current.files).toHaveLength(2)
      expect(result.current.files[0].type).toBe('video')
      expect(result.current.files[1].type).toBe('image')
    })
  })

  // ============================================================================
  // T-063b: Video Size Validation
  // ============================================================================

  describe('Video Size Validation', () => {
    it('should reject video exceeding MAX_VIDEO_SIZE (1GB)', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      // Create a file that's larger than 1GB (mock size)
      const largeContent = new Uint8Array(100) // Small content
      const file = new File([largeContent], 'large.mp4', { type: 'video/mp4' })
      // Override size property
      Object.defineProperty(file, 'size', { value: 1.5 * 1024 * 1024 * 1024 }) // 1.5GB
      ;(file as any).arrayBuffer = async () => largeContent.buffer

      await act(async () => {
        await result.current.addFiles([file])
      })

      expect(result.current.files).toHaveLength(0)
      expect(result.current.error).toBe('error.fileTooLarge')
    })

    it('should accept video under MAX_VIDEO_SIZE', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      const file = createMockVideoFile(100) // 100MB (under 256MB limit)

      await act(async () => {
        await result.current.addFiles([file])
      })

      expect(result.current.files).toHaveLength(1)
      expect(result.current.error).toBeNull()
    })
  })

  // ============================================================================
  // T-063c: Video Format Validation
  // ============================================================================

  describe('Video Format Validation', () => {
    it('should accept MP4 format', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      const file = createMockVideoFile(10, 'test.mp4')

      await act(async () => {
        await result.current.addFiles([file])
      })

      expect(result.current.files).toHaveLength(1)
    })

    it('should accept WebM format', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      const content = new Uint8Array(10 * 1024 * 1024)
      const file = new File([content], 'test.webm', { type: 'video/webm' })
      ;(file as any).arrayBuffer = async () => content.buffer

      await act(async () => {
        await result.current.addFiles([file])
      })

      expect(result.current.files).toHaveLength(1)
    })

    it('should accept unsupported video formats as generic file type (AVI)', async () => {
      // Note: Unknown MIME types are accepted as 'file' type
      // This behavior allows any file to be uploaded for distributed storage
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      const content = new Uint8Array(10 * 1024 * 1024)
      const file = new File([content], 'test.avi', { type: 'video/x-msvideo' })
      ;(file as any).arrayBuffer = async () => content.buffer

      await act(async () => {
        await result.current.addFiles([file])
      })

      // AVI is accepted as generic 'file' type (not rejected)
      expect(result.current.files).toHaveLength(1)
      expect(result.current.files[0].type).toBe('file')
      expect(result.current.error).toBeNull()
    })
  })

  // ============================================================================
  // T-063d: Video Thumbnail Extraction
  // ============================================================================

  describe('Video Thumbnail Extraction', () => {
    it('should extract thumbnail from video file', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      const file = createMockVideoFile(10)

      await act(async () => {
        await result.current.addFiles([file])
      })

      // Thumbnail should be generated as preview
      expect(result.current.files[0].preview).toBeDefined()
    })

    it('should store video duration', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      const file = createMockVideoFile(10)

      await act(async () => {
        await result.current.addFiles([file])
      })

      // Duration should be stored (from mock: 120 seconds)
      expect(result.current.files[0].duration).toBe(120)
    })
  })

  // ============================================================================
  // T-063e: Video Upload Process
  // ============================================================================

  describe('Video Upload Process', () => {
    it('should upload video file successfully', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      const file = createMockVideoFile(10)

      await act(async () => {
        await result.current.addFiles([file])
      })

      await act(async () => {
        await result.current.uploadAll()
      })

      expect(result.current.state).toBe('complete')
      expect(result.current.files[0].status).toBe('complete')
      expect(result.current.files[0].merkleRoot).toBeDefined()
    })

    it('should track upload progress for large video', async () => {
      const onProgress = jest.fn()
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
        onProgress,
      }))

      const file = createMockVideoFile(100)

      await act(async () => {
        await result.current.addFiles([file])
      })

      await act(async () => {
        await result.current.uploadAll()
      })

      // Progress should have been called
      expect(onProgress).toHaveBeenCalled()
    })
  })

  // ============================================================================
  // T-063f: Mixed Media Upload
  // ============================================================================

  describe('Mixed Media Upload', () => {
    it('should handle mixed image and video uploads', async () => {
      const { result } = renderHook(() => useMediaUpload({
        storageNodeUrl: STORAGE_NODE_URL,
        includeVideo: true,
      }))

      const videoFile = createMockVideoFile(10)
      const imageFile = createMockImageFile(100)

      await act(async () => {
        await result.current.addFiles([videoFile, imageFile])
      })

      await act(async () => {
        await result.current.uploadAll()
      })

      expect(result.current.state).toBe('complete')
      expect(result.current.files).toHaveLength(2)
      expect(result.current.files.every(f => f.status === 'complete')).toBe(true)
    })
  })
})

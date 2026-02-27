/**
 * PostItem Component Tests
 * 
 * T059: Test that PostItem uses shared WorkerPool
 */

import { render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import { PostItem } from '@/components/PostItem'

// Mock postCodec
let mockDecodedText = ''
let mockDecodedMedia: Array<{ mime: string; data: Uint8Array }> = []
jest.mock('@/lib/postCodec', () => ({
  decodePostContent: jest.fn().mockImplementation(() => ({
    text: mockDecodedText,
    media: mockDecodedMedia,
  })),
  mediaToDataUrl: jest.fn().mockImplementation((item: { mime: string; data: Uint8Array }) => 
    `data:${item.mime};base64,mock`
  ),
}))

// Mock CSS module
jest.mock('@/components/Timeline.module.css', () => ({
  post: 'post',
  postHeader: 'postHeader',
  author: 'author',
  block: 'block',
  content: 'content',
  text: 'text',
  postFooter: 'postFooter',
  postId: 'postId',
  reply: 'reply',
  contentLoading: 'contentLoading',
  error: 'error',
}))

// Mock the i18n hook
jest.mock('@/i18n/context', () => ({
  useLocale: () => ({
    t: (key: string, params?: Record<string, string>) => {
      const translations: Record<string, string> = {
        'content.loading': '読み込み中...',
        'content.error': params?.error || 'エラー',
      }
      return translations[key] || key
    },
  }),
}))

// Mock useStorage hook
const mockRecoverContent = jest.fn()
let mockIsReady = true

jest.mock('@/hooks/useStorage', () => ({
  useStorage: () => ({
    recoverContent: mockRecoverContent,
    isReady: mockIsReady,
  }),
}))

// Shared pool mock tracking
const sharedPoolSpy = {
  executeCount: 0,
  poolInstance: null as unknown,
}

// Mock WorkerPool - 共有プール使用を追跡
jest.mock('@/workers/WorkerPool', () => {
  const mockPool = {
    execute: jest.fn().mockImplementation(() => {
      sharedPoolSpy.executeCount++
      return Promise.resolve(new Uint8Array(32))
    }),
    waitUntilReady: jest.fn().mockResolvedValue(undefined),
    isReady: true,
    size: 4,
    terminate: jest.fn(),
  }
  sharedPoolSpy.poolInstance = mockPool

  return {
    WorkerPool: jest.fn().mockReturnValue(mockPool),
    getSharedWorkerPool: jest.fn().mockReturnValue(mockPool),
    resetSharedWorkerPool: jest.fn(),
  }
})

describe('PostItem', () => {
  const defaultProps = {
    postId: 1,
    author: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
    contentHash: 'abc123',
    createdAt: 1000,
    parentId: null,
  }

  beforeEach(() => {
    jest.clearAllMocks()
    sharedPoolSpy.executeCount = 0
    mockRecoverContent.mockReset()
    mockIsReady = true
    mockDecodedText = ''
    mockDecodedMedia = []
  })

  describe('V1 inline content', () => {
    it('renders inline content directly without using Worker', async () => {
      const inlineContent = 'Hello, World!'
      
      render(
        <PostItem
          {...defaultProps}
          inlineContent={inlineContent}
        />
      )

      // インラインコンテンツが直接表示される
      expect(screen.getByText(inlineContent)).toBeInTheDocument()
      
      // Workerは使用されない
      expect(mockRecoverContent).not.toHaveBeenCalled()
    })

    it('displays author address shortened', () => {
      render(
        <PostItem
          {...defaultProps}
          inlineContent="Test content"
        />
      )

      expect(screen.getByText('5GrwvaEF...GKutQY')).toBeInTheDocument()
    })

    it('displays post ID', () => {
      render(
        <PostItem
          {...defaultProps}
          inlineContent="Test content"
        />
      )

      expect(screen.getByText(/Post #1/)).toBeInTheDocument()
    })
  })

  describe('V2 distributed storage content', () => {
    const contentRef = {
      root: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
             17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32],
      k: 3,
      n: 5,
      total_size: 100,
      ciphertext_len: 128,
      shard_size: 43,
      compressed: false,
    }

    it('test_post_item_uses_shared_pool - recovers content using shared WorkerPool via useStorage', async () => {
      mockDecodedText = 'Recovered content from distributed storage'
      mockRecoverContent.mockResolvedValueOnce({ data: new Uint8Array([]) })

      render(
        <PostItem
          {...defaultProps}
          contentRef={contentRef}
        />
      )

      // recoverContent が呼び出される（内部で共有WorkerPoolを使用）
      await waitFor(() => {
        expect(mockRecoverContent).toHaveBeenCalledTimes(1)
      })

      // 復元されたコンテンツが表示される
      await waitFor(() => {
        expect(screen.getByText('Recovered content from distributed storage')).toBeInTheDocument()
      })
    })

    it('shows loading state while recovering content', async () => {
      // 遅延を追加
      mockRecoverContent.mockImplementationOnce(() => 
        new Promise(resolve => setTimeout(() => resolve({ data: new Uint8Array([]) }), 100))
      )

      render(
        <PostItem
          {...defaultProps}
          contentRef={contentRef}
        />
      )

      // ローディング状態が表示される
      expect(screen.getByText('読み込み中...')).toBeInTheDocument()
    })

    it('shows error state when recovery fails', async () => {
      mockRecoverContent.mockRejectedValueOnce(new Error('Network error'))

      render(
        <PostItem
          {...defaultProps}
          contentRef={contentRef}
        />
      )

      await waitFor(() => {
        expect(screen.getByText(/Network error/)).toBeInTheDocument()
      })
    })

    it('waits for isReady before recovering', async () => {
      mockIsReady = false

      render(
        <PostItem
          {...defaultProps}
          contentRef={contentRef}
        />
      )

      // isReady が false の間は recoverContent は呼び出されない
      expect(mockRecoverContent).not.toHaveBeenCalled()
    })
  })

  describe('reply handling', () => {
    it('shows reply indicator when parentId is set', () => {
      render(
        <PostItem
          {...defaultProps}
          inlineContent="Reply content"
          parentId={42}
        />
      )

      expect(screen.getByText(/Reply to #42/)).toBeInTheDocument()
    })

    it('does not show reply indicator when parentId is null', () => {
      render(
        <PostItem
          {...defaultProps}
          inlineContent="Original post"
          parentId={null}
        />
      )

      expect(screen.queryByText(/Reply to/)).not.toBeInTheDocument()
    })
  })
})

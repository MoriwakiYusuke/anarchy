import { render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import { Timeline } from '@/components/Timeline'

// Mock the i18n hook
jest.mock('@/i18n', () => ({
  useLocale: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'timeline.loading': '読み込み中...',
        'timeline.empty': '投稿がありません',
      }
      return translations[key] || key
    },
  }),
}))

// Mock PostItem — assert author + that contentRef was forwarded.
jest.mock('@/components/PostItem', () => ({
  PostItem: ({ postId, author, contentRef }: { postId: number; author: string; contentRef?: unknown }) => (
    <div data-testid={`post-${postId}`}>
      <span data-testid={`author-${postId}`}>{author}</span>
      <span data-testid={`has-ref-${postId}`}>{contentRef ? 'yes' : 'no'}</span>
    </div>
  ),
}))

jest.mock('@/components/Timeline.module.css', () => ({
  loading: 'loading',
  empty: 'empty',
  timeline: 'timeline',
}))

describe('Timeline', () => {
  describe('loading states', () => {
    it('shows loading state initially', () => {
      const mockUnsafeApi = {
        query: {
          Post: {
            Posts: {
              getEntries: jest.fn().mockImplementation(() => new Promise(() => {})),
            },
          },
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)
      expect(screen.getByText('読み込み中...')).toBeInTheDocument()
    })

    it('shows empty state when no posts', async () => {
      const mockUnsafeApi = {
        query: {
          Post: {
            Posts: {
              getEntries: jest.fn().mockResolvedValue([]),
            },
            ContentRefs: {
              getEntries: jest.fn().mockResolvedValue([]),
            },
          },
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)

      await waitFor(() => {
        expect(screen.getByText('投稿がありません')).toBeInTheDocument()
      })
    })
  })

  describe('error handling', () => {
    it('handles missing Post pallet gracefully', async () => {
      const mockUnsafeApi = {
        query: {
          // No Post pallet
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)

      await waitFor(() => {
        expect(screen.getByText('投稿がありません')).toBeInTheDocument()
      })
    })
  })

  describe('content refs', () => {
    it('forwards ContentRef to PostItem when present', async () => {
      const mockUnsafeApi = {
        query: {
          Post: {
            Posts: {
              getEntries: jest.fn().mockResolvedValue([
                {
                  keyArgs: [1],
                  value: {
                    author: '0x1234567890abcdef',
                    content_hash: { asHex: () => '0xabc123' },
                    created_at: 100,
                  },
                },
              ]),
            },
            ContentRefs: {
              getEntries: jest.fn().mockResolvedValue([
                {
                  keyArgs: [1],
                  value: {
                    root: { asBytes: () => new Uint8Array(32).fill(7) },
                    k: 3,
                    n: 5,
                    size: 100,
                    ciphertext_len: 128,
                    shard_size: 43,
                    compressed: false,
                  },
                },
              ]),
            },
          },
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)

      await waitFor(() => {
        expect(screen.getByTestId('post-1')).toBeInTheDocument()
        expect(screen.getByTestId('has-ref-1')).toHaveTextContent('yes')
      })
    })
  })
})

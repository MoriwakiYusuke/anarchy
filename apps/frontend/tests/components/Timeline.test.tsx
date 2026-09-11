import { render, screen, waitFor, act } from '@testing-library/react'
import '@testing-library/jest-dom'
import { Timeline } from '@/components/Timeline'

// mount 回数を数えて「refresh で PostItem が unmount されていない」ことを見る
let postItemMounts = 0

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
jest.mock('@/components/PostItem', () => {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const React = require('react')
  return {
    PostItem: ({ postId, author, contentRef }: { postId: number; author: string; contentRef?: unknown }) => {
      React.useEffect(() => {
        postItemMounts += 1
      }, [])
      return (
        <div data-testid={`post-${postId}`}>
          <span data-testid={`author-${postId}`}>{author}</span>
          <span data-testid={`has-ref-${postId}`}>{contentRef ? 'yes' : 'no'}</span>
        </div>
      )
    },
  }
})

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

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} account={null} signer={null} />)
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

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} account={null} signer={null} />)

      await waitFor(() => {
        expect(screen.getByText('投稿がありません')).toBeInTheDocument()
      })
    })
  })

  describe('refresh keeps mounted posts', () => {
    const entry = (id: number) => ({
      keyArgs: [id],
      value: {
        author: '0x1234567890abcdef',
        content_hash: { asHex: () => '0xabc123' },
        created_at: 100 + id,
      },
    })

    it('does not swap in the loading placeholder (and unmount PostItems) on refreshTrigger', async () => {
      postItemMounts = 0
      // 2 回目の fetch は手動で resolve して「ロード中」の瞬間を観測する
      let resolveSecond: (v: unknown[]) => void = () => {}
      const getEntries = jest
        .fn()
        .mockResolvedValueOnce([entry(1)])
        .mockImplementationOnce(() => new Promise<unknown[]>((r) => { resolveSecond = r }))
      const mockUnsafeApi = {
        query: {
          Post: {
            Posts: { getEntries },
            ContentRefs: { getEntries: jest.fn().mockResolvedValue([]) },
          },
        },
      }

      const { rerender } = render(
        <Timeline client={null} unsafeApi={mockUnsafeApi} account={null} signer={null} refreshTrigger={0} />,
      )
      await waitFor(() => expect(screen.getByTestId('post-1')).toBeInTheDocument())
      expect(postItemMounts).toBe(1)

      // 返信投稿後の onReplyPosted 相当: refreshTrigger を bump
      rerender(
        <Timeline client={null} unsafeApi={mockUnsafeApi} account={null} signer={null} refreshTrigger={1} />,
      )
      await waitFor(() => expect(getEntries).toHaveBeenCalledTimes(2))

      // 再フェッチ中も既存の投稿は表示されたまま (ローディング差し替えで消えない)
      expect(screen.queryByText('読み込み中...')).not.toBeInTheDocument()
      expect(screen.getByTestId('post-1')).toBeInTheDocument()

      await act(async () => {
        resolveSecond([entry(1), entry(2)])
      })
      await waitFor(() => expect(screen.getByTestId('post-2')).toBeInTheDocument())
      // post-1 は remount されていない (repliesExpanded 等のローカル state が保たれる)
      expect(postItemMounts).toBe(2)
    })
  })

  describe('error handling', () => {
    it('handles missing Post pallet gracefully', async () => {
      const mockUnsafeApi = {
        query: {
          // No Post pallet
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} account={null} signer={null} />)

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

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} account={null} signer={null} />)

      await waitFor(() => {
        expect(screen.getByTestId('post-1')).toBeInTheDocument()
        expect(screen.getByTestId('has-ref-1')).toHaveTextContent('yes')
      })
    })
  })
})

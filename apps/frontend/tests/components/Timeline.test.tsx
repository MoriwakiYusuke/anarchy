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

// Mock PostItem component - 実際のprops形式に合わせる
jest.mock('@/components/PostItem', () => ({
  PostItem: ({ postId, author, inlineContent }: { postId: number; author: string; inlineContent?: string }) => (
    <div data-testid={`post-${postId}`}>
      <span data-testid={`author-${postId}`}>{author}</span>
      <span data-testid={`content-${postId}`}>{inlineContent || ''}</span>
    </div>
  ),
}))

// Mock CSS module
jest.mock('@/components/Timeline.module.css', () => ({
  loading: 'loading',
  empty: 'empty',
  timeline: 'timeline',
}))

describe('Timeline', () => {
  describe('PAPI value format handling', () => {
    it('handles value with asBytes() method', async () => {
      const mockContent = 'Test content with asBytes'
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
            Contents: {
              getEntries: jest.fn().mockResolvedValue([
                {
                  keyArgs: [1],
                  value: {
                    asBytes: () => new TextEncoder().encode(mockContent),
                  },
                },
              ]),
            },
            ContentRefs: {
              getEntries: jest.fn().mockResolvedValue([]),
            },
          },
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)

      await waitFor(() => {
        expect(screen.getByTestId('content-1')).toHaveTextContent(mockContent)
      })
    })

    it('handles value as direct Uint8Array', async () => {
      const mockContent = 'Test content as Uint8Array'
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
            Contents: {
              getEntries: jest.fn().mockResolvedValue([
                {
                  keyArgs: [1],
                  // Direct Uint8Array (no asBytes method)
                  value: new TextEncoder().encode(mockContent),
                },
              ]),
            },
            ContentRefs: {
              getEntries: jest.fn().mockResolvedValue([]),
            },
          },
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)

      await waitFor(() => {
        expect(screen.getByTestId('content-1')).toHaveTextContent(mockContent)
      })
    })

    it('handles value as Array of numbers', async () => {
      const mockContent = 'Array content'
      const bytes = Array.from(new TextEncoder().encode(mockContent))
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
            Contents: {
              getEntries: jest.fn().mockResolvedValue([
                {
                  keyArgs: [1],
                  // Array of byte values
                  value: bytes,
                },
              ]),
            },
            ContentRefs: {
              getEntries: jest.fn().mockResolvedValue([]),
            },
          },
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)

      await waitFor(() => {
        expect(screen.getByTestId('content-1')).toHaveTextContent(mockContent)
      })
    })

    it('handles BoundedVec object with .value property', async () => {
      const mockContent = 'BoundedVec content'
      const bytes = Array.from(new TextEncoder().encode(mockContent))
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
            Contents: {
              getEntries: jest.fn().mockResolvedValue([
                {
                  keyArgs: [1],
                  // BoundedVec-like object
                  value: { value: bytes },
                },
              ]),
            },
            ContentRefs: {
              getEntries: jest.fn().mockResolvedValue([]),
            },
          },
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)

      await waitFor(() => {
        expect(screen.getByTestId('content-1')).toHaveTextContent(mockContent)
      })
    })

    it('handles null/undefined value gracefully', async () => {
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
            Contents: {
              getEntries: jest.fn().mockResolvedValue([
                {
                  keyArgs: [1],
                  value: null,
                },
              ]),
            },
            ContentRefs: {
              getEntries: jest.fn().mockResolvedValue([]),
            },
          },
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)

      // Should render without crashing, content will be empty
      await waitFor(() => {
        expect(screen.getByTestId('content-1')).toHaveTextContent('')
      })
    })

    it('handles value object without asBytes but with nested structure', async () => {
      const mockContent = 'Nested structure'
      const bytes = new TextEncoder().encode(mockContent)
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
            Contents: {
              getEntries: jest.fn().mockResolvedValue([
                {
                  keyArgs: [1],
                  // Object that looks like it might have asBytes but doesn't
                  value: {
                    // No asBytes method
                    value: Array.from(bytes),
                  },
                },
              ]),
            },
            ContentRefs: {
              getEntries: jest.fn().mockResolvedValue([]),
            },
          },
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)

      await waitFor(() => {
        expect(screen.getByTestId('content-1')).toHaveTextContent(mockContent)
      })
    })
  })

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
            Contents: {
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

      // Should show empty state without crashing
      await waitFor(() => {
        expect(screen.getByText('投稿がありません')).toBeInTheDocument()
      })
    })

    it('handles Contents storage error gracefully', async () => {
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
            Contents: {
              getEntries: jest.fn().mockRejectedValue(new Error('Storage error')),
            },
            ContentRefs: {
              getEntries: jest.fn().mockResolvedValue([]),
            },
          },
        },
      }

      render(<Timeline client={null} unsafeApi={mockUnsafeApi} />)

      // Should render post without crashing, content will be empty
      await waitFor(() => {
        expect(screen.getByTestId('post-1')).toBeInTheDocument()
      })
    })
  })
})

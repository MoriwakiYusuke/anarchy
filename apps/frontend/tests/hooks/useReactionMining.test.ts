/**
 * useReactionMining Hook Tests
 * 
 * T041: [US2] Jest test: useReactionMining pauses on visibility change
 * T042: [US2] Jest test: useReactionMining resumes on visibility return
 * T043: [US2] Jest test: mining state correctly reflects isPaused
 * 
 * Feature: 017-reaction-mining
 */

import { renderHook, act, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

/** Timeout for async test operations in milliseconds */
const TEST_TIMEOUT_MS = 5000

// Store original document.hidden
let originalHidden: boolean

// Mock document.hidden
const mockDocumentHidden = (hidden: boolean) => {
  Object.defineProperty(document, 'hidden', {
    configurable: true,
    get: () => hidden,
  })
}

// Mock Worker class
class MockCryptoWorker {
  onmessage: ((event: MessageEvent) => void) | null = null
  onerror: ((event: ErrorEvent) => void) | null = null
  private terminated = false
  private miningTimeout: NodeJS.Timeout | null = null

  constructor() {
    setTimeout(() => {
      if (!this.terminated) {
        this.onmessage?.({ data: { type: 'ready' } } as MessageEvent)
      }
    }, 0)
  }

  postMessage(message: {
    id: string
    type: string
    payload: unknown
  }) {
    if (this.terminated) return

    if (message.type === 'mine_reaction') {
      // Simulate slow mining - will complete after 100ms
      this.miningTimeout = setTimeout(() => {
        if (this.terminated) return
        this.onmessage?.({
          data: {
            id: message.id,
            success: true,
            result: {
              nonce: BigInt(999),
              iterations: 5000,
              hashRate: 50000,
              elapsedMs: 100,
            },
          },
        } as MessageEvent)
      }, 100)
    }
  }

  terminate() {
    this.terminated = true
    if (this.miningTimeout) {
      clearTimeout(this.miningTimeout)
    }
    this.onmessage = null
    this.onerror = null
  }
}

// Store original Worker
const OriginalWorker = global.Worker

// Mock getReactionChallenge and submitReaction
jest.mock('@/services/reactionService', () => ({
  getReactionChallenge: jest.fn().mockResolvedValue({
    challenge: new Uint8Array(32).fill(0xab),
    blockNumber: 100,
    difficulty: 8,
  }),
  submitReaction: jest.fn().mockResolvedValue({
    success: true,
    txHash: '0x1234567890abcdef',
    reward: BigInt(1_000_000_000_000), // 1 MORAL
  }),
  ReactionType: {
    Like: 'Like',
    Boost: 'Boost',
    Bad: 'Bad',
  },
}))

// Import hook after mocks
import { useReactionMining, MiningStatus, ReactionType } from '@/hooks/useReactionMining'

describe('useReactionMining Hook - Visibility API', () => {
  beforeAll(() => {
    originalHidden = document.hidden
    // @ts-ignore - Mock Worker
    global.Worker = MockCryptoWorker
  })

  beforeEach(() => {
    jest.useFakeTimers()
    mockDocumentHidden(false)
  })

  afterEach(() => {
    jest.useRealTimers()
    jest.clearAllMocks()
  })

  afterAll(() => {
    mockDocumentHidden(originalHidden)
    global.Worker = OriginalWorker
  })

  const mockClient = {
    _request: jest.fn().mockResolvedValue('0x1234'),
  }

  const mockUnsafeApi = {
    query: {
      Reaction: {
        CurrentDifficulty: {
          getValue: jest.fn().mockResolvedValue(8),
        },
      },
    },
    tx: {
      Reaction: {
        react: jest.fn().mockReturnValue({
          signAndSubmit: jest.fn().mockResolvedValue({ success: true }),
        }),
      },
    },
  }

  const mockSigner = {
    publicKey: new Uint8Array(32),
    sign: jest.fn(),
  }

  /**
   * T041: useReactionMining pauses on visibility change
   */
  test('should pause mining when document becomes hidden', async () => {
    const { result } = renderHook(() =>
      useReactionMining({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        signer: mockSigner as any,
      })
    )

    // Start mining
    await act(async () => {
      await result.current.startMining(BigInt(1), ReactionType.Like)
      jest.advanceTimersByTime(10) // Allow challenge fetch
    })

    // Verify mining started
    await waitFor(
      () => {
        expect(result.current.status).toBe('mining')
      },
      { timeout: TEST_TIMEOUT_MS }
    )

    // Simulate tab becoming hidden
    act(() => {
      mockDocumentHidden(true)
      document.dispatchEvent(new Event('visibilitychange'))
    })

    // Should pause mining
    await waitFor(
      () => {
        expect(result.current.status).toBe('paused')
      },
      { timeout: TEST_TIMEOUT_MS }
    )
  })

  /**
   * T042: useReactionMining resumes on visibility return
   */
  test('should allow resuming mining when tab becomes visible again', async () => {
    const { result } = renderHook(() =>
      useReactionMining({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        signer: mockSigner as any,
      })
    )

    // Start mining
    await act(async () => {
      await result.current.startMining(BigInt(1), ReactionType.Like)
      jest.advanceTimersByTime(10)
    })

    await waitFor(
      () => {
        expect(result.current.status).toBe('mining')
      },
      { timeout: TEST_TIMEOUT_MS }
    )

    // Pause by hiding tab
    act(() => {
      mockDocumentHidden(true)
      document.dispatchEvent(new Event('visibilitychange'))
    })

    await waitFor(
      () => {
        expect(result.current.status).toBe('paused')
      },
      { timeout: TEST_TIMEOUT_MS }
    )

    // Tab becomes visible again
    mockDocumentHidden(false)

    // User resumes mining
    await act(async () => {
      await result.current.resume()
      jest.advanceTimersByTime(10)
    })

    // Should be mining again
    await waitFor(
      () => {
        expect(result.current.status).toBe('mining')
      },
      { timeout: TEST_TIMEOUT_MS }
    )
  })

  /**
   * T043: mining state correctly reflects isPaused (via error.code)
   */
  test('should set VisibilityPaused error when mining is paused', async () => {
    const { result } = renderHook(() =>
      useReactionMining({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        signer: mockSigner as any,
      })
    )

    // Start mining
    await act(async () => {
      await result.current.startMining(BigInt(1), ReactionType.Like)
      jest.advanceTimersByTime(10)
    })

    await waitFor(
      () => {
        expect(result.current.status).toBe('mining')
      },
      { timeout: TEST_TIMEOUT_MS }
    )

    // Hide tab
    act(() => {
      mockDocumentHidden(true)
      document.dispatchEvent(new Event('visibilitychange'))
    })

    // Should have paused status and VisibilityPaused error
    await waitFor(
      () => {
        expect(result.current.status).toBe('paused')
        expect(result.current.error).not.toBeNull()
        expect(result.current.error?.code).toBe('VisibilityPaused')
      },
      { timeout: TEST_TIMEOUT_MS }
    )
  })

  /**
   * Additional test: cancel clears state
   */
  test('should clear state when cancel is called', async () => {
    const { result } = renderHook(() =>
      useReactionMining({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        signer: mockSigner as any,
      })
    )

    // Start mining
    await act(async () => {
      await result.current.startMining(BigInt(1), ReactionType.Like)
      jest.advanceTimersByTime(10)
    })

    await waitFor(
      () => {
        expect(result.current.status).toBe('mining')
      },
      { timeout: TEST_TIMEOUT_MS }
    )

    // Cancel mining
    act(() => {
      result.current.cancel()
    })

    // Should return to idle
    expect(result.current.status).toBe('idle')
    expect(result.current.error).toBeNull()
    expect(result.current.progress).toBeNull()
  })

  /**
   * Additional test: initial state is idle
   */
  test('should have idle status initially', () => {
    const { result } = renderHook(() =>
      useReactionMining({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        signer: mockSigner as any,
      })
    )

    expect(result.current.status).toBe('idle')
    expect(result.current.error).toBeNull()
    expect(result.current.progress).toBeNull()
    expect(result.current.result).toBeNull()
  })
})

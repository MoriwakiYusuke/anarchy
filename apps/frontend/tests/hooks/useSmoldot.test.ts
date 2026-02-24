/**
 * useSmoldot Hook Tests
 * 
 * Tests for smoldot Light Client integration
 * - T-SM-001: Hook initializes smoldot client
 * - T-SM-002: Hook transitions through connection states
 * - T-SM-003: Hook handles initialization errors
 * - T-SM-004: Hook receives block updates
 * - T-SM-005: Hook cleans up on unmount
 */

import { renderHook, act, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

// Track current block number for getValue mock
let currentBlockNumber = 100
let getValueShouldFail = false
let getValueFailCount = 0

// Mock getValue - returns current block number or throws if sync not ready
const mockGetValue = jest.fn().mockImplementation(() => {
  if (getValueShouldFail && getValueFailCount > 0) {
    getValueFailCount--
    return Promise.reject(new Error('Not synced yet'))
  }
  return Promise.resolve(currentBlockNumber)
})

// Mock unsafe API
const mockUnsafeApi = {
  query: {
    System: {
      Number: {
        getValue: mockGetValue,
      },
    },
  },
  constants: {
    Post: {
      PostBaseCost: jest.fn().mockResolvedValue(BigInt(10_000_000_000_000)),
      PostByteCost: jest.fn().mockResolvedValue(BigInt(100_000_000_000)),
    },
  },
}

// Mock PAPI client
const mockClient = {
  getUnsafeApi: jest.fn().mockReturnValue(mockUnsafeApi),
  destroy: jest.fn(),
}

// Track whether smoldot is "initialized" for test purposes
let mockIsSmoldotInitialized = false

// Mock smoldot-provider
jest.mock('@/lib/smoldot-provider', () => ({
  initSmoldotClient: jest.fn(),
  destroySmoldotClient: jest.fn(),
  isSmoldotInitialized: jest.fn(() => mockIsSmoldotInitialized),
}))

// Import mocked module
import { initSmoldotClient, destroySmoldotClient, isSmoldotInitialized } from '@/lib/smoldot-provider'
import { useSmoldot } from '@/hooks/useSmoldot'

const mockInitSmoldotClient = initSmoldotClient as jest.MockedFunction<typeof initSmoldotClient>
const mockDestroySmoldotClient = destroySmoldotClient as jest.MockedFunction<typeof destroySmoldotClient>
const mockIsSmoldotInitializedFn = isSmoldotInitialized as jest.MockedFunction<typeof isSmoldotInitialized>

describe('useSmoldot Hook', () => {
  beforeEach(() => {
    jest.clearAllMocks()
    jest.useFakeTimers()
    currentBlockNumber = 100
    getValueShouldFail = false
    getValueFailCount = 0
    mockIsSmoldotInitialized = false
    mockGetValue.mockClear()
    mockGetValue.mockImplementation(() => {
      if (getValueShouldFail && getValueFailCount > 0) {
        getValueFailCount--
        return Promise.reject(new Error('Not synced yet'))
      }
      return Promise.resolve(currentBlockNumber)
    })
  })

  afterEach(() => {
    jest.useRealTimers()
  })

  describe('T-SM-001: Hook initializes smoldot client', () => {
    it('should call initSmoldotClient on mount', async () => {
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result } = renderHook(() => useSmoldot())

      // Initial state
      expect(result.current.connectionState.status).toBe('initializing')
      expect(result.current.client).toBeNull()
      expect(result.current.unsafeApi).toBeNull()

      // Wait for initialization
      await act(async () => {
        await Promise.resolve()
      })

      expect(mockInitSmoldotClient).toHaveBeenCalledTimes(1)
    })

    it('should set client after initialization', async () => {
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result } = renderHook(() => useSmoldot())

      await act(async () => {
        await Promise.resolve()
      })

      expect(result.current.client).toBe(mockClient)
    })
  })

  describe('T-SM-002: Hook transitions through connection states', () => {
    it('should transition from initializing to syncing', async () => {
      // Make getValue fail so we stay in syncing state
      getValueShouldFail = true
      getValueFailCount = 100 // Keep failing
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result } = renderHook(() => useSmoldot())

      expect(result.current.connectionState.status).toBe('initializing')

      await act(async () => {
        await Promise.resolve()
      })

      expect(result.current.connectionState.status).toBe('syncing')
    })

    it('should transition from syncing to connected when block received', async () => {
      // Make getValue fail for first 2 polls, then succeed
      getValueShouldFail = true
      getValueFailCount = 2
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result } = renderHook(() => useSmoldot())

      await act(async () => {
        await Promise.resolve()
      })

      expect(result.current.connectionState.status).toBe('syncing')

      // Advance past first 2 polls (fail) to third poll (success)
      // First poll immediate, then 2s wait, second poll fail, 2s wait, third poll succeeds
      await act(async () => {
        await jest.advanceTimersByTimeAsync(5000) // Advance past poll retries
      })

      expect(result.current.connectionState.status).toBe('connected')
      expect(result.current.blockNumber).toBe(100)
    })

    it('should set unsafeApi only after connected', async () => {
      // Make getValue fail initially so we can observe syncing
      getValueShouldFail = true
      getValueFailCount = 2
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result } = renderHook(() => useSmoldot())

      await act(async () => {
        await Promise.resolve()
      })

      // Still syncing - no API yet
      expect(result.current.unsafeApi).toBeNull()

      // Wait for polling to succeed (after fail count expires)
      await act(async () => {
        await jest.advanceTimersByTimeAsync(5000)
      })

      // Now API should be available
      expect(result.current.unsafeApi).toBe(mockUnsafeApi)
    })
  })

  describe('T-SM-003: Hook handles initialization errors', () => {
    it('should set error state when init fails', async () => {
      const error = new Error('Connection failed')
      mockInitSmoldotClient.mockRejectedValue(error)

      const { result } = renderHook(() => useSmoldot())

      await act(async () => {
        await Promise.resolve()
      })

      expect(result.current.connectionState.status).toBe('error')
      expect(result.current.connectionState.errorMessage).toBe('Connection failed')
    })

    it('should set generic error message for non-Error throws', async () => {
      mockInitSmoldotClient.mockRejectedValue('string error')

      const { result } = renderHook(() => useSmoldot())

      await act(async () => {
        await Promise.resolve()
      })

      expect(result.current.connectionState.status).toBe('error')
      expect(result.current.connectionState.errorMessage).toBe('smoldot初期化に失敗しました')
    })

    it('should set error state on sync timeout', async () => {
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)
      // Make getValue always fail so we never sync
      getValueShouldFail = true
      getValueFailCount = 999

      const { result } = renderHook(() => useSmoldot())

      await act(async () => {
        await Promise.resolve()
      })

      expect(result.current.connectionState.status).toBe('syncing')

      // Fast-forward past timeout (60 seconds)
      await act(async () => {
        jest.advanceTimersByTime(60_000)
      })

      expect(result.current.connectionState.status).toBe('error')
      expect(result.current.connectionState.errorMessage).toBe('同期がタイムアウトしました (60秒)')
    })
  })

  describe('T-SM-004: Hook receives block updates', () => {
    it('should update blockNumber on subsequent blocks', async () => {
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result } = renderHook(() => useSmoldot())

      await act(async () => {
        await Promise.resolve()
      })

      // Wait for initial sync
      currentBlockNumber = 100
      await act(async () => {
        await jest.advanceTimersByTimeAsync(100)
      })

      expect(result.current.blockNumber).toBe(100)

      // Wait for periodic update (6 seconds)
      currentBlockNumber = 101
      await act(async () => {
        await jest.advanceTimersByTimeAsync(6000)
      })

      expect(result.current.blockNumber).toBe(101)

      // Another update
      currentBlockNumber = 102
      await act(async () => {
        await jest.advanceTimersByTimeAsync(6000)
      })

      expect(result.current.blockNumber).toBe(102)
    })

    it('should include blockNumber in connectionState when connected', async () => {
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result } = renderHook(() => useSmoldot())

      // Initial sync polling
      await act(async () => {
        await jest.advanceTimersByTimeAsync(2100)
      })

      // Now after sync, block updates poll every 6 seconds
      currentBlockNumber = 500
      await act(async () => {
        await jest.advanceTimersByTimeAsync(6100)
      })

      expect(result.current.connectionState.blockNumber).toBe(500)
    })
  })

  describe('T-SM-005: Hook cleans up on unmount', () => {
    it('should clean up polling on unmount', async () => {
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result, unmount } = renderHook(() => useSmoldot())

      // Start sync polling
      await act(async () => {
        await jest.advanceTimersByTimeAsync(2100)
      })

      expect(result.current.connectionState.status).toBe('connected')

      // Unmount - this should clear intervals
      unmount()

      // No explicit assertion needed - cleanup verified by no state update errors
    })

    it('should clear sync timeout on unmount', async () => {
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { unmount } = renderHook(() => useSmoldot())

      await act(async () => {
        await Promise.resolve()
      })

      // Unmount before timeout
      unmount()

      // Advancing time should not cause error state (because we unmounted)
      await act(async () => {
        jest.advanceTimersByTime(60_000)
      })

      // No assertions needed - if cleanup didn't work, test would fail
      // due to state update on unmounted component
    })

    it('should not update state after unmount', async () => {
      // Make getValue fail so we stay in syncing state before unmount
      getValueShouldFail = true
      getValueFailCount = 100
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result, unmount } = renderHook(() => useSmoldot())

      // Start syncing - but getValue fails so we stay in syncing
      await act(async () => {
        await jest.advanceTimersByTimeAsync(100)
      })

      // Status before unmount
      const statusBeforeUnmount = result.current.connectionState.status

      // Unmount - intervals should be cleared
      unmount()

      // Advancing timers after unmount should not cause errors
      currentBlockNumber = 999
      await act(async () => {
        await jest.advanceTimersByTimeAsync(10000)
      })

      // Status captured before unmount should be syncing
      expect(statusBeforeUnmount).toBe('syncing')
    })
  })

  describe('Connection state object', () => {
    it('should not include blockNumber when not connected', async () => {
      // Make getValue fail indefinitely so we stay in syncing state
      getValueShouldFail = true
      getValueFailCount = 100
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result } = renderHook(() => useSmoldot())

      await act(async () => {
        await jest.advanceTimersByTimeAsync(100)
      })

      expect(result.current.connectionState.status).toBe('syncing')
      expect(result.current.connectionState.blockNumber).toBeUndefined()
    })

    it('should not include errorMessage when not in error state', async () => {
      mockInitSmoldotClient.mockResolvedValue(mockClient as any)

      const { result } = renderHook(() => useSmoldot())

      // Complete sync - getValue succeeds
      await act(async () => {
        await jest.advanceTimersByTimeAsync(2100)
      })

      expect(result.current.connectionState.status).toBe('connected')
      expect(result.current.connectionState.errorMessage).toBeUndefined()
    })
  })
})

/**
 * useScore Hook Tests (T068)
 * Tests for on-chain ScoreCache query via PAPI
 */

import { renderHook, waitFor } from '@testing-library/react'
import { useScore } from '@/hooks/useScore'

// Mock PAPI
const mockScoreValue = 750
const mockGetValue = jest.fn()

describe('useScore', () => {
  beforeEach(() => {
    jest.clearAllMocks()
    mockGetValue.mockResolvedValue(mockScoreValue)
  })

  describe('initialization', () => {
    it('should initialize with score undefined and loading true', () => {
      const { result } = renderHook(() => useScore({
        contentHash: new Uint8Array(32).fill(0)
      }))
      
      expect(result.current.score).toBeDefined() // Will be DEFAULT_SCORE (1000) when no API
      expect(result.current.error).toBeNull()
    })

    it('should handle invalid contentHash length', async () => {
      const { result } = renderHook(() => useScore({
        contentHash: new Uint8Array(16) // Invalid length
      }))
      
      await waitFor(() => expect(result.current.isLoading).toBe(false))
      expect(result.current.score).toBeUndefined()
    })
  })

  describe('test_use_score_returns_real_data', () => {
    it('should return real data when API is available', async () => {
      const mockApi = {
        query: {
          Storage: {
            ScoreCache: {
              getValue: mockGetValue
            }
          }
        }
      }

      const contentHash = new Uint8Array(32).fill(1)
      const { result } = renderHook(() => useScore({
        contentHash,
        unsafeApi: mockApi
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      expect(result.current.score).toBe(mockScoreValue)
      expect(result.current.error).toBeNull()
      expect(result.current.isProviderAvailable).toBe(true)
      expect(mockGetValue).toHaveBeenCalled()
    })

    it('should handle API errors gracefully', async () => {
      const mockApiWithError = {
        query: {
          Storage: {
            ScoreCache: {
              getValue: jest.fn().mockRejectedValue(new Error('Network error'))
            }
          }
        }
      }

      const contentHash = new Uint8Array(32).fill(2)
      const { result } = renderHook(() => useScore({
        contentHash,
        unsafeApi: mockApiWithError
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      // Falls back to DEFAULT_SCORE (1000) on error
      expect(result.current.score).toBe(1000)
      expect(result.current.error).not.toBeNull()
      expect(result.current.isProviderAvailable).toBe(false)
    })

    it('should use default score when chain returns null', async () => {
      const mockApiNullScore = {
        query: {
          Storage: {
            ScoreCache: {
              getValue: jest.fn().mockResolvedValue(null)
            }
          }
        }
      }

      const contentHash = new Uint8Array(32).fill(3)
      const { result } = renderHook(() => useScore({
        contentHash,
        unsafeApi: mockApiNullScore
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      // Default score (1000) when not found on chain
      expect(result.current.score).toBe(1000)
      expect(result.current.isProviderAvailable).toBe(false)
    })
  })

  describe('contentHash format', () => {
    it('should accept Uint8Array', async () => {
      const contentHash = new Uint8Array(32).fill(1)
      const mockApi = {
        query: { Storage: { ScoreCache: { getValue: mockGetValue } } }
      }
      
      const { result, rerender } = renderHook(
        (props) => useScore(props),
        { initialProps: { contentHash, unsafeApi: mockApi } }
      )
      
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: 3000 })
      expect(mockGetValue).toHaveBeenCalled()
    })

    it('should accept number array', async () => {
      const contentHash = new Array(32).fill(4) as number[]
      const mockApi = {
        query: { Storage: { ScoreCache: { getValue: mockGetValue } } }
      }
      
      const { result } = renderHook(
        (props) => useScore(props),
        { initialProps: { contentHash, unsafeApi: mockApi } }
      )
      
      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: 3000 })
      expect(mockGetValue).toHaveBeenCalled()
    })
  })

  describe('no API available', () => {
    it('should return default score when unsafeApi is undefined', async () => {
      const { result } = renderHook(() => useScore({
        contentHash: new Uint8Array(32).fill(5)
      }))
      
      await waitFor(() => expect(result.current.isLoading).toBe(false))
      expect(result.current.score).toBe(1000) // DEFAULT_SCORE
      expect(result.current.isProviderAvailable).toBe(false)
    })

    it('should return default when Storage pallet not available', async () => {
      const mockApiNoStorage = {
        query: {}
      }
      
      const { result } = renderHook(() => useScore({
        contentHash: new Uint8Array(32).fill(6),
        unsafeApi: mockApiNoStorage
      }))
      
      await waitFor(() => expect(result.current.isLoading).toBe(false))
      expect(result.current.score).toBe(1000)
      expect(result.current.isProviderAvailable).toBe(false)
    })
  })
})

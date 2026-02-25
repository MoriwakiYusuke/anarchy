/**
 * useNickname Hook Tests
 * 
 * T-038: useNickname hook unit tests
 * Test-First Development - tests written before implementation
 */

import { renderHook, act, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

/** Timeout for async test operations in milliseconds */
const TEST_TIMEOUT_MS = 5000

// Mock signAndSubmit
const mockSignAndSubmit = jest.fn()

// Mock PAPI query
const mockQueryNickname = jest.fn()

// Mock unsafeApi
const mockUnsafeApi = {
  tx: {
    Nickname: {
      set_nickname: jest.fn().mockReturnValue({
        signAndSubmit: mockSignAndSubmit,
      }),
      clear_nickname: jest.fn().mockReturnValue({
        signAndSubmit: mockSignAndSubmit,
      }),
    },
  },
  query: {
    Nickname: {
      Nicknames: mockQueryNickname,
    },
  },
}

// Mock signer
const mockSigner = {
  publicKey: new Uint8Array(32).fill(1),
  sign: jest.fn(),
} as unknown

// Mock client
const mockClient = {
  getUnsafeApi: jest.fn().mockReturnValue(mockUnsafeApi),
}

// Import after mocks - we'll create this hook
// Using conditional import pattern for TDD
let useNickname: typeof import('@/hooks/useNickname').useNickname
try {
  useNickname = require('@/hooks/useNickname').useNickname
} catch {
  // Hook not implemented yet - define placeholder
  useNickname = () => ({
    nickname: null,
    isLoading: false,
    error: null,
    setNickname: async () => {},
    clearNickname: async () => {},
    refetch: async () => {},
    state: { status: 'idle' },
  })
}

describe('useNickname Hook', () => {
  const TEST_ADDRESS = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY'
  const TEST_NICKNAME = 'alice_anarchy'

  beforeEach(() => {
    jest.clearAllMocks()
    mockSignAndSubmit.mockReset()
    mockQueryNickname.mockReset()
  })

  // ============================================================================
  // T-038a: Query Nickname
  // ============================================================================

  describe('Query Nickname', () => {
    it('should fetch nickname on mount', async () => {
      const encodedNickname = new TextEncoder().encode(TEST_NICKNAME)
      mockQueryNickname.mockResolvedValue(encodedNickname)

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
      }))

      await waitFor(() => {
        expect(result.current.nickname).toBe(TEST_NICKNAME)
      }, { timeout: TEST_TIMEOUT_MS })
    })

    it('should return null when no nickname is set', async () => {
      mockQueryNickname.mockResolvedValue(null)

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: TEST_TIMEOUT_MS })

      expect(result.current.nickname).toBeNull()
    })

    it('should handle query error gracefully', async () => {
      mockQueryNickname.mockRejectedValue(new Error('Query failed'))

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: TEST_TIMEOUT_MS })

      expect(result.current.error).toBe('error.nicknameQueryFailed')
    })

    it('should refetch on accountId change', async () => {
      const encodedNickname = new TextEncoder().encode(TEST_NICKNAME)
      mockQueryNickname.mockResolvedValue(encodedNickname)

      const { result, rerender } = renderHook(
        ({ accountId }) => useNickname({
          client: mockClient,
          unsafeApi: mockUnsafeApi,
          accountId,
        }),
        { initialProps: { accountId: TEST_ADDRESS } }
      )

      await waitFor(() => {
        expect(result.current.nickname).toBe(TEST_NICKNAME)
      }, { timeout: TEST_TIMEOUT_MS })

      // Change account
      const newAddress = '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty'
      const newNickname = 'bob_builder'
      mockQueryNickname.mockResolvedValue(new TextEncoder().encode(newNickname))

      rerender({ accountId: newAddress })

      await waitFor(() => {
        expect(result.current.nickname).toBe(newNickname)
      }, { timeout: TEST_TIMEOUT_MS })
    })
  })

  // ============================================================================
  // T-038b: Set Nickname
  // ============================================================================

  describe('Set Nickname', () => {
    it('should set nickname successfully', async () => {
      mockSignAndSubmit.mockResolvedValue({ txHash: '0x1234' })
      mockQueryNickname.mockResolvedValue(null)

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: mockSigner,
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: TEST_TIMEOUT_MS })

      await act(async () => {
        await result.current.setNickname(TEST_NICKNAME)
      })

      expect(mockUnsafeApi.tx.Nickname.set_nickname).toHaveBeenCalled()
      expect(mockSignAndSubmit).toHaveBeenCalled()
    })

    it('should validate nickname before submitting', async () => {
      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: mockSigner,
      }))

      // Empty nickname should fail
      await act(async () => {
        await result.current.setNickname('')
      })

      expect(result.current.error).toBe('error.nicknameEmpty')
      expect(mockSignAndSubmit).not.toHaveBeenCalled()
    })

    it('should reject nickname exceeding 128 bytes', async () => {
      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: mockSigner,
      }))

      // 150 bytes nickname
      const longNickname = 'あ'.repeat(50) // Each あ is ~3 bytes in UTF-8 = 150 bytes

      await act(async () => {
        await result.current.setNickname(longNickname)
      })

      expect(result.current.error).toBe('error.nicknameTooLong')
      expect(mockSignAndSubmit).not.toHaveBeenCalled()
    })

    it('should handle transaction failure', async () => {
      mockSignAndSubmit.mockRejectedValue(new Error('Transaction rejected'))
      mockQueryNickname.mockResolvedValue(null)

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: mockSigner,
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: TEST_TIMEOUT_MS })

      await act(async () => {
        await result.current.setNickname(TEST_NICKNAME)
      })

      expect(result.current.error).toBe('error.nicknameSetFailed')
    })

    it('should require signer', async () => {
      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: null,
      }))

      await act(async () => {
        await result.current.setNickname(TEST_NICKNAME)
      })

      expect(result.current.error).toBe('error.signerRequired')
      expect(mockSignAndSubmit).not.toHaveBeenCalled()
    })
  })

  // ============================================================================
  // T-038c: Clear Nickname
  // ============================================================================

  describe('Clear Nickname', () => {
    it('should clear nickname successfully', async () => {
      mockSignAndSubmit.mockResolvedValue({ txHash: '0x1234' })
      mockQueryNickname.mockResolvedValue(new TextEncoder().encode(TEST_NICKNAME))

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: mockSigner,
      }))

      await waitFor(() => {
        expect(result.current.nickname).toBe(TEST_NICKNAME)
      }, { timeout: TEST_TIMEOUT_MS })

      await act(async () => {
        await result.current.clearNickname()
      })

      expect(mockUnsafeApi.tx.Nickname.clear_nickname).toHaveBeenCalled()
      expect(mockSignAndSubmit).toHaveBeenCalled()
    })

    it('should handle clear failure', async () => {
      mockSignAndSubmit.mockRejectedValue(new Error('Transaction failed'))
      mockQueryNickname.mockResolvedValue(new TextEncoder().encode(TEST_NICKNAME))

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: mockSigner,
      }))

      await waitFor(() => {
        expect(result.current.nickname).toBe(TEST_NICKNAME)
      }, { timeout: TEST_TIMEOUT_MS })

      await act(async () => {
        await result.current.clearNickname()
      })

      expect(result.current.error).toBe('error.nicknameClearFailed')
    })
  })

  // ============================================================================
  // T-038d: State Machine
  // ============================================================================

  describe('State Machine', () => {
    it('should start in idle state', async () => {
      mockQueryNickname.mockResolvedValue(null)

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: TEST_TIMEOUT_MS })

      expect(result.current.state.status).toBe('idle')
    })

    it('should transition to pending during setNickname', async () => {
      // Use a long-running promise
      let resolveSetNickname: () => void
      mockSignAndSubmit.mockImplementation(() => new Promise(resolve => {
        resolveSetNickname = () => resolve({ txHash: '0x1234' })
      }))
      mockQueryNickname.mockResolvedValue(null)

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: mockSigner,
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: TEST_TIMEOUT_MS })

      // Start the set operation
      act(() => {
        result.current.setNickname(TEST_NICKNAME)
      })

      // Check pending state
      await waitFor(() => {
        expect(result.current.state.status).toBe('pending')
      }, { timeout: TEST_TIMEOUT_MS })

      // Complete the transaction
      await act(async () => {
        resolveSetNickname!()
      })

      await waitFor(() => {
        expect(result.current.state.status).toBe('success')
      }, { timeout: TEST_TIMEOUT_MS })
    })

    it('should transition to error on failure', async () => {
      mockSignAndSubmit.mockRejectedValue(new Error('Failed'))
      mockQueryNickname.mockResolvedValue(null)

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: mockSigner,
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: TEST_TIMEOUT_MS })

      await act(async () => {
        await result.current.setNickname(TEST_NICKNAME)
      })

      expect(result.current.state.status).toBe('error')
    })
  })

  // ============================================================================
  // T-038e: Callbacks
  // ============================================================================

  describe('Callbacks', () => {
    it('should call onSuccess after successful set', async () => {
      mockSignAndSubmit.mockResolvedValue({ txHash: '0x1234' })
      mockQueryNickname.mockResolvedValue(null)
      const onSuccess = jest.fn()

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: mockSigner,
        onSuccess,
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: TEST_TIMEOUT_MS })

      await act(async () => {
        await result.current.setNickname(TEST_NICKNAME)
      })

      expect(onSuccess).toHaveBeenCalledWith(TEST_NICKNAME)
    })

    it('should call onError on failure', async () => {
      mockSignAndSubmit.mockRejectedValue(new Error('Transaction failed'))
      mockQueryNickname.mockResolvedValue(null)
      const onError = jest.fn()

      const { result } = renderHook(() => useNickname({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        accountId: TEST_ADDRESS,
        signer: mockSigner,
        onError,
      }))

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      }, { timeout: TEST_TIMEOUT_MS })

      await act(async () => {
        await result.current.setNickname(TEST_NICKNAME)
      })

      expect(onError).toHaveBeenCalled()
    })
  })
})

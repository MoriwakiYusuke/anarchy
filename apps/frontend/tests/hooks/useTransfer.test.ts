/**
 * useTransfer Hook Tests
 * 
 * T-022: useTransfer hook unit tests
 * Test-First Development - tests written before implementation
 */

import { renderHook, act, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

/** Timeout for async test operations in milliseconds */
const TEST_TIMEOUT_MS = 5000

// Mock polkadot-api
const mockSignAndSubmit = jest.fn()
const mockGetUnsafeApi = jest.fn()

jest.mock('polkadot-api', () => ({
  ...jest.requireActual('polkadot-api'),
}))

// Mock @polkadot/util-crypto
jest.mock('@polkadot/util-crypto', () => ({
  decodeAddress: jest.fn((address: string) => {
    // Valid test addresses start with '5'
    if (address && address.startsWith('5') && address.length >= 47) {
      return new Uint8Array(32).fill(1)
    }
    throw new Error('Invalid address')
  }),
}))

// Import after mocks
import { useTransfer } from '@/hooks/useTransfer'
import { TransferStatus } from '@/types/transfer'

describe('useTransfer Hook', () => {
  const mockSigner = {
    publicKey: new Uint8Array(32).fill(1),
    sign: jest.fn(),
  } as any

  const mockClient = {
    getUnsafeApi: jest.fn(),
    getFinalizedBlock: jest.fn(),
  }

  const mockUnsafeApi = {
    tx: {
      Balances: {
        transfer_keep_alive: jest.fn().mockReturnValue({
          signAndSubmit: mockSignAndSubmit,
        }),
      },
    },
  }

  beforeEach(() => {
    jest.clearAllMocks()
    mockSignAndSubmit.mockReset()
  })

  // ============================================================================
  // T-022a: Initial State
  // ============================================================================

  describe('Initial State', () => {
    it('should initialize with idle status', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
      }))

      expect(result.current.state.status).toBe('idle')
      expect(result.current.state.recipient).toBeUndefined()
      expect(result.current.state.amount).toBeUndefined()
      expect(result.current.state.txHash).toBeUndefined()
      expect(result.current.state.error).toBeUndefined()
    })
  })

  // ============================================================================
  // T-022b: Recipient Validation
  // ============================================================================

  describe('Recipient Validation', () => {
    it('should validate correct SS58 address', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
      }))

      const validation = result.current.validateRecipient('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty')
      expect(validation.valid).toBe(true)
      expect(validation.error).toBeUndefined()
    })

    it('should reject invalid address', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
      }))

      const validation = result.current.validateRecipient('invalid_address')
      expect(validation.valid).toBe(false)
      expect(validation.error).toBe('error.invalidAddressLength')
    })

    it('should reject empty address', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
      }))

      const validation = result.current.validateRecipient('')
      expect(validation.valid).toBe(false)
      expect(validation.error).toBe('error.emptyRecipient')
    })

    it('should reject self-transfer', () => {
      const senderAddress = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY'
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress,
      }))

      const validation = result.current.validateRecipient(senderAddress)
      expect(validation.valid).toBe(false)
      expect(validation.error).toBe('error.selfTransfer')
    })
  })

  // ============================================================================
  // T-022c: Amount Validation
  // ============================================================================

  describe('Amount Validation', () => {
    it('should validate positive amount within balance', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        balance: BigInt(100_000_000_000_000), // 100 MORAL
      }))

      // validateAmount takes a string amount
      const validation = result.current.validateAmount('50')
      
      expect(validation.valid).toBe(true)
      expect(validation.error).toBeUndefined()
    })

    it('should reject zero amount', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        balance: BigInt(100_000_000_000_000),
      }))

      const validation = result.current.validateAmount('0')
      
      expect(validation.valid).toBe(false)
      expect(validation.error).toBe('error.amountTooSmall')
    })

    it('should reject amount exceeding balance', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        balance: BigInt(50_000_000_000_000), // 50 MORAL
      }))

      // Try to transfer 100 MORAL (more than balance)
      const validation = result.current.validateAmount('100')
      
      expect(validation.valid).toBe(false)
      expect(validation.error).toBe('error.amountExceedsBalance')
    })
  })

  // ============================================================================
  // T-022d: Transfer State Transitions
  // ============================================================================

  describe('Transfer State Transitions', () => {
    it('should transition to confirming when transfer() is called', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        balance: BigInt(100_000_000_000_000), // 100 MORAL
      }))

      act(() => {
        result.current.transfer('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty', '1')
      })

      expect(result.current.state.status).toBe('confirming')
      expect(result.current.state.recipient).toBe('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty')
      expect(result.current.state.amount).toBe(BigInt(1_000_000_000_000))
    })

    it('should transition back to idle when cancel() is called', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        balance: BigInt(100_000_000_000_000),
      }))

      // First, move to confirming
      act(() => {
        result.current.transfer('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty', '1')
      })
      expect(result.current.state.status).toBe('confirming')

      // Cancel
      act(() => {
        result.current.cancel()
      })
      expect(result.current.state.status).toBe('idle')
      expect(result.current.state.recipient).toBeUndefined()
      expect(result.current.state.amount).toBeUndefined()
    })

    it('should reset to idle when reset() is called', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        balance: BigInt(100_000_000_000_000),
      }))

      // Move to confirming
      act(() => {
        result.current.transfer('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty', '1')
      })

      // Reset
      act(() => {
        result.current.reset()
      })

      expect(result.current.state.status).toBe('idle')
    })
  })

  // ============================================================================
  // T-022e: Successful Transfer
  // ============================================================================

  describe('Successful Transfer', () => {
    it('should complete transfer and transition to success', async () => {
      // Mock successful transaction
      mockSignAndSubmit.mockResolvedValue({ txHash: '0x1234567890abcdef' })

      const onSuccess = jest.fn()
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        balance: BigInt(100_000_000_000_000),
        onSuccess,
      }))

      // Initiate transfer
      act(() => {
        result.current.transfer('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty', '1')
      })

      // Confirm transfer
      await act(async () => {
        await result.current.confirm()
      })

      await waitFor(() => {
        expect(result.current.state.status).toBe('success')
      }, { timeout: TEST_TIMEOUT_MS })

      expect(result.current.state.txHash).toBeDefined()
      expect(onSuccess).toHaveBeenCalled()
    })
  })

  // ============================================================================
  // T-022f: Failed Transfer
  // ============================================================================

  describe('Failed Transfer', () => {
    it('should transition to error on failed transaction', async () => {
      // Mock failed transaction
      mockSignAndSubmit.mockRejectedValue(new Error('Transaction failed'))

      const onError = jest.fn()
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        balance: BigInt(100_000_000_000_000),
        onError,
      }))

      // Initiate transfer
      act(() => {
        result.current.transfer('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty', '1')
      })

      // Confirm transfer
      await act(async () => {
        await result.current.confirm()
      })

      await waitFor(() => {
        expect(result.current.state.status).toBe('error')
      }, { timeout: TEST_TIMEOUT_MS })

      expect(result.current.state.error).toBeDefined()
      expect(onError).toHaveBeenCalled()
    })
  })

  // ============================================================================
  // T-022g: Edge Cases
  // ============================================================================

  describe('Edge Cases', () => {
    it('should not allow confirm() without prior transfer()', async () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: mockSigner,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
      }))

      // Try to confirm without calling transfer() first
      await act(async () => {
        await result.current.confirm()
      })

      // Should remain in idle state
      expect(result.current.state.status).toBe('idle')
    })

    it('should handle missing signer gracefully', () => {
      const { result } = renderHook(() => useTransfer({
        client: mockClient,
        unsafeApi: mockUnsafeApi,
        signer: null,
        senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        balance: BigInt(100_000_000_000_000),
      }))

      // Should still be able to call transfer
      act(() => {
        result.current.transfer('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty', '1')
      })

      expect(result.current.state.status).toBe('confirming')
    })
  })
})

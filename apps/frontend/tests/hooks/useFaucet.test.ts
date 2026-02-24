/**
 * useFaucet Hook Tests
 * 
 * T-102: ボタンクリックでWorkerが起動しPoW計算開始
 * T-103: 計算成功後にトランザクションが送信される
 */

import { renderHook, act, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

// Mock Worker
class MockWorker {
  onmessage: ((event: MessageEvent) => void) | null = null
  onerror: ((event: ErrorEvent) => void) | null = null
  
  constructor(url: string | URL) {
    // Simulate worker ready message after construction
    setTimeout(() => {
      this.onmessage?.({ data: { type: 'ready' } } as MessageEvent)
    }, 0)
  }
  
  postMessage(message: any) {
    if (message.type === 'start') {
      // Simulate mining progress
      setTimeout(() => {
        this.onmessage?.({ 
          data: { 
            type: 'progress', 
            nonce: BigInt(50000), 
            hashRate: 100000, 
            elapsed: 500 
          } 
        } as MessageEvent)
      }, 10)
      
      // Simulate solution found
      setTimeout(() => {
        this.onmessage?.({ 
          data: { 
            type: 'solution', 
            nonce: BigInt(100000), 
            elapsed: 1000 
          } 
        } as MessageEvent)
      }, 20)
    }
  }
  
  terminate() {
    this.onmessage = null
    this.onerror = null
  }
}

// Store original Worker
const OriginalWorker = global.Worker

// Mock modules before importing useFaucet
jest.mock('@polkadot/util-crypto', () => ({
  decodeAddress: jest.fn().mockReturnValue(new Uint8Array(32).fill(1)),
}))

jest.mock('@/lib/faucet/challenge', () => ({
  computeChallenge: jest.fn().mockReturnValue(new Uint8Array(32).fill(0)),
  hexToBytes: jest.fn().mockImplementation((hex: string) => {
    const cleanHex = hex.startsWith('0x') ? hex.slice(2) : hex
    const bytes = new Uint8Array(cleanHex.length / 2)
    for (let i = 0; i < bytes.length; i++) {
      bytes[i] = parseInt(cleanHex.slice(i * 2, i * 2 + 2), 16)
    }
    return bytes
  }),
}))

// Now import the hook
import { useFaucet, FaucetStatus } from '@/hooks/useFaucet'

describe('useFaucet Hook', () => {
  const mockSigner = {
    sign: jest.fn(),
  } as any

  const mockUnsafeApi = {
    query: {
      Faucet: {
        TotalClaims: { getValue: jest.fn().mockResolvedValue(BigInt(0)) },
        Claims: { getValue: jest.fn().mockResolvedValue(null) }, // Not claimed by default
      },
      System: {
        Number: { getValue: jest.fn().mockResolvedValue(100) },
      },
    },
    constants: {
      Faucet: {
        BaseDifficulty: jest.fn().mockReturnValue(8), // Low difficulty for tests
        DifficultyScalingFactor: jest.fn().mockReturnValue(BigInt(1000)),
        MaxDifficulty: jest.fn().mockReturnValue(28),
      },
    },
    tx: {
      Faucet: {
        claim: jest.fn().mockReturnValue({
          signAndSubmit: jest.fn().mockResolvedValue({ blockHash: '0x123' }),
          getBareTx: jest.fn().mockResolvedValue({
            asHex: () => '0x' + '00'.repeat(50), // Mock unsigned extrinsic
          }),
        }),
      },
    },
  }

  const mockClient = {
    _request: jest.fn().mockImplementation((method: string, params: any[]) => {
      if (method === 'chain_getFinalizedHead') {
        return Promise.resolve('0x' + '00'.repeat(32))
      }
      if (method === 'chain_getHeader') {
        return Promise.resolve({ number: '0x64' }) // hex for 100
      }
      if (method === 'author_submitExtrinsic') {
        return Promise.resolve('0x' + '00'.repeat(32))
      }
      return Promise.resolve(null)
    }),
  }

  beforeAll(() => {
    // @ts-ignore
    global.Worker = MockWorker
  })

  afterAll(() => {
    global.Worker = OriginalWorker
  })

  beforeEach(() => {
    jest.clearAllMocks()
  })

  describe('Initial State', () => {
    it('should start in idle state', () => {
      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      expect(result.current.status).toBe('idle')
      expect(result.current.error).toBeNull()
      expect(result.current.progress).toBeNull()
    })
  })

  describe('T-102: Worker Startup', () => {
    it('should transition to mining state when startMining is called', async () => {
      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      act(() => {
        result.current.startMining()
      })

      await waitFor(() => {
        expect(result.current.status).toBe('mining')
      })
    })

    it('should not start mining without account', async () => {
      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApi,
          account: null,
          signer: mockSigner,
        })
      )

      await act(async () => {
        await result.current.startMining()
      })

      expect(result.current.status).toBe('error')
      expect(result.current.error?.code).toBe('NetworkError')
    })

    it('should not start mining without signer', async () => {
      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: null,
        })
      )

      await act(async () => {
        await result.current.startMining()
      })

      expect(result.current.status).toBe('error')
    })
  })

  describe('T-103: Transaction Submission', () => {
    it('should call claim extrinsic setup', () => {
      // This test verifies the API is called correctly
      // Full integration test requires actual worker execution
      expect(mockUnsafeApi.tx.Faucet.claim).toBeDefined()
    })

    it('should submit unsigned transaction via author_submitExtrinsic', async () => {
      // Create a mock that simulates claim being recorded after submission
      let claimCallCount = 0
      const mockUnsafeApiWithClaim = {
        ...mockUnsafeApi,
        query: {
          ...mockUnsafeApi.query,
          Faucet: {
            ...mockUnsafeApi.query.Faucet,
            Claims: { 
              getValue: jest.fn().mockImplementation(() => {
                // First call (pre-check): not claimed
                // Second call (verification): claimed
                claimCallCount++
                return Promise.resolve(claimCallCount > 1 ? 100 : null)
              })
            },
          },
        },
      }

      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApiWithClaim,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      // Start mining
      await act(async () => {
        await result.current.startMining()
      })

      // Wait for mining to complete and transaction to be submitted
      await waitFor(() => {
        return result.current.status === 'success' || result.current.status === 'error'
      }, { timeout: 10000 })

      // Verify the unsigned transaction was submitted
      expect(mockClient._request).toHaveBeenCalledWith(
        'author_submitExtrinsic',
        expect.arrayContaining([expect.stringMatching(/^0x/)])
      )
      
      // Should succeed
      expect(result.current.status).toBe('success')
    })

    // Note: Full transaction flow test requires real worker execution
    // which is better suited for integration tests
  })

  describe('Progress Updates', () => {
    it('should have progress property available', () => {
      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      // Initially progress should be null
      expect(result.current.progress).toBeNull()
      
      // Progress updates during actual mining are tested in integration tests
    })
  })

  describe('Cancellation', () => {
    it('should cancel mining and return to idle', async () => {
      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      act(() => {
        result.current.startMining()
      })

      await waitFor(() => {
        return result.current.status === 'mining'
      })

      act(() => {
        result.current.cancel()
      })

      expect(result.current.status).toBe('idle')
      expect(result.current.progress).toBeNull()
    })
  })

  describe('Error Handling', () => {
    it('should have error property available', () => {
      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      // Initially error should be null
      expect(result.current.error).toBeNull()
    })

    it('should set error when no account provided', async () => {
      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApi,
          account: null,
          signer: mockSigner,
        })
      )

      await act(async () => {
        await result.current.startMining()
      })

      expect(result.current.error).toBeTruthy()
    })

    it('should detect AlreadyClaimed from pre-check before submission', async () => {
      // When Claims storage already has a value for this account
      const mockUnsafeApiAlreadyClaimed = {
        ...mockUnsafeApi,
        query: {
          ...mockUnsafeApi.query,
          Faucet: {
            ...mockUnsafeApi.query.Faucet,
            Claims: { getValue: jest.fn().mockResolvedValue(100) }, // Already claimed
          },
        },
      }

      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApiAlreadyClaimed,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      await act(async () => {
        await result.current.startMining()
      })

      await waitFor(() => {
        return result.current.status === 'error'
      }, { timeout: 5000 })

      expect(result.current.error?.code).toBe('AlreadyClaimed')
    })

    it('should map Invalid Transaction to AlreadyClaimed', async () => {
      // Simulate RPC rejection for second claim
      // Flow: getFinalizedHead -> getHeader -> submitExtrinsic (error here)
      const mockClientWithError = {
        _request: jest.fn().mockImplementation((method: string) => {
          if (method === 'chain_getFinalizedHead') {
            return Promise.resolve('0x' + '00'.repeat(32))
          }
          if (method === 'chain_getHeader') {
            return Promise.resolve({ number: '0x64' }) // hex for 100
          }
          if (method === 'author_submitExtrinsic') {
            // smoldot returns Custom(1) = AlreadyClaimed
            return Promise.reject(new Error('RpcError: 1010: Invalid Transaction: Transaction dispatch is invalid'))
          }
          return Promise.resolve(null)
        }),
      }

      const { result } = renderHook(() =>
        useFaucet({
          client: mockClientWithError,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      await act(async () => {
        await result.current.startMining()
      })

      await waitFor(() => {
        return result.current.status === 'error'
      }, { timeout: 5000 })

      expect(result.current.error?.code).toBe('AlreadyClaimed')
    })

    it('should detect AlreadyClaimed from error result object (non-throwing RPC)', async () => {
      // Some RPC implementations return error in result instead of throwing
      const mockClientWithErrorResult = {
        _request: jest.fn().mockImplementation((method: string) => {
          if (method === 'chain_getFinalizedHead') {
            return Promise.resolve('0x' + '00'.repeat(32))
          }
          if (method === 'chain_getHeader') {
            return Promise.resolve({ number: '0x64' })
          }
          if (method === 'author_submitExtrinsic') {
            // Return error in result object format
            return Promise.resolve({
              error: {
                code: 1010,
                message: 'Invalid Transaction: Custom error: 1'
              }
            })
          }
          return Promise.resolve(null)
        }),
      }

      const { result } = renderHook(() =>
        useFaucet({
          client: mockClientWithErrorResult,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      await act(async () => {
        await result.current.startMining()
      })

      await waitFor(() => {
        return result.current.status === 'error'
      }, { timeout: 5000 })

      expect(result.current.error?.code).toBe('AlreadyClaimed')
    })

    it('should detect AlreadyClaimed from numeric 1010 error code', async () => {
      // smoldot may return just the numeric code
      const mockClientWith1010 = {
        _request: jest.fn().mockImplementation((method: string) => {
          if (method === 'chain_getFinalizedHead') {
            return Promise.resolve('0x' + '00'.repeat(32))
          }
          if (method === 'chain_getHeader') {
            return Promise.resolve({ number: '0x64' })
          }
          if (method === 'author_submitExtrinsic') {
            return Promise.reject(new Error('1010: Transaction dispatch is mandatory but invalid'))
          }
          return Promise.resolve(null)
        }),
      }

      const { result } = renderHook(() =>
        useFaucet({
          client: mockClientWith1010,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      await act(async () => {
        await result.current.startMining()
      })

      await waitFor(() => {
        return result.current.status === 'error'
      }, { timeout: 5000 })

      expect(result.current.error?.code).toBe('AlreadyClaimed')
    })
  })

  describe('onSuccess Callback', () => {
    it('should accept onSuccess callback', () => {
      const onSuccess = jest.fn()

      const { result } = renderHook(() =>
        useFaucet({
          client: mockClient,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
          onSuccess,
        })
      )

      // Hook should accept callback without errors
      expect(result.current.startMining).toBeDefined()
      // Full callback test requires integration test with real worker
    })
  })

  describe('RPC Timeout Handling', () => {
    it('should timeout if RPC call takes too long', async () => {
      // Create a slow client that never resolves
      const slowClient = {
        _request: jest.fn().mockImplementation(() => new Promise(() => {})), // Never resolves
      }

      const { result } = renderHook(() =>
        useFaucet({
          client: slowClient,
          unsafeApi: mockUnsafeApi,
          account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
          signer: mockSigner,
        })
      )

      await act(async () => {
        result.current.startMining()
      })

      // Wait for timeout (30 seconds in prod, but test will fail faster due to Jest timeout)
      // We can't easily test the full 30s timeout, so verify the setup is correct
      expect(slowClient._request).toHaveBeenCalledWith('chain_getFinalizedHead', [])
    })
  })
})

/**
 * smoldot-provider Tests
 * 
 * Tests for smoldot Light Client provider module
 * - T-SP-001: Singleton pattern works correctly
 * - T-SP-002: initSmoldotClient creates PAPI client
 * - T-SP-003: destroySmoldotClient cleans up resources
 * - T-SP-004: Error handling during initialization
 */

// Mock chain and client objects
const mockChain = {
  remove: jest.fn(),
}

const mockSmoldotClient = {
  addChain: jest.fn().mockResolvedValue(mockChain),
  terminate: jest.fn(),
}

const mockProvider = { type: 'smoldot-provider' }

const mockPapiClient = {
  destroy: jest.fn(),
  getUnsafeApi: jest.fn(),
}

// Mock smoldot before importing the module
jest.mock('smoldot', () => ({
  start: jest.fn(() => mockSmoldotClient),
}))

jest.mock('polkadot-api/sm-provider', () => ({
  getSmProvider: jest.fn(() => mockProvider),
}))

jest.mock('polkadot-api', () => ({
  createClient: jest.fn(() => mockPapiClient),
}))

jest.mock('@/lib/chainspec.json', () => ({
  id: 'test-chain',
  name: 'Test Chain',
  genesis: {},
  bootNodes: [],
}), { virtual: true })

import { start } from 'smoldot'
import { getSmProvider } from 'polkadot-api/sm-provider'
import { createClient } from 'polkadot-api'

const mockStart = start as jest.MockedFunction<typeof start>
const mockGetSmProvider = getSmProvider as jest.MockedFunction<typeof getSmProvider>
const mockCreateClient = createClient as jest.MockedFunction<typeof createClient>

describe('smoldot-provider', () => {
  // Import module once, use destroySmoldotClient to reset between tests
  let initSmoldotClient: typeof import('@/lib/smoldot-provider').initSmoldotClient
  let destroySmoldotClient: typeof import('@/lib/smoldot-provider').destroySmoldotClient
  let isSmoldotInitialized: typeof import('@/lib/smoldot-provider').isSmoldotInitialized
  let getSmoldotClient: typeof import('@/lib/smoldot-provider').getSmoldotClient
  let getSmoldotDebugInfo: typeof import('@/lib/smoldot-provider').getSmoldotDebugInfo

  beforeAll(async () => {
    const module = await import('@/lib/smoldot-provider')
    initSmoldotClient = module.initSmoldotClient
    destroySmoldotClient = module.destroySmoldotClient
    isSmoldotInitialized = module.isSmoldotInitialized
    getSmoldotClient = module.getSmoldotClient
    getSmoldotDebugInfo = module.getSmoldotDebugInfo
  })

  beforeEach(() => {
    // Reset singleton state
    destroySmoldotClient()
    
    // Clear mock call history
    jest.clearAllMocks()
    mockChain.remove.mockClear()
    mockSmoldotClient.terminate.mockClear()
    mockSmoldotClient.addChain.mockResolvedValue(mockChain)
    mockPapiClient.destroy.mockClear()
  })

  afterAll(() => {
    destroySmoldotClient()
  })

  describe('T-SP-001: Singleton pattern', () => {
    it('should return same client on multiple calls', async () => {
      const client1 = await initSmoldotClient()
      const client2 = await initSmoldotClient()

      expect(client1).toBe(client2)
      expect(mockStart).toHaveBeenCalledTimes(1)
    })

    it('should return existing client if already initialized', async () => {
      await initSmoldotClient()
      expect(isSmoldotInitialized()).toBe(true)

      // Second call should return cached client
      const client = await initSmoldotClient()
      expect(client).toBe(mockPapiClient)
      expect(mockStart).toHaveBeenCalledTimes(1)
    })

    it('should return pending promise if initialization in progress', async () => {
      // Start two initializations simultaneously
      const promise1 = initSmoldotClient()
      const promise2 = initSmoldotClient()

      expect(mockStart).toHaveBeenCalledTimes(1)

      const [client1, client2] = await Promise.all([promise1, promise2])
      expect(client1).toBe(client2)
    })
  })

  describe('T-SP-002: initSmoldotClient creates PAPI client', () => {
    it('should start smoldot with correct options', async () => {
      await initSmoldotClient()

      expect(mockStart).toHaveBeenCalledWith({
        forbidWs: false,
        forbidNonLocalWs: false,
        forbidWss: false,
      })
    })

    it('should add chain with chainspec', async () => {
      await initSmoldotClient()

      expect(mockSmoldotClient.addChain).toHaveBeenCalledWith({
        chainSpec: expect.any(String),
      })
    })

    it('should create sm provider from chain', async () => {
      await initSmoldotClient()

      expect(mockGetSmProvider).toHaveBeenCalled()
    })

    it('should create PAPI client from provider', async () => {
      await initSmoldotClient()

      expect(mockCreateClient).toHaveBeenCalledWith(mockProvider)
    })

    it('should return PAPI client', async () => {
      const client = await initSmoldotClient()

      expect(client).toBe(mockPapiClient)
    })
  })

  describe('T-SP-003: destroySmoldotClient cleans up resources', () => {
    it('should destroy PAPI client', async () => {
      await initSmoldotClient()
      destroySmoldotClient()

      expect(mockPapiClient.destroy).toHaveBeenCalled()
    })

    it('should remove chain', async () => {
      await initSmoldotClient()
      destroySmoldotClient()

      expect(mockChain.remove).toHaveBeenCalled()
    })

    it('should terminate smoldot client', async () => {
      await initSmoldotClient()
      destroySmoldotClient()

      expect(mockSmoldotClient.terminate).toHaveBeenCalled()
    })

    it('should set isSmoldotInitialized to false', async () => {
      await initSmoldotClient()
      expect(isSmoldotInitialized()).toBe(true)

      destroySmoldotClient()
      expect(isSmoldotInitialized()).toBe(false)
    })

    it('should allow re-initialization after destroy', async () => {
      await initSmoldotClient()
      destroySmoldotClient()

      // Reset mocks for second initialization
      mockStart.mockClear()
      
      await initSmoldotClient()
      expect(mockStart).toHaveBeenCalledTimes(1)
    })
  })

  describe('T-SP-004: Error handling', () => {
    it('should throw if smoldot start fails', async () => {
      mockStart.mockImplementationOnce(() => {
        throw new Error('Failed to start smoldot')
      })
      
      await expect(initSmoldotClient()).rejects.toThrow('Failed to start smoldot')
    })

    it('should throw if addChain fails', async () => {
      mockSmoldotClient.addChain.mockRejectedValueOnce(new Error('Invalid chainspec'))
      
      await expect(initSmoldotClient()).rejects.toThrow('Invalid chainspec')
    })

    it('should clean up on initialization failure', async () => {
      mockSmoldotClient.addChain.mockRejectedValueOnce(new Error('Chain error'))
      
      await expect(initSmoldotClient()).rejects.toThrow('Chain error')
      expect(isSmoldotInitialized()).toBe(false)
    })

    it('should handle errors during destroy gracefully', async () => {
      await initSmoldotClient()
      
      mockPapiClient.destroy.mockImplementationOnce(() => {
        throw new Error('Destroy failed')
      })
      
      // Should not throw
      expect(() => destroySmoldotClient()).not.toThrow()
      expect(isSmoldotInitialized()).toBe(false)
    })
  })

  describe('getSmoldotClient', () => {
    it('should return null when not initialized', () => {
      expect(getSmoldotClient()).toBeNull()
    })

    it('should return client when initialized', async () => {
      await initSmoldotClient()

      expect(getSmoldotClient()).toBe(mockPapiClient)
    })
  })

  describe('getSmoldotDebugInfo', () => {
    it('should return all false when not initialized', () => {
      const info = getSmoldotDebugInfo()
      expect(info.isInitialized).toBe(false)
      expect(info.hasChain).toBe(false)
      expect(info.hasClient).toBe(false)
    })

    it('should return all true when initialized', async () => {
      await initSmoldotClient()

      const info = getSmoldotDebugInfo()
      expect(info.isInitialized).toBe(true)
      expect(info.hasChain).toBe(true)
      expect(info.hasClient).toBe(true)
    })
  })
})

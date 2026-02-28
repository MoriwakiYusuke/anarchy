/**
 * ReactionService Tests
 * 
 * T051: [US3] Jest test: reactionService fetches current difficulty
 * T058: [US3] Fetch current difficulty before mining in reactionService.ts
 * 
 * Feature: 017-reaction-mining
 */

import { describe, test, expect, jest, beforeEach } from '@jest/globals'

// Mock blakejs
jest.mock('blakejs', () => ({
  blake2b: jest.fn().mockReturnValue(new Uint8Array(32).fill(0xab)),
}))

// Mock @polkadot-api/substrate-bindings
jest.mock('@polkadot-api/substrate-bindings', () => ({
  getSs58AddressInfo: jest.fn().mockReturnValue({
    isValid: true,
    publicKey: new Uint8Array(32).fill(1),
  }),
}))

// Import after mocks
import { getReactionChallenge, ReactionType } from '@/services/reactionService'

describe('ReactionService', () => {
  describe('getReactionChallenge', () => {
    const mockClient = {
      _request: jest.fn(),
    }

    const mockUnsafeApi = {
      query: {
        Reaction: {
          CurrentDifficulty: {
            getValue: jest.fn(),
          },
        },
      },
    }

    beforeEach(() => {
      jest.clearAllMocks()
      // Default mock implementations
      mockClient._request.mockImplementation((method: string) => {
        if (method === 'chain_getFinalizedHead') {
          return Promise.resolve('0x1234567890abcdef')
        }
        if (method === 'chain_getHeader') {
          return Promise.resolve({ number: '0x64' }) // Block 100
        }
        return Promise.reject(new Error(`Unknown method: ${method}`))
      })
    })

    /**
     * T051: reactionService fetches current difficulty
     */
    test('should fetch current difficulty from chain', async () => {
      const expectedDifficulty = 12

      mockUnsafeApi.query.Reaction.CurrentDifficulty.getValue.mockResolvedValue(
        expectedDifficulty
      )

      const result = await getReactionChallenge(
        mockClient,
        mockUnsafeApi,
        BigInt(1),
        '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY'
      )

      expect(mockUnsafeApi.query.Reaction.CurrentDifficulty.getValue).toHaveBeenCalled()
      expect(result.difficulty).toBe(expectedDifficulty)
    })

    test('should use default difficulty when query returns null', async () => {
      mockUnsafeApi.query.Reaction.CurrentDifficulty.getValue.mockResolvedValue(null)

      const result = await getReactionChallenge(
        mockClient,
        mockUnsafeApi,
        BigInt(1),
        '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY'
      )

      // Should fallback to default (16)
      expect(result.difficulty).toBe(16)
    })

    test('should return challenge, blockNumber and difficulty', async () => {
      mockUnsafeApi.query.Reaction.CurrentDifficulty.getValue.mockResolvedValue(8)

      const result = await getReactionChallenge(
        mockClient,
        mockUnsafeApi,
        BigInt(42),
        '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY'
      )

      expect(result).toHaveProperty('challenge')
      expect(result).toHaveProperty('blockNumber')
      expect(result).toHaveProperty('difficulty')
      expect(result.challenge).toBeInstanceOf(Uint8Array)
      expect(result.challenge.length).toBe(32)
      expect(result.blockNumber).toBe(100) // 0x64 = 100
      expect(result.difficulty).toBe(8)
    })

    test('should throw on failed finalized head fetch', async () => {
      mockClient._request.mockImplementation((method: string) => {
        if (method === 'chain_getFinalizedHead') {
          return Promise.resolve(null)
        }
        return Promise.reject(new Error('Unknown method'))
      })

      await expect(
        getReactionChallenge(
          mockClient,
          mockUnsafeApi,
          BigInt(1),
          '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY'
        )
      ).rejects.toThrow('Failed to get finalized head')
    })

    test('should throw on failed header fetch', async () => {
      mockClient._request.mockImplementation((method: string) => {
        if (method === 'chain_getFinalizedHead') {
          return Promise.resolve('0x1234')
        }
        if (method === 'chain_getHeader') {
          return Promise.resolve(null)
        }
        return Promise.reject(new Error('Unknown method'))
      })

      await expect(
        getReactionChallenge(
          mockClient,
          mockUnsafeApi,
          BigInt(1),
          '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY'
        )
      ).rejects.toThrow('Failed to get block header')
    })
  })

  describe('ReactionType enum', () => {
    test('should have correct values', () => {
      expect(ReactionType.Like).toBe('Like')
      expect(ReactionType.Boost).toBe('Boost')
      expect(ReactionType.Bad).toBe('Bad')
    })
  })
})

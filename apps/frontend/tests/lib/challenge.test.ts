/**
 * Challenge Utility Tests
 * 
 * Tests for PoW challenge computation and verification
 */

import {
  computeChallenge,
  computePoWHash,
  countLeadingZeroBits,
  verifyProof,
  hexToBytes,
  bytesToHex,
} from '@/lib/faucet/challenge'

describe('Challenge Utilities', () => {
  describe('countLeadingZeroBits', () => {
    it('should count 8 zeros for a zero byte', () => {
      const hash = new Uint8Array([0, 0xFF, 0, 0])
      expect(countLeadingZeroBits(hash)).toBe(8)
    })

    it('should count 16 zeros for two zero bytes', () => {
      const hash = new Uint8Array([0, 0, 0xFF, 0])
      expect(countLeadingZeroBits(hash)).toBe(16)
    })

    it('should count partial zeros in non-zero byte', () => {
      // 0x0F = 0b00001111 = 4 leading zeros
      const hash = new Uint8Array([0x0F, 0xFF, 0xFF, 0xFF])
      expect(countLeadingZeroBits(hash)).toBe(4)
    })

    it('should count 1 zero for 0x7F', () => {
      // 0x7F = 0b01111111 = 1 leading zero
      const hash = new Uint8Array([0x7F, 0xFF, 0xFF, 0xFF])
      expect(countLeadingZeroBits(hash)).toBe(1)
    })

    it('should count 0 zeros for 0xFF', () => {
      const hash = new Uint8Array([0xFF, 0x00, 0x00, 0x00])
      expect(countLeadingZeroBits(hash)).toBe(0)
    })

    it('should count all zeros for all-zero array', () => {
      const hash = new Uint8Array(32).fill(0)
      expect(countLeadingZeroBits(hash)).toBe(256) // 32 * 8
    })

    it('should handle 0x01 correctly', () => {
      // 0x01 = 0b00000001 = 7 leading zeros
      const hash = new Uint8Array([0x01, 0xFF, 0xFF, 0xFF])
      expect(countLeadingZeroBits(hash)).toBe(7)
    })
  })

  describe('hexToBytes', () => {
    it('should convert hex string to bytes', () => {
      const hex = '0x0102030405'
      const bytes = hexToBytes(hex)
      expect(bytes).toEqual(new Uint8Array([1, 2, 3, 4, 5]))
    })

    it('should handle hex string without 0x prefix', () => {
      const hex = '0102030405'
      const bytes = hexToBytes(hex)
      expect(bytes).toEqual(new Uint8Array([1, 2, 3, 4, 5]))
    })

    it('should convert 32-byte block hash', () => {
      const hex = '0x' + '00'.repeat(32)
      const bytes = hexToBytes(hex)
      expect(bytes.length).toBe(32)
      expect(bytes.every((b) => b === 0)).toBe(true)
    })
  })

  describe('bytesToHex', () => {
    it('should convert bytes to hex string', () => {
      const bytes = new Uint8Array([1, 2, 3, 4, 5])
      const hex = bytesToHex(bytes)
      expect(hex).toBe('0x0102030405')
    })

    it('should handle zero bytes', () => {
      const bytes = new Uint8Array([0, 0, 0])
      const hex = bytesToHex(bytes)
      expect(hex).toBe('0x000000')
    })

    it('should be inverse of hexToBytes', () => {
      const original = '0xff00aabb'
      const bytes = hexToBytes(original)
      const result = bytesToHex(bytes)
      expect(result).toBe(original)
    })
  })

  describe('computeChallenge', () => {
    it('should produce 32-byte output', () => {
      const blockHash = new Uint8Array(32).fill(1)
      const accountId = new Uint8Array(32).fill(2)
      const challenge = computeChallenge(blockHash, accountId)
      expect(challenge.length).toBe(32)
    })

    it('should produce different output for different inputs', () => {
      const blockHash1 = new Uint8Array(32).fill(1)
      const blockHash2 = new Uint8Array(32).fill(2)
      const accountId = new Uint8Array(32).fill(3)
      
      const challenge1 = computeChallenge(blockHash1, accountId)
      const challenge2 = computeChallenge(blockHash2, accountId)
      
      expect(challenge1).not.toEqual(challenge2)
    })

    it('should produce same output for same inputs', () => {
      const blockHash = new Uint8Array(32).fill(1)
      const accountId = new Uint8Array(32).fill(2)
      
      const challenge1 = computeChallenge(blockHash, accountId)
      const challenge2 = computeChallenge(blockHash, accountId)
      
      expect(challenge1).toEqual(challenge2)
    })
  })

  describe('computePoWHash', () => {
    it('should produce 32-byte output', () => {
      const challenge = new Uint8Array(32).fill(1)
      const nonce = BigInt(12345)
      const hash = computePoWHash(challenge, nonce)
      expect(hash.length).toBe(32)
    })

    it('should produce different output for different nonces', () => {
      const challenge = new Uint8Array(32).fill(1)
      const hash1 = computePoWHash(challenge, BigInt(1))
      const hash2 = computePoWHash(challenge, BigInt(2))
      expect(hash1).not.toEqual(hash2)
    })
  })

  describe('verifyProof', () => {
    it('should verify proof with enough leading zeros', () => {
      // Find a valid nonce with low difficulty for testing
      const challenge = new Uint8Array(32).fill(1)
      let nonce = BigInt(0)
      const difficulty = 4 // Low difficulty for fast test
      
      // Search for valid nonce (should find quickly with low difficulty)
      for (let i = 0; i < 10000; i++) {
        if (verifyProof(challenge, BigInt(i), difficulty)) {
          nonce = BigInt(i)
          break
        }
      }
      
      expect(verifyProof(challenge, nonce, difficulty)).toBe(true)
    })

    it('should reject proof without enough leading zeros', () => {
      const challenge = new Uint8Array(32).fill(1)
      // Very high difficulty - extremely unlikely to find valid nonce
      const difficulty = 256
      
      // Test first few nonces - should all fail
      for (let i = 0; i < 100; i++) {
        expect(verifyProof(challenge, BigInt(i), difficulty)).toBe(false)
      }
    })

    it('should accept difficulty 0 for any nonce', () => {
      const challenge = new Uint8Array(32).fill(1)
      // Difficulty 0 means no zeros required
      expect(verifyProof(challenge, BigInt(0), 0)).toBe(true)
      expect(verifyProof(challenge, BigInt(12345), 0)).toBe(true)
    })
  })
})

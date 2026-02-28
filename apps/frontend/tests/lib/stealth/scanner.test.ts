/**
 * T051: Jest tests for StealthScanner class
 *
 * Tests block scanning functionality for stealth payment detection
 */

import { StealthScanner, ScanResult } from '@/lib/stealth/scanner';
import type { ScanProgress, EphemeralKeyEntry } from '@/lib/stealth/types';

// Define test pubkeys with first byte matching for detection logic
const pubkey1 = new Uint8Array([1, ...new Array(31).fill(0)]);
const pubkey2 = new Uint8Array([2, ...new Array(31).fill(0)]);

// Mock wasm module
jest.mock('anarchy-wasm-engine', () => ({
  // scan_transaction(view_key, ephemeral_pubkey, stealth_pubkey, spend_pubkey) -> bool
  scan_transaction: jest.fn((viewKey: Uint8Array, ephemeralPubkey: Uint8Array, _stealthPubkey: Uint8Array, _spendPubkey: Uint8Array) => {
    // Mock: return true if first byte of ephemeral key matches view key
    return ephemeralPubkey[0] === viewKey[0];
  }),
}));

// Mock getSs58AddressInfo to decode test addresses
jest.mock('@polkadot-api/substrate-bindings', () => ({
  getSs58AddressInfo: jest.fn((address: string) => {
    // Return valid decoded info for test addresses
    if (address === 'test_addr_1') {
      return { isValid: true, publicKey: new Uint8Array([1, ...new Array(31).fill(0)]) };
    }
    if (address === 'test_addr_2') {
      return { isValid: true, publicKey: new Uint8Array([2, ...new Array(31).fill(0)]) };
    }
    return { isValid: false };
  }),
}));

// Mock API module
jest.mock('@/lib/stealth/api', () => ({
  getEphemeralKeys: jest.fn(async (_api: unknown, blockNumber: number) => {
    // Return mock ephemeral keys for specific blocks (using camelCase as per EphemeralKeyEntry interface)
    if (blockNumber === 100) {
      return [
        { ephemeralPubkey: new Uint8Array([1, ...new Array(31).fill(0)]), stealthAddress: 'test_addr_1', blockNumber: 100 },
      ];
    }
    if (blockNumber === 105) {
      return [
        { ephemeralPubkey: new Uint8Array([2, ...new Array(31).fill(0)]), stealthAddress: 'test_addr_2', blockNumber: 105 },
      ];
    }
    return null;
  }),
}));

describe('StealthScanner', () => {
  const mockViewKey = new Uint8Array([1, ...new Array(31).fill(0)]);
  const mockSpendPubkey = new Uint8Array([2, ...new Array(31).fill(0)]);
  const mockApi = { query: {} };

  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('constructor', () => {
    it('should initialize with view key and spend pubkey', () => {
      const scanner = new StealthScanner(mockViewKey, mockSpendPubkey, mockApi);
      expect(scanner).toBeDefined();
    });
  });

  describe('scanBlockRange', () => {
    it('should scan a range of blocks', async () => {
      const scanner = new StealthScanner(mockViewKey, mockSpendPubkey, mockApi);
      const progressUpdates: ScanProgress[] = [];

      const results = await scanner.scanBlockRange(
        100,
        105,
        (progress) => progressUpdates.push(progress)
      );

      // Should have scanned 6 blocks (100-105 inclusive)
      expect(results).toBeDefined();
      expect(progressUpdates.length).toBeGreaterThan(0);
      expect(progressUpdates[progressUpdates.length - 1].currentBlock).toBe(105);
    });

    it('should detect owned transactions', async () => {
      const scanner = new StealthScanner(mockViewKey, mockSpendPubkey, mockApi);
      const results = await scanner.scanBlockRange(100, 105);

      // Block 100 has ephemeral key with first byte = 1 matching view key
      const ownedResults = results.filter(r => r.isOwned);
      expect(ownedResults.length).toBe(1);
      expect(ownedResults[0].blockNumber).toBe(100);
    });

    it('should skip empty blocks', async () => {
      const scanner = new StealthScanner(mockViewKey, mockSpendPubkey, mockApi);
      const results = await scanner.scanBlockRange(101, 103);

      // Blocks 101-103 have no ephemeral keys
      expect(results.length).toBe(0);
    });

    it('should report progress correctly', async () => {
      const scanner = new StealthScanner(mockViewKey, mockSpendPubkey, mockApi);
      const progressUpdates: ScanProgress[] = [];

      await scanner.scanBlockRange(100, 104, (progress) => {
        progressUpdates.push(progress);
      });

      // Check progress updates (ScanProgress has: currentBlock, targetBlock, percentage, detectedCount)
      expect(progressUpdates[0].targetBlock).toBe(104);
      expect(progressUpdates[progressUpdates.length - 1].percentage).toBe(100);
    });
  });

  describe('stop', () => {
    it('should stop scanning when requested', async () => {
      const scanner = new StealthScanner(mockViewKey, mockSpendPubkey, mockApi);

      // Start scanning a large range
      const scanPromise = scanner.scanBlockRange(1, 1000);

      // Stop after a short delay
      setTimeout(() => scanner.stop(), 10);

      const results = await scanPromise;

      // Should have stopped before scanning all blocks
      expect(results.length).toBeLessThan(1000);
    });
  });
});

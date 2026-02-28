/**
 * Coin Selection Algorithm Tests
 * 
 * T063 [US4] Jest test: coin selection algorithm
 */

import { 
  selectCoins, 
  SelectionResult, 
  CoinSelectionOptions,
  LINKABILITY_WARNING_THRESHOLD 
} from '../../../src/lib/stealth/coinSelection';
import { DetectedStealthBalance } from '../../../src/lib/stealth/types';

describe('coinSelection', () => {
  // テストヘルパー: バランスを作成
  const createBalance = (
    address: string, 
    balance: bigint, 
    spent: boolean = false
  ): DetectedStealthBalance => ({
    stealthAddress: address,
    balance,
    receivedAt: 100,
    txHash: new Uint8Array(32),
    spent,
    ephemeralPubkey: new Uint8Array(32),
  });

  describe('selectCoins', () => {
    it('should select single coin exactly matching amount', () => {
      const balances: DetectedStealthBalance[] = [
        createBalance('addr1', BigInt(100_000_000_000_000)),
      ];

      const result = selectCoins(balances, BigInt(100_000_000_000_000));

      expect(result.selected.length).toBe(1);
      expect(result.selected[0].stealthAddress).toBe('addr1');
      expect(result.total).toBe(BigInt(100_000_000_000_000));
      expect(result.change).toBe(BigInt(0));
      expect(result.linkabilityWarning).toBe(false);
    });

    it('should select smallest sufficient coin', () => {
      const balances: DetectedStealthBalance[] = [
        createBalance('addr1', BigInt(50_000_000_000_000)),
        createBalance('addr2', BigInt(100_000_000_000_000)),
        createBalance('addr3', BigInt(150_000_000_000_000)),
      ];

      const result = selectCoins(balances, BigInt(80_000_000_000_000));

      expect(result.selected.length).toBe(1);
      expect(result.selected[0].stealthAddress).toBe('addr2');
      expect(result.total).toBe(BigInt(100_000_000_000_000));
      expect(result.change).toBe(BigInt(20_000_000_000_000));
    });

    it('should combine multiple coins when needed', () => {
      const balances: DetectedStealthBalance[] = [
        createBalance('addr1', BigInt(30_000_000_000_000)),
        createBalance('addr2', BigInt(40_000_000_000_000)),
        createBalance('addr3', BigInt(50_000_000_000_000)),
      ];

      const result = selectCoins(balances, BigInt(100_000_000_000_000));

      expect(result.selected.length).toBe(3);
      expect(result.total).toBe(BigInt(120_000_000_000_000));
      expect(result.change).toBe(BigInt(20_000_000_000_000));
    });

    it('should return linkability warning when using multiple inputs', () => {
      const balances: DetectedStealthBalance[] = [
        createBalance('addr1', BigInt(30_000_000_000_000)),
        createBalance('addr2', BigInt(40_000_000_000_000)),
      ];

      const result = selectCoins(balances, BigInt(60_000_000_000_000));

      expect(result.selected.length).toBe(2);
      expect(result.linkabilityWarning).toBe(true);
    });

    it('should skip spent coins', () => {
      const balances: DetectedStealthBalance[] = [
        createBalance('addr1', BigInt(100_000_000_000_000), true), // spent
        createBalance('addr2', BigInt(50_000_000_000_000), false),
      ];

      const result = selectCoins(balances, BigInt(50_000_000_000_000));

      expect(result.selected.length).toBe(1);
      expect(result.selected[0].stealthAddress).toBe('addr2');
    });

    it('should return empty selection if insufficient funds', () => {
      const balances: DetectedStealthBalance[] = [
        createBalance('addr1', BigInt(30_000_000_000_000)),
        createBalance('addr2', BigInt(40_000_000_000_000)),
      ];

      const result = selectCoins(balances, BigInt(100_000_000_000_000));

      expect(result.selected.length).toBe(0);
      expect(result.sufficient).toBe(false);
    });

    it('should handle empty balance list', () => {
      const result = selectCoins([], BigInt(100_000_000_000_000));

      expect(result.selected.length).toBe(0);
      expect(result.sufficient).toBe(false);
    });

    it('should handle zero amount request', () => {
      const balances: DetectedStealthBalance[] = [
        createBalance('addr1', BigInt(100_000_000_000_000)),
      ];

      const result = selectCoins(balances, BigInt(0));

      expect(result.selected.length).toBe(0);
      expect(result.sufficient).toBe(true);
      expect(result.total).toBe(BigInt(0));
    });
  });

  describe('selectCoins with fee consideration', () => {
    it('should account for tx fee in selection', () => {
      const balances: DetectedStealthBalance[] = [
        createBalance('addr1', BigInt(100_000_000_000_000)),
      ];

      const fee = BigInt(1_000_000_000_000); // 1 MORAL fee
      const result = selectCoins(balances, BigInt(99_500_000_000_000), { fee });

      // Need 99.5 + 1 = 100.5 MORAL, but only have 100
      expect(result.sufficient).toBe(false);
    });

    it('should succeed when fee is covered', () => {
      const balances: DetectedStealthBalance[] = [
        createBalance('addr1', BigInt(100_000_000_000_000)),
        createBalance('addr2', BigInt(50_000_000_000_000)),
      ];

      const fee = BigInt(1_000_000_000_000);
      const result = selectCoins(balances, BigInt(99_000_000_000_000), { fee });

      expect(result.sufficient).toBe(true);
      expect(result.total).toBeGreaterThanOrEqual(BigInt(100_000_000_000_000));
    });
  });

  describe('LINKABILITY_WARNING_THRESHOLD', () => {
    it('should be at least 2', () => {
      expect(LINKABILITY_WARNING_THRESHOLD).toBeGreaterThanOrEqual(2);
    });
  });
});

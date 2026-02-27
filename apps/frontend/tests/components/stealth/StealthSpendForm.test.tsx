/**
 * T064 [US4] Jest test: StealthSpendForm submission
 *
 * Tests for spending stealth balances
 */

import { StealthSpendForm, validateSpendForm, SpendFormValues } from '@/components/stealth/StealthSpendForm';
import { DetectedStealthBalance } from '@/lib/stealth/types';

// Mock wasm functions
jest.mock('anarchy-wasm-engine', () => ({
  derive_stealth_private_key: jest.fn(() => new Uint8Array(32)),
}));

describe('StealthSpendForm', () => {
  // テストヘルパー
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

  describe('validateSpendForm', () => {
    const validBalance = createBalance('5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY', BigInt(100_000_000_000_000));

    it('should accept valid form values', () => {
      const values: SpendFormValues = {
        selectedBalances: [validBalance],
        recipientAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        amount: BigInt(50_000_000_000_000),
      };

      const result = validateSpendForm(values);
      expect(result.valid).toBe(true);
      expect(result.errors).toEqual({});
    });

    it('should reject when no balance selected', () => {
      const values: SpendFormValues = {
        selectedBalances: [],
        recipientAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        amount: BigInt(50_000_000_000_000),
      };

      const result = validateSpendForm(values);
      expect(result.valid).toBe(false);
      expect(result.errors.selectedBalances).toBe('残高を選択してください');
    });

    it('should reject empty recipient address', () => {
      const values: SpendFormValues = {
        selectedBalances: [validBalance],
        recipientAddress: '',
        amount: BigInt(50_000_000_000_000),
      };

      const result = validateSpendForm(values);
      expect(result.valid).toBe(false);
      expect(result.errors.recipientAddress).toBe('送金先アドレスを入力してください');
    });

    it('should reject zero amount', () => {
      const values: SpendFormValues = {
        selectedBalances: [validBalance],
        recipientAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        amount: BigInt(0),
      };

      const result = validateSpendForm(values);
      expect(result.valid).toBe(false);
      expect(result.errors.amount).toBe('金額を入力してください');
    });

    it('should reject insufficient funds', () => {
      const values: SpendFormValues = {
        selectedBalances: [validBalance],
        recipientAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        amount: BigInt(200_000_000_000_000), // 200 MORAL but only have 100
      };

      const result = validateSpendForm(values);
      expect(result.valid).toBe(false);
      expect(result.errors.amount).toBe('残高が不足しています');
    });

    it('should allow exact amount match', () => {
      const values: SpendFormValues = {
        selectedBalances: [validBalance],
        recipientAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        amount: BigInt(100_000_000_000_000), // Exact match
      };

      const result = validateSpendForm(values);
      // Note: May need fee consideration in real impl
      expect(result.valid).toBe(true);
    });

    it('should detect linkability when multiple inputs selected', () => {
      const balance1 = createBalance('addr1', BigInt(50_000_000_000_000));
      const balance2 = createBalance('addr2', BigInt(50_000_000_000_000));

      const values: SpendFormValues = {
        selectedBalances: [balance1, balance2],
        recipientAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        amount: BigInt(80_000_000_000_000),
      };

      const result = validateSpendForm(values);
      expect(result.valid).toBe(true);
      expect(result.linkabilityWarning).toBe(true);
    });

    it('should reject spent balances', () => {
      const spentBalance = createBalance('addr1', BigInt(100_000_000_000_000), true);

      const values: SpendFormValues = {
        selectedBalances: [spentBalance],
        recipientAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
        amount: BigInt(50_000_000_000_000),
      };

      const result = validateSpendForm(values);
      expect(result.valid).toBe(false);
      expect(result.errors.selectedBalances).toBe('使用済みの残高が含まれています');
    });
  });

  describe('SpendFormValues total calculation', () => {
    it('should sum multiple balance amounts correctly', () => {
      const balances = [
        createBalance('addr1', BigInt(50_000_000_000_000)),
        createBalance('addr2', BigInt(30_000_000_000_000)),
        createBalance('addr3', BigInt(20_000_000_000_000)),
      ];

      const total = balances.reduce((sum, b) => sum + b.balance, BigInt(0));
      expect(total).toBe(BigInt(100_000_000_000_000));
    });
  });

  describe('StealthSpendForm component export', () => {
    it('should export StealthSpendForm component', () => {
      expect(StealthSpendForm).toBeDefined();
    });

    it('should export validateSpendForm function', () => {
      expect(validateSpendForm).toBeDefined();
      expect(typeof validateSpendForm).toBe('function');
    });
  });
});

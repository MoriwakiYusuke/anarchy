/**
 * T052: Jest tests for DetectedStealthBalance state management
 *
 * Tests the balance store for managing detected stealth balances (in-memory)
 */

import { 
  BalanceStore, 
  createBalanceStore, 
  DetectedBalance 
} from '@/lib/stealth/balanceStore';

describe('BalanceStore', () => {
  describe('createBalanceStore', () => {
    it('should create an empty store', () => {
      const store = createBalanceStore();
      expect(store.getAll()).toEqual([]);
    });
  });

  describe('add', () => {
    it('should add a new balance', () => {
      const store = createBalanceStore();
      
      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(100_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      txHash: new Uint8Array(32),
      });

      expect(store.getAll().length).toBe(1);
      expect(store.getAll()[0].stealthAddress).toBe('addr1');
    });

    it('should update balance for duplicate addresses', () => {
      const store = createBalanceStore();
      
      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(100_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      txHash: new Uint8Array(32),
      });

      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(200_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      txHash: new Uint8Array(32),
      });

      expect(store.getAll().length).toBe(1);
      expect(store.getAll()[0].balance).toBe(BigInt(200_000_000_000_000).toString());
    });

    it('should notify subscribers', () => {
      const store = createBalanceStore();
      const callback = jest.fn();
      
      store.subscribe(callback);
      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(100_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      txHash: new Uint8Array(32),
      });

      expect(callback).toHaveBeenCalledWith(expect.any(Array));
    });
  });

  describe('remove', () => {
    it('should remove a balance by address', () => {
      const store = createBalanceStore();
      
      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(100_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      txHash: new Uint8Array(32),
      });

      store.remove('addr1');
      expect(store.getAll().length).toBe(0);
    });
  });

  describe('getTotalBalance', () => {
    it('should sum all balances correctly', () => {
      const store = createBalanceStore();
      
      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(100_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      txHash: new Uint8Array(32),
      });

      store.add({
        stealthAddress: 'addr2',
        balance: BigInt(50_000_000_000_000),
        blockNumber: 101,
        ephemeralPubkey: new Uint8Array([1, ...new Array(31).fill(0)]),
        txHash: new Uint8Array(32),
      });

      expect(store.getTotalBalance()).toBe(BigInt(150_000_000_000_000));
    });

    it('should return 0 for empty store', () => {
      const store = createBalanceStore();
      expect(store.getTotalBalance()).toBe(BigInt(0));
    });
  });

  describe('subscribe and unsubscribe', () => {
    it('should allow subscription and unsubscription', () => {
      const store = createBalanceStore();
      const callback = jest.fn();
      
      const unsubscribe = store.subscribe(callback);
      
      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(100_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      txHash: new Uint8Array(32),
      });

      expect(callback).toHaveBeenCalledTimes(1);

      unsubscribe();
      
      store.add({
        stealthAddress: 'addr2',
        balance: BigInt(50_000_000_000_000),
        blockNumber: 101,
        ephemeralPubkey: new Uint8Array([1, ...new Array(31).fill(0)]),
        txHash: new Uint8Array(32),
      });

      expect(callback).toHaveBeenCalledTimes(1); // Not called again
    });
  });

  describe('clear', () => {
    it('should remove all balances', () => {
      const store = createBalanceStore();
      
      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(100_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      txHash: new Uint8Array(32),
      });

      store.clear();
      expect(store.getAll().length).toBe(0);
    });
  });
});

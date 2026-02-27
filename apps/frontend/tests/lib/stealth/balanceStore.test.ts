/**
 * T052: Jest tests for DetectedStealthBalance state management
 *
 * Tests the balance store for persisting and managing detected stealth balances
 */

import { 
  BalanceStore, 
  createBalanceStore, 
  DetectedBalance 
} from '@/lib/stealth/balanceStore';

// Mock localStorage
const localStorageMock = {
  store: {} as Record<string, string>,
  getItem: jest.fn((key: string) => localStorageMock.store[key] || null),
  setItem: jest.fn((key: string, value: string) => {
    localStorageMock.store[key] = value;
  }),
  removeItem: jest.fn((key: string) => {
    delete localStorageMock.store[key];
  }),
  clear: jest.fn(() => {
    localStorageMock.store = {};
  }),
};

Object.defineProperty(window, 'localStorage', {
  value: localStorageMock,
});

describe('BalanceStore', () => {
  beforeEach(() => {
    localStorageMock.clear();
    jest.clearAllMocks();
  });

  describe('createBalanceStore', () => {
    it('should create an empty store', () => {
      const store = createBalanceStore();
      expect(store.getAll()).toEqual([]);
    });

    it('should load existing balances from localStorage', () => {
      const existingBalances: DetectedBalance[] = [
        {
          stealthAddress: 'addr1',
          balance: BigInt(100_000_000_000_000).toString(),
          blockNumber: 100,
          ephemeralPubkey: Array.from(new Uint8Array(32)),
          detectedAt: Date.now(),
        },
      ];
      localStorageMock.setItem('stealth_balances', JSON.stringify(existingBalances));

      const store = createBalanceStore();
      expect(store.getAll().length).toBe(1);
      expect(store.getAll()[0].stealthAddress).toBe('addr1');
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
      });

      expect(store.getAll().length).toBe(1);
      expect(store.getAll()[0].stealthAddress).toBe('addr1');
    });

    it('should persist to localStorage', () => {
      const store = createBalanceStore();
      
      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(100_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      });

      expect(localStorageMock.setItem).toHaveBeenCalledWith(
        'stealth_balances',
        expect.any(String)
      );
    });

    it('should not add duplicates', () => {
      const store = createBalanceStore();
      
      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(100_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      });

      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(200_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      });

      expect(store.getAll().length).toBe(1);
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
      });

      store.remove('addr1');
      expect(store.getAll().length).toBe(0);
    });

    it('should persist removal to localStorage', () => {
      const store = createBalanceStore();
      
      store.add({
        stealthAddress: 'addr1',
        balance: BigInt(100_000_000_000_000),
        blockNumber: 100,
        ephemeralPubkey: new Uint8Array(32),
      });

      const callCount = localStorageMock.setItem.mock.calls.length;
      store.remove('addr1');

      expect(localStorageMock.setItem).toHaveBeenCalledTimes(callCount + 1);
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
      });

      store.add({
        stealthAddress: 'addr2',
        balance: BigInt(50_000_000_000_000),
        blockNumber: 101,
        ephemeralPubkey: new Uint8Array([1, ...new Array(31).fill(0)]),
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
      });

      expect(callback).toHaveBeenCalledTimes(1);

      unsubscribe();
      
      store.add({
        stealthAddress: 'addr2',
        balance: BigInt(50_000_000_000_000),
        blockNumber: 101,
        ephemeralPubkey: new Uint8Array([1, ...new Array(31).fill(0)]),
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
      });

      store.clear();
      expect(store.getAll().length).toBe(0);
    });
  });
});

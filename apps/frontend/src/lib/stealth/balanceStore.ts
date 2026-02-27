/**
 * T057: Detected Balance State Store
 *
 * Manages detected stealth balances with localStorage persistence
 */

const STORAGE_KEY = 'stealth_balances';

export interface DetectedBalance {
  stealthAddress: string;
  balance: string; // BigInt as string for JSON serialization
  blockNumber: number;
  ephemeralPubkey: number[]; // Uint8Array as number[] for JSON
  detectedAt: number;
  spent?: boolean;
  spentAt?: number;
}

export interface AddBalanceParams {
  stealthAddress: string;
  balance: bigint;
  blockNumber: number;
  ephemeralPubkey: Uint8Array;
}

export interface BalanceStore {
  /** Get all detected balances */
  getAll(): DetectedBalance[];
  
  /** Get unspent balances only */
  getUnspent(): DetectedBalance[];
  
  /** Add a new detected balance */
  add(params: AddBalanceParams): void;
  
  /** Remove a balance by address */
  remove(address: string): void;
  
  /** Mark a balance as spent */
  markSpent(address: string): void;
  
  /** Get total balance (sum of all unspent) */
  getTotalBalance(): bigint;
  
  /** Subscribe to changes */
  subscribe(callback: (balances: DetectedBalance[]) => void): () => void;
  
  /** Clear all balances */
  clear(): void;
}

/**
 * Create a new balance store with localStorage persistence
 */
export function createBalanceStore(): BalanceStore {
  let balances: DetectedBalance[] = [];
  const subscribers = new Set<(balances: DetectedBalance[]) => void>();

  // Load from localStorage
  const loadFromStorage = () => {
    if (typeof window === 'undefined') return;
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        balances = JSON.parse(stored);
      }
    } catch (error) {
      console.warn('[BalanceStore] Failed to load from localStorage:', error);
    }
  };

  // Save to localStorage
  const saveToStorage = () => {
    if (typeof window === 'undefined') return;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(balances));
    } catch (error) {
      console.warn('[BalanceStore] Failed to save to localStorage:', error);
    }
  };

  // Notify all subscribers
  const notifySubscribers = () => {
    for (const callback of subscribers) {
      callback([...balances]);
    }
  };

  // Initialize from storage
  loadFromStorage();

  return {
    getAll(): DetectedBalance[] {
      return [...balances];
    },

    getUnspent(): DetectedBalance[] {
      return balances.filter((b) => !b.spent);
    },

    add(params: AddBalanceParams): void {
      // Check for duplicate
      const exists = balances.some(
        (b) => b.stealthAddress === params.stealthAddress
      );
      if (exists) {
        return;
      }

      const newBalance: DetectedBalance = {
        stealthAddress: params.stealthAddress,
        balance: params.balance.toString(),
        blockNumber: params.blockNumber,
        ephemeralPubkey: Array.from(params.ephemeralPubkey),
        detectedAt: Date.now(),
      };

      balances.push(newBalance);
      saveToStorage();
      notifySubscribers();
    },

    remove(address: string): void {
      balances = balances.filter((b) => b.stealthAddress !== address);
      saveToStorage();
      notifySubscribers();
    },

    markSpent(address: string): void {
      const balance = balances.find((b) => b.stealthAddress === address);
      if (balance) {
        balance.spent = true;
        balance.spentAt = Date.now();
        saveToStorage();
        notifySubscribers();
      }
    },

    getTotalBalance(): bigint {
      return balances
        .filter((b) => !b.spent)
        .reduce((sum, b) => sum + BigInt(b.balance), BigInt(0));
    },

    subscribe(callback: (balances: DetectedBalance[]) => void): () => void {
      subscribers.add(callback);
      return () => {
        subscribers.delete(callback);
      };
    },

    clear(): void {
      balances = [];
      saveToStorage();
      notifySubscribers();
    },
  };
}

// Singleton instance for app-wide use
let _storeInstance: BalanceStore | null = null;

export function getBalanceStore(): BalanceStore {
  if (!_storeInstance) {
    _storeInstance = createBalanceStore();
  }
  return _storeInstance;
}

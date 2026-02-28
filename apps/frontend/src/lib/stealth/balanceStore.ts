/**
 * T057: Detected Balance State Store
 *
 * Manages detected stealth balances in memory (no persistence)
 */

import type { DetectedStealthBalance } from './types';

/**
 * Internal format for balance data
 */
export interface DetectedBalance {
  stealthAddress: string;
  balance: string; // BigInt as string
  blockNumber: number;
  ephemeralPubkey: number[]; // Uint8Array as number[]
  txHash: number[]; // Uint8Array as number[]
  detectedAt: number;
  spent?: boolean;
  spentAt?: number;
}

export interface AddBalanceParams {
  stealthAddress: string;
  balance: bigint;
  blockNumber: number;
  ephemeralPubkey: Uint8Array;
  txHash: Uint8Array;
}

export interface BalanceStore {
  /** Get all detected balances (internal serializable format) */
  getAll(): DetectedBalance[];
  
  /** Get all balances as DetectedStealthBalance (API-compatible format) */
  getAllAsStealthBalance(): DetectedStealthBalance[];
  
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
 * Convert internal format to DetectedStealthBalance
 */
export function toStealthBalance(internal: DetectedBalance): DetectedStealthBalance {
  return {
    stealthAddress: internal.stealthAddress,
    balance: BigInt(internal.balance),
    receivedAt: internal.blockNumber,
    txHash: new Uint8Array(internal.txHash),
    spent: internal.spent ?? false,
    ephemeralPubkey: new Uint8Array(internal.ephemeralPubkey),
  };
}

/**
 * Create a new balance store (in-memory only)
 */
export function createBalanceStore(): BalanceStore {
  let balances: DetectedBalance[] = [];
  const subscribers = new Set<(balances: DetectedBalance[]) => void>();

  // Notify all subscribers
  const notifySubscribers = () => {
    for (const callback of subscribers) {
      callback([...balances]);
    }
  };

  return {
    getAll(): DetectedBalance[] {
      return [...balances];
    },

    getAllAsStealthBalance(): DetectedStealthBalance[] {
      return balances.map(toStealthBalance);
    },

    getUnspent(): DetectedBalance[] {
      return balances.filter((b) => !b.spent);
    },

    add(params: AddBalanceParams): void {
      // Check for duplicate - if exists, update the balance
      const existingIndex = balances.findIndex(
        (b) => b.stealthAddress === params.stealthAddress
      );
      
      if (existingIndex >= 0) {
        // Update existing balance
        balances[existingIndex].balance = params.balance.toString();
        console.log(`[BalanceStore] Updated existing balance for ${params.stealthAddress}: ${params.balance.toString()}`);
        notifySubscribers();
        return;
      }

      const newBalance: DetectedBalance = {
        stealthAddress: params.stealthAddress,
        balance: params.balance.toString(),
        blockNumber: params.blockNumber,
        ephemeralPubkey: Array.from(params.ephemeralPubkey),
        txHash: Array.from(params.txHash),
        detectedAt: Date.now(),
      };

      console.log(`[BalanceStore] Adding new balance for ${params.stealthAddress}: ${params.balance.toString()}`);
      balances.push(newBalance);
      notifySubscribers();
    },

    remove(address: string): void {
      balances = balances.filter((b) => b.stealthAddress !== address);
      notifySubscribers();
    },

    markSpent(address: string): void {
      const balance = balances.find((b) => b.stealthAddress === address);
      if (balance) {
        balance.spent = true;
        balance.spentAt = Date.now();
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

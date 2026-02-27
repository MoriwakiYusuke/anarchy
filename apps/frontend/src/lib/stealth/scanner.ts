/**
 * T054-T055: StealthScanner Class
 * T080: RPC retry logic with exponential backoff
 * T083: Web Worker fallback for unsupported browsers
 *
 * Scans blockchain blocks for stealth payments owned by the user.
 */

import { getEphemeralKeys } from './api';
import type { EphemeralKeyEntry, ScanProgress } from './types';

// Re-export ScanProgress from types
export type { ScanProgress } from './types';

/**
 * T083: Check if Web Workers are supported
 * Falls back to main thread processing when not available
 */
export function isWorkerSupported(): boolean {
  if (typeof window === 'undefined') return false;
  return typeof Worker !== 'undefined';
}

export interface ScanResult {
  blockNumber: number;
  stealthAddress: string;
  ephemeralPubkey: Uint8Array;
  isOwned: boolean;
}

export interface ScanOptions {
  batchSize?: number;
  delayBetweenBatches?: number;
  maxRetries?: number;
  baseRetryDelayMs?: number;
  /** T083: Use main thread instead of Worker (default: auto-detect) */
  useMainThread?: boolean;
}

/**
 * T080: Retry with exponential backoff
 */
async function retryWithBackoff<T>(
  fn: () => Promise<T>,
  maxRetries: number = 3,
  baseDelayMs: number = 1000
): Promise<T> {
  let lastError: unknown;
  
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error;
      
      if (attempt < maxRetries) {
        const delay = baseDelayMs * Math.pow(2, attempt);
        console.warn(`[RPC] Retry ${attempt + 1}/${maxRetries} after ${delay}ms`);
        await new Promise(resolve => setTimeout(resolve, delay));
      }
    }
  }
  
  throw lastError;
}

/**
 * T054: StealthScanner class for detecting owned stealth payments
 */
export class StealthScanner {
  private viewKey: Uint8Array;
  private spendPubkey: Uint8Array;
  private api: unknown;
  private stopped = false;

  constructor(viewKey: Uint8Array, spendPubkey: Uint8Array, api: unknown) {
    this.viewKey = viewKey;
    this.spendPubkey = spendPubkey;
    this.api = api;
  }

  /**
   * T055: Scan a range of blocks for stealth payments
   */
  async scanBlockRange(
    startBlock: number,
    endBlock: number,
    onProgress?: (progress: ScanProgress) => void,
    options: ScanOptions = {}
  ): Promise<ScanResult[]> {
    const { 
      batchSize = 1000, // T082: Optimized batch size for performance
      delayBetweenBatches = 50,
      maxRetries = 3,
      baseRetryDelayMs = 1000,
    } = options;
    const results: ScanResult[] = [];
    const total = endBlock - startBlock + 1;
    let scanned = 0;
    let found = 0;

    this.stopped = false;

    // Import wasm module
    const wasm = await import('anarchy-wasm-engine');

    // Process blocks in batches
    for (let batchStart = startBlock; batchStart <= endBlock; batchStart += batchSize) {
      if (this.stopped) break;

      const batchEnd = Math.min(batchStart + batchSize - 1, endBlock);
      const batchPromises: Promise<{ blockNumber: number; entries: EphemeralKeyEntry[] | null }>[] = [];

      // Queue batch of block fetches with retry logic
      for (let blockNum = batchStart; blockNum <= batchEnd; blockNum++) {
        batchPromises.push(
          retryWithBackoff(
            () => getEphemeralKeys(this.api, blockNum),
            maxRetries,
            baseRetryDelayMs
          ).then((entries) => ({
            blockNumber: blockNum,
            entries,
          })).catch((error) => {
            console.warn(`[StealthScanner] Failed to fetch block ${blockNum} after retries:`, error);
            return { blockNumber: blockNum, entries: null };
          })
        );
      }

      // Process batch results
      const batchResults = await Promise.all(batchPromises);

      for (const { blockNumber, entries } of batchResults) {
        if (this.stopped) break;

        scanned++;

        if (entries && entries.length > 0) {
          // Check each ephemeral key entry
          for (const entry of entries) {
            try {
              // scan_transaction(view_key, ephemeral_pubkey, stealth_address, spend_pubkey) -> bool
              const isOwned = wasm.scan_transaction(
                this.viewKey,
                entry.ephemeralPubkey,
                entry.stealthAddress,
                this.spendPubkey
              );

              if (isOwned) {
                found++;
                results.push({
                  blockNumber,
                  stealthAddress: entry.stealthAddress,
                  ephemeralPubkey: entry.ephemeralPubkey,
                  isOwned: true,
                });
              }
            } catch (error) {
              console.warn(`[StealthScanner] Error scanning block ${blockNumber}:`, error);
            }
          }
        }

        // Report progress (matching ScanProgress interface from types.ts)
        onProgress?.({
          currentBlock: blockNumber,
          targetBlock: endBlock,
          percentage: Math.round((scanned / total) * 100),
          detectedCount: found,
        });
      }

      // Small delay between batches to avoid overwhelming the API
      if (batchEnd < endBlock && delayBetweenBatches > 0) {
        await new Promise((resolve) => setTimeout(resolve, delayBetweenBatches));
      }
    }

    return results;
  }

  /**
   * Stop scanning
   */
  stop(): void {
    this.stopped = true;
  }

  /**
   * Check if scanner is currently running
   */
  isRunning(): boolean {
    return !this.stopped;
  }
}

/**
 * Create a new scanner instance
 */
export function createScanner(
  viewKey: Uint8Array,
  spendPubkey: Uint8Array,
  api: unknown
): StealthScanner {
  return new StealthScanner(viewKey, spendPubkey, api);
}

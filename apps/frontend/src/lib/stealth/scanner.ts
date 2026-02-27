/**
 * T054-T055: StealthScanner Class
 *
 * Scans blockchain blocks for stealth payments owned by the user.
 */

import { getEphemeralKeys } from './api';
import type { EphemeralKeyEntry, ScanProgress } from './types';

// Re-export ScanProgress from types
export type { ScanProgress } from './types';

export interface ScanResult {
  blockNumber: number;
  stealthAddress: string;
  ephemeralPubkey: Uint8Array;
  isOwned: boolean;
}

export interface ScanOptions {
  batchSize?: number;
  delayBetweenBatches?: number;
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
    const { batchSize = 10, delayBetweenBatches = 50 } = options;
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

      // Queue batch of block fetches
      for (let blockNum = batchStart; blockNum <= batchEnd; blockNum++) {
        batchPromises.push(
          getEphemeralKeys(this.api, blockNum).then((entries) => ({
            blockNumber: blockNum,
            entries,
          }))
        );
      }

      // Process batch results
      const batchResults = await Promise.all(batchPromises);

      for (const { blockNumber, entries } of batchResults) {
        if (this.stopped) break;

        scanned++;

        if (!entries || entries.length === 0) {
          continue;
        }

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

/**
 * T054-T055: StealthScanner Class
 * T080: RPC retry logic with exponential backoff
 * T083: Web Worker fallback for unsupported browsers
 *
 * Scans blockchain blocks for stealth payments owned by the user.
 */

import { getEphemeralKeys } from './api';
import type { EphemeralKeyEntry, ScanProgress } from './types';
import { getSs58AddressInfo } from '@polkadot-api/substrate-bindings';
import * as wasm from 'anarchy-wasm-engine';

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
    
    // Debug: Log the keys being used for scanning
    console.log('[StealthScanner] Initialized with keys:', {
      viewKeyLen: viewKey.length,
      viewKeyFirst8: Array.from(viewKey.slice(0, 8)),
      spendPubkeyLen: spendPubkey.length,
      spendPubkeyFirst8: Array.from(spendPubkey.slice(0, 8)),
    });
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
          console.log(`[StealthScanner] Block ${blockNumber}: Processing ${entries.length} entries`);
          
          // Check each ephemeral key entry
          for (const entry of entries) {
            try {
              // Handle both camelCase and snake_case from chain
              const rawEntry = entry as unknown as Record<string, unknown>;
              const ephemeralPubkey = rawEntry.ephemeralPubkey ?? rawEntry.ephemeral_pubkey;
              const stealthAddress = rawEntry.stealthAddress ?? rawEntry.stealth_address;
              
              console.log('[StealthScanner] Extracted values:', {
                ephemeralPubkey,
                ephemeralPubkeyType: ephemeralPubkey?.constructor?.name,
                ephemeralPubkeyLen: ephemeralPubkey instanceof Uint8Array ? ephemeralPubkey.length : 
                                    Array.isArray(ephemeralPubkey) ? ephemeralPubkey.length : 'N/A',
                stealthAddress,
                stealthAddressType: stealthAddress?.constructor?.name,
              });
              
              // Convert to proper types
              let ephemeralBytes: Uint8Array;
              if (ephemeralPubkey instanceof Uint8Array) {
                ephemeralBytes = ephemeralPubkey;
              } else if (ArrayBuffer.isView(ephemeralPubkey)) {
                ephemeralBytes = new Uint8Array((ephemeralPubkey as ArrayBufferView).buffer);
              } else if (Array.isArray(ephemeralPubkey)) {
                ephemeralBytes = new Uint8Array(ephemeralPubkey);
              } else if (typeof ephemeralPubkey === 'object' && ephemeralPubkey !== null) {
                // Binary type from PAPI - might be { bytes: Uint8Array } or similar
                const bytes = (ephemeralPubkey as { asBytes?: () => Uint8Array }).asBytes?.();
                if (bytes) {
                  ephemeralBytes = bytes;
                } else {
                  console.warn('[StealthScanner] Unknown ephemeral pubkey format:', ephemeralPubkey);
                  continue;
                }
              } else {
                console.warn('[StealthScanner] Invalid ephemeral pubkey:', ephemeralPubkey);
                continue;
              }
              
              // Convert stealth address to string
              let stealthAddressStr: string;
              if (typeof stealthAddress === 'string') {
                stealthAddressStr = stealthAddress;
              } else if (stealthAddress && typeof (stealthAddress as { toString: () => string }).toString === 'function') {
                stealthAddressStr = (stealthAddress as { toString: () => string }).toString();
              } else {
                console.warn('[StealthScanner] Invalid stealth address:', stealthAddress);
                continue;
              }
              
              // Decode SS58 address to get pubkey bytes
              const addressInfo = getSs58AddressInfo(stealthAddressStr);
              if (!addressInfo.isValid) {
                console.warn('[StealthScanner] Invalid SS58 address:', stealthAddressStr);
                continue;
              }
              
              const stealthPubkeyBytes = addressInfo.publicKey;
              
              // Debug: compute expected stealth pubkey using wasm
              let expectedPubkey: Uint8Array | null = null;
              try {
                // Import derive function to compute what we expect
                const deriveFn = (wasm as unknown as { debug_derive_expected_pubkey?: (v: Uint8Array, e: Uint8Array, s: Uint8Array) => Uint8Array | null }).debug_derive_expected_pubkey;
                if (deriveFn) {
                  expectedPubkey = deriveFn(this.viewKey, ephemeralBytes, this.spendPubkey);
                }
              } catch {
                // Ignore if function doesn't exist
              }
              
              console.log('[StealthScanner] Calling scan_transaction with:', {
                viewKeyLen: this.viewKey.length,
                viewKeyFirst8: Array.from(this.viewKey.slice(0, 8)),
                ephemeralBytesLen: ephemeralBytes.length,
                ephemeralBytesFirst8: Array.from(ephemeralBytes.slice(0, 8)),
                stealthPubkeyBytesLen: stealthPubkeyBytes.length,
                stealthPubkeyFirst8: Array.from(stealthPubkeyBytes.slice(0, 8)),
                expectedPubkeyFirst8: expectedPubkey ? Array.from(expectedPubkey.slice(0, 8)) : 'N/A',
                spendPubkeyLen: this.spendPubkey.length,
                spendPubkeyFirst8: Array.from(this.spendPubkey.slice(0, 8)),
                pubkeysMatch: expectedPubkey ? Array.from(expectedPubkey).every((v, i) => v === stealthPubkeyBytes[i]) : 'N/A',
              });
              
              // scan_transaction(view_key, ephemeral_pubkey, stealth_pubkey, spend_pubkey) -> bool
              const isOwned = wasm.scan_transaction(
                this.viewKey,
                ephemeralBytes,
                stealthPubkeyBytes,
                this.spendPubkey
              );

              console.log(`[StealthScanner] scan_transaction result: ${isOwned}`);

              if (isOwned) {
                found++;
                results.push({
                  blockNumber,
                  stealthAddress: stealthAddressStr,
                  ephemeralPubkey: ephemeralBytes,
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

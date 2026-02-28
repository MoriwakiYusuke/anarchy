/**
 * T045: PAPI wrapper for stealth pallet transactions
 *
 * Provides type-safe wrappers for interacting with the stealth pallet on-chain.
 */

import type { Binary } from 'polkadot-api'
import type { EphemeralKeyEntry } from './types'

// Type for the API shape (from useSmoldot's getUnsafeApi)
export interface StealthApi {
  tx: {
    Stealth: {
      send_to_stealth: (params: {
        stealth_address: string
        ephemeral_pubkey: Binary
        amount: bigint
      }) => {
        signAndSubmit: (signer: unknown) => Promise<{ txHash: string }>
      }
    }
  }
  query: {
    Stealth: {
      EphemeralKeys: {
        getValue: (blockNumber: number) => Promise<EphemeralKeyEntry[] | null>
        watchValue: (
          blockNumber: number | 'latest',
          callback: (value: EphemeralKeyEntry[] | null) => void
        ) => () => void
      }
    }
  }
}

export interface SendToStealthParams {
  /** Stealth address (SS58 encoded) */
  stealthAddress: string
  /** Ephemeral public key (32 bytes) */
  ephemeralPubkey: Uint8Array
  /** Amount in smallest units (1 MORAL = 10^12 units) */
  amount: bigint
}

export interface SendToStealthResult {
  txHash: string
  success: boolean
}

/**
 * Send funds to a stealth address
 *
 * @param api - The unsafe API from useSmoldot
 * @param signer - The signer (from useAuth or similar)
 * @param params - Transaction parameters
 * @returns Transaction result
 */
export async function sendToStealth(
  api: unknown,
  signer: unknown,
  params: SendToStealthParams
): Promise<SendToStealthResult> {
  console.log('[sendToStealth] Params:', {
    stealthAddress: params.stealthAddress,
    ephemeralPubkeyLength: params.ephemeralPubkey.length,
    amount: params.amount.toString(),
  });
  
  if (!api) {
    throw new Error('API not available');
  }
  if (!signer) {
    throw new Error('Signer not available');
  }

  // Import Binary from polkadot-api dynamically
  const { Binary } = await import('polkadot-api');
  
  // Validate the stealth address before submitting
  const { getSs58AddressInfo } = await import('@polkadot-api/substrate-bindings');
  const validation = getSs58AddressInfo(params.stealthAddress);
  console.log('[sendToStealth] Address validation:', validation.isValid);
  if (!validation.isValid) {
    throw new Error(`Invalid stealth address: ${params.stealthAddress}`);
  }

  const typedApi = api as StealthApi;

  // Convert ephemeral pubkey to Binary
  const ephemeralPubkeyBinary = Binary.fromBytes(params.ephemeralPubkey);

  console.log('[sendToStealth] Creating transaction...');
  
  // Create and submit transaction
  const tx = typedApi.tx.Stealth.send_to_stealth({
    stealth_address: params.stealthAddress,
    ephemeral_pubkey: ephemeralPubkeyBinary,
    amount: params.amount,
  });

  console.log('[sendToStealth] Signing and submitting...');
  const result = await tx.signAndSubmit(signer);
  console.log('[sendToStealth] Success:', result.txHash);

  return {
    txHash: result.txHash,
    success: true,
  };
}

/**
 * Query ephemeral keys for a specific block
 *
 * @param api - The unsafe API from useSmoldot
 * @param blockNumber - Block number to query
 * @returns Array of ephemeral key entries or null
 */
export async function getEphemeralKeys(
  api: unknown,
  blockNumber: number
): Promise<EphemeralKeyEntry[] | null> {
  if (!api) {
    throw new Error('API not available');
  }

  const typedApi = api as StealthApi;
  return typedApi.query.Stealth.EphemeralKeys.getValue(blockNumber);
}

/**
 * Watch ephemeral keys at latest block
 *
 * @param api - The unsafe API from useSmoldot
 * @param callback - Called when new ephemeral keys appear
 * @returns Unsubscribe function
 */
export function watchEphemeralKeys(
  api: unknown,
  callback: (entries: EphemeralKeyEntry[] | null) => void
): () => void {
  if (!api) {
    throw new Error('API not available');
  }

  const typedApi = api as StealthApi;
  return typedApi.query.Stealth.EphemeralKeys.watchValue('latest', callback);
}

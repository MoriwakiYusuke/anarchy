/**
 * WebAuthn Utility Functions
 * 
 * Provides base64URL encoding/decoding, WYSIWYS challenge generation,
 * and passkey ID derivation.
 */

import { blake2b } from '@noble/hashes/blake2.js';
import { sha256 } from '@noble/hashes/sha2.js';

/**
 * Base64URL encode a Uint8Array
 * WebAuthn uses Base64URL (URL-safe Base64 without padding)
 */
export function base64UrlEncode(data: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < data.length; i++) {
    binary += String.fromCharCode(data[i]);
  }
  return btoa(binary)
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=/g, '');
}

/**
 * Base64URL decode a string to Uint8Array
 */
export function base64UrlDecode(str: string): Uint8Array {
  // Add padding if needed
  let padded = str
    .replace(/-/g, '+')
    .replace(/_/g, '/');
  
  // Add padding
  const paddingNeeded = (4 - (padded.length % 4)) % 4;
  padded += '='.repeat(paddingNeeded);
  
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/**
 * Generate WYSIWYS (What You Sign Is What You See) challenge
 * 
 * Format: prefix(16 bytes) + SHA-256(content)(32 bytes) + suffix(16 bytes)
 * Total: 64 bytes
 * 
 * The content hash is embedded in the middle so the on-chain verification
 * can extract and verify it matches the posted content.
 * 
 * @param content - The content string to sign
 * @returns 64-byte challenge with embedded content hash
 */
export async function generateWysiwysChallenge(content: string): Promise<Uint8Array> {
  // Encode content to UTF-8
  const encoder = new TextEncoder();
  const contentBytes = encoder.encode(content);
  
  // Calculate SHA-256 hash using Web Crypto API (more standard)
  // Falls back to @noble/hashes if not available
  let contentHash: Uint8Array;
  if (typeof crypto !== 'undefined' && crypto.subtle) {
    const hashBuffer = await crypto.subtle.digest('SHA-256', contentBytes);
    contentHash = new Uint8Array(hashBuffer);
  } else {
    contentHash = sha256(contentBytes);
  }
  
  // Create 64-byte challenge
  // Format: [random prefix 16b][content hash 32b][random suffix 16b]
  const challenge = new Uint8Array(64);
  
  // Generate random prefix and suffix
  if (typeof crypto !== 'undefined' && crypto.getRandomValues) {
    crypto.getRandomValues(challenge.subarray(0, 16));  // prefix
    crypto.getRandomValues(challenge.subarray(48, 64)); // suffix
  } else {
    // Fallback for testing environments
    for (let i = 0; i < 16; i++) {
      challenge[i] = Math.floor(Math.random() * 256);
      challenge[48 + i] = Math.floor(Math.random() * 256);
    }
  }
  
  // Embed content hash in the middle
  challenge.set(contentHash, 16);
  
  return challenge;
}

/**
 * Extract content hash from WYSIWYS challenge
 * Used for verification purposes
 * 
 * @param challenge - 64-byte challenge
 * @returns 32-byte content hash
 */
export function extractContentHashFromChallenge(challenge: Uint8Array): Uint8Array {
  if (challenge.length < 48) {
    throw new Error('Challenge too short to contain content hash');
  }
  return challenge.slice(16, 48);
}

/**
 * Derive passkey ID from COSE public key
 * 
 * This MUST match the Identity Pallet's passkey ID derivation:
 * passkey_id = Blake2-256(cose_public_key)
 * 
 * @param cosePublicKey - COSE encoded public key bytes
 * @returns 32-byte passkey ID (Blake2-256 hash)
 */
export function derivePasskeyId(cosePublicKey: Uint8Array): Uint8Array {
  // Use Blake2b with 32-byte output (Blake2-256)
  return blake2b(cosePublicKey, { dkLen: 32 });
}

/**
 * Concatenate multiple Uint8Arrays
 */
export function concatBytes(...arrays: Uint8Array[]): Uint8Array {
  const totalLength = arrays.reduce((sum, arr) => sum + arr.length, 0);
  const result = new Uint8Array(totalLength);
  let offset = 0;
  for (const arr of arrays) {
    result.set(arr, offset);
    offset += arr.length;
  }
  return result;
}

/**
 * Compare two Uint8Arrays for equality
 */
export function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

/**
 * Convert a hex string to Uint8Array
 */
export function hexToBytes(hex: string): Uint8Array {
  const cleanHex = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (cleanHex.length % 2 !== 0) {
    throw new Error('Invalid hex string length');
  }
  const bytes = new Uint8Array(cleanHex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(cleanHex.substr(i * 2, 2), 16);
  }
  return bytes;
}

/**
 * Convert a Uint8Array to hex string
 */
export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * WebAuthn PublicKeyCredentialCreationOptions helper
 * Creates options for navigator.credentials.create()
 */
export interface CreateCredentialOptions {
  /** User ID (Uint8Array, max 64 bytes) */
  userId: Uint8Array;
  /** User name (account identifier) */
  userName: string;
  /** User display name (human-readable, optional) */
  userDisplayName?: string;
  /** Challenge bytes (should be random for registration) */
  challenge: Uint8Array;
  /** RP ID (defaults to current hostname) */
  rpId?: string;
  /** RP name */
  rpName?: string;
  /** Timeout in milliseconds (default 120000 = 2 minutes) */
  timeout?: number;
}

export function createCredentialCreationOptions(
  options: CreateCredentialOptions
): PublicKeyCredentialCreationOptions {
  const rpId = options.rpId || (typeof window !== 'undefined' ? window.location.hostname : 'localhost');
  
  return {
    challenge: options.challenge as BufferSource,
    rp: {
      name: options.rpName || 'Anarchy',
      id: rpId,
    },
    user: {
      id: options.userId as BufferSource,
      name: options.userName,
      displayName: options.userDisplayName || options.userName,
    },
    pubKeyCredParams: [
      { type: 'public-key', alg: -7 },   // ES256 (ECDSA w/ SHA-256)
      { type: 'public-key', alg: -257 }, // RS256 (RSASSA-PKCS1-v1_5)
    ],
    authenticatorSelection: {
      authenticatorAttachment: 'platform',
      userVerification: 'required',
      residentKey: 'required',
      requireResidentKey: true,
    },
    timeout: options.timeout || 120000,
    attestation: 'none', // We don't need attestation for privacy
  };
}

/**
 * WebAuthn PublicKeyCredentialRequestOptions helper
 * Creates options for navigator.credentials.get()
 */
export interface GetCredentialOptions {
  /** Challenge bytes (WYSIWYS challenge for signing) */
  challenge: Uint8Array;
  /** RP ID (defaults to current hostname) */
  rpId?: string;
  /** Timeout in milliseconds (default 120000 = 2 minutes) */
  timeout?: number;
  /** Allowed credential IDs (empty for discoverable credentials) */
  allowCredentials?: { id: Uint8Array; type: 'public-key' }[];
}

export function createCredentialRequestOptions(
  options: GetCredentialOptions
): PublicKeyCredentialRequestOptions {
  const rpId = options.rpId || (typeof window !== 'undefined' ? window.location.hostname : 'localhost');
  
  return {
    challenge: options.challenge as BufferSource,
    rpId,
    timeout: options.timeout || 120000,
    userVerification: 'required',
    allowCredentials: options.allowCredentials?.map(cred => ({
      id: cred.id as BufferSource,
      type: cred.type,
    })) || [],
  };
}

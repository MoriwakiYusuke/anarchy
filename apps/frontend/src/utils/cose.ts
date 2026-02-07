/**
 * COSE (CBOR Object Signing and Encryption) Utility Functions
 * 
 * Extracts COSE public keys from WebAuthn attestation objects.
 * Used for passkey registration to send the public key to the Identity Pallet.
 */

import { decode as cborDecode } from 'cbor-x';

/**
 * COSE Key Types
 */
export const COSE_KTY = {
  OKP: 1,  // Octet Key Pair (EdDSA)
  EC2: 2,  // Elliptic Curve (ECDSA)
  RSA: 3,  // RSA
} as const;

/**
 * COSE Algorithms
 */
export const COSE_ALG = {
  ES256: -7,    // ECDSA w/ SHA-256
  ES384: -35,   // ECDSA w/ SHA-384
  ES512: -36,   // ECDSA w/ SHA-512
  RS256: -257,  // RSASSA-PKCS1-v1_5 w/ SHA-256
  EdDSA: -8,    // EdDSA
} as const;

/**
 * COSE EC Curves
 */
export const COSE_CRV = {
  P256: 1,  // NIST P-256
  P384: 2,  // NIST P-384
  P521: 3,  // NIST P-521
  Ed25519: 6,
} as const;

/**
 * COSE Key map labels
 * https://www.iana.org/assignments/cose/cose.xhtml#key-common-parameters
 */
const COSE_KEY_LABELS = {
  KTY: 1,   // Key Type
  ALG: 3,   // Algorithm
  CRV: -1,  // Curve (for EC2/OKP)
  X: -2,    // X Coordinate (for EC2/OKP)
  Y: -3,    // Y Coordinate (for EC2)
  N: -1,    // Modulus (for RSA)
  E: -2,    // Exponent (for RSA)
} as const;

/**
 * Parsed COSE Key structure
 */
export interface COSEKey {
  /** Key type (1=OKP, 2=EC2, 3=RSA) */
  kty: number;
  /** Algorithm (-7=ES256, -257=RS256, etc.) */
  alg: number;
  /** Curve (for EC2/OKP keys) */
  crv?: number;
  /** X coordinate (for EC2/OKP keys) */
  x?: Uint8Array;
  /** Y coordinate (for EC2 keys) */
  y?: Uint8Array;
  /** Raw COSE-encoded bytes */
  raw: Uint8Array;
}

/**
 * Extract COSE public key from WebAuthn attestation object
 * 
 * The attestation object structure (CBOR):
 * {
 *   "fmt": "none" | "packed" | ...,
 *   "attStmt": { ... },
 *   "authData": <binary authenticator data>
 * }
 * 
 * The authData structure:
 * - rpIdHash (32 bytes)
 * - flags (1 byte)
 * - signCount (4 bytes)
 * - [if AT flag] attestedCredentialData:
 *   - aaguid (16 bytes)
 *   - credentialIdLength (2 bytes, big-endian)
 *   - credentialId (credentialIdLength bytes)
 *   - credentialPublicKey (COSE_Key, remaining bytes)
 * 
 * @param attestationObject - Raw attestation object from WebAuthn credentials.create()
 * @returns COSE-encoded public key bytes
 * @throws Error if attestation object is invalid or no credential data
 */
export function extractCosePublicKey(attestationObject: ArrayBuffer): Uint8Array {
  const attestationBytes = new Uint8Array(attestationObject);
  
  // Decode CBOR attestation object
  let attestation: { fmt?: string; authData?: Uint8Array };
  try {
    attestation = cborDecode(attestationBytes);
  } catch (e) {
    throw new Error(`Failed to decode attestation object: ${e}`);
  }
  
  if (!attestation.authData) {
    throw new Error('Attestation object missing authData');
  }
  
  return extractCosePublicKeyFromAuthData(attestation.authData);
}

/**
 * Extract COSE public key from authenticator data
 * 
 * @param authData - Authenticator data bytes
 * @returns COSE-encoded public key bytes
 */
export function extractCosePublicKeyFromAuthData(authData: Uint8Array): Uint8Array {
  // Minimum size: rpIdHash(32) + flags(1) + signCount(4) = 37 bytes
  if (authData.length < 37) {
    throw new Error('AuthData too short');
  }
  
  // Check AT (Attested credential data) flag (bit 6)
  const flags = authData[32];
  const hasAttestedCredData = (flags & 0x40) !== 0;
  
  if (!hasAttestedCredData) {
    throw new Error('AuthData does not contain attested credential data');
  }
  
  // Skip to attested credential data
  // rpIdHash (32) + flags (1) + signCount (4) = 37
  let offset = 37;
  
  // Skip aaguid (16 bytes)
  offset += 16;
  
  if (authData.length < offset + 2) {
    throw new Error('AuthData too short for credential ID length');
  }
  
  // Read credential ID length (2 bytes, big-endian)
  const credIdLen = (authData[offset] << 8) | authData[offset + 1];
  offset += 2;
  
  // Skip credential ID
  offset += credIdLen;
  
  if (authData.length <= offset) {
    throw new Error('AuthData too short for credential public key');
  }
  
  // Remaining bytes are the COSE public key
  return authData.slice(offset);
}

/**
 * Parse a COSE public key into its components
 * 
 * @param coseKey - COSE-encoded public key bytes
 * @returns Parsed COSE key structure
 */
export function parseCoseKey(coseKey: Uint8Array): COSEKey {
  let decoded: Map<number, unknown>;
  try {
    decoded = cborDecode(coseKey);
  } catch (e) {
    throw new Error(`Failed to decode COSE key: ${e}`);
  }
  
  // Handle both Map and plain object from cbor-x
  const get = (key: number): unknown => {
    if (decoded instanceof Map) {
      return decoded.get(key);
    }
    return (decoded as Record<number, unknown>)[key];
  };
  
  const kty = get(COSE_KEY_LABELS.KTY) as number;
  const alg = get(COSE_KEY_LABELS.ALG) as number;
  
  if (typeof kty !== 'number') {
    throw new Error('COSE key missing kty (key type)');
  }
  
  const result: COSEKey = {
    kty,
    alg: typeof alg === 'number' ? alg : 0,
    raw: coseKey,
  };
  
  // Parse EC2 specific fields
  if (kty === COSE_KTY.EC2) {
    const crv = get(COSE_KEY_LABELS.CRV) as number;
    const x = get(COSE_KEY_LABELS.X) as Uint8Array;
    const y = get(COSE_KEY_LABELS.Y) as Uint8Array;
    
    if (typeof crv === 'number') result.crv = crv;
    if (x instanceof Uint8Array) result.x = x;
    if (y instanceof Uint8Array) result.y = y;
  }
  
  // Parse OKP specific fields
  if (kty === COSE_KTY.OKP) {
    const crv = get(COSE_KEY_LABELS.CRV) as number;
    const x = get(COSE_KEY_LABELS.X) as Uint8Array;
    
    if (typeof crv === 'number') result.crv = crv;
    if (x instanceof Uint8Array) result.x = x;
  }
  
  return result;
}

/**
 * Validate that a COSE key is supported for WebAuthn
 * Currently supports ES256 (P-256) which is the most common
 * 
 * @param coseKey - Parsed COSE key
 * @returns true if the key is supported
 */
export function isSupportedKey(coseKey: COSEKey): boolean {
  // ES256 (ECDSA with P-256)
  if (coseKey.kty === COSE_KTY.EC2 && 
      coseKey.alg === COSE_ALG.ES256 && 
      coseKey.crv === COSE_CRV.P256) {
    return true;
  }
  
  // RS256 is also commonly supported
  if (coseKey.kty === COSE_KTY.RSA && coseKey.alg === COSE_ALG.RS256) {
    return true;
  }
  
  return false;
}

/**
 * Get algorithm name from COSE algorithm identifier
 */
export function getAlgorithmName(alg: number): string {
  switch (alg) {
    case COSE_ALG.ES256: return 'ES256';
    case COSE_ALG.ES384: return 'ES384';
    case COSE_ALG.ES512: return 'ES512';
    case COSE_ALG.RS256: return 'RS256';
    case COSE_ALG.EdDSA: return 'EdDSA';
    default: return `Unknown(${alg})`;
  }
}

/**
 * Get curve name from COSE curve identifier
 */
export function getCurveName(crv: number): string {
  switch (crv) {
    case COSE_CRV.P256: return 'P-256';
    case COSE_CRV.P384: return 'P-384';
    case COSE_CRV.P521: return 'P-521';
    case COSE_CRV.Ed25519: return 'Ed25519';
    default: return `Unknown(${crv})`;
  }
}

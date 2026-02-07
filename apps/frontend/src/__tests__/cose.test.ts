import { describe, it, expect } from 'vitest';
import { encode as cborEncode } from 'cbor-x';
import {
  extractCosePublicKey,
  extractCosePublicKeyFromAuthData,
  parseCoseKey,
  isSupportedKey,
  getAlgorithmName,
  getCurveName,
  COSE_KTY,
  COSE_ALG,
  COSE_CRV,
} from '../utils/cose';

/**
 * Create a mock COSE key for testing
 * This creates a valid ES256 (P-256) COSE key structure
 */
function createMockCoseKey(options?: { kty?: number; alg?: number; crv?: number }): Uint8Array {
  const kty = options?.kty ?? COSE_KTY.EC2;
  const alg = options?.alg ?? COSE_ALG.ES256;
  const crv = options?.crv ?? COSE_CRV.P256;

  // Create CBOR map with numeric keys
  // Using Map to ensure integer keys
  const coseMap = new Map<number, unknown>();
  coseMap.set(1, kty);   // kty
  coseMap.set(3, alg);   // alg
  coseMap.set(-1, crv);  // crv
  coseMap.set(-2, new Uint8Array(32).fill(0x11));  // x
  coseMap.set(-3, new Uint8Array(32).fill(0x22));  // y

  return new Uint8Array(cborEncode(coseMap));
}

/**
 * Create mock authenticator data with attested credential data
 */
function createMockAuthData(credentialPublicKey: Uint8Array, credentialId?: Uint8Array): Uint8Array {
  const rpIdHash = new Uint8Array(32).fill(0xaa);
  const flags = 0x45; // UP + UV + AT flags
  const signCount = new Uint8Array([0x00, 0x00, 0x00, 0x01]);
  const aaguid = new Uint8Array(16).fill(0xbb);
  const credId = credentialId || new Uint8Array(32).fill(0xcc);
  const credIdLen = new Uint8Array([(credId.length >> 8) & 0xff, credId.length & 0xff]);

  const authData = new Uint8Array(
    32 + 1 + 4 + 16 + 2 + credId.length + credentialPublicKey.length
  );

  let offset = 0;
  authData.set(rpIdHash, offset); offset += 32;
  authData[offset++] = flags;
  authData.set(signCount, offset); offset += 4;
  authData.set(aaguid, offset); offset += 16;
  authData.set(credIdLen, offset); offset += 2;
  authData.set(credId, offset); offset += credId.length;
  authData.set(credentialPublicKey, offset);

  return authData;
}

/**
 * Create mock attestation object with authData
 */
function createMockAttestationObject(authData: Uint8Array): ArrayBuffer {
  const attestation = {
    fmt: 'none',
    attStmt: {},
    authData: authData,
  };
  const encoded = cborEncode(attestation);
  // cbor-x returns Uint8Array, we need ArrayBuffer
  return encoded.buffer.slice(encoded.byteOffset, encoded.byteOffset + encoded.byteLength);
}

describe('cose utilities', () => {
  describe('extractCosePublicKey', () => {
    it('should extract COSE key from valid attestation object', () => {
      const coseKey = createMockCoseKey();
      const authData = createMockAuthData(coseKey);
      const attestationObject = createMockAttestationObject(authData);

      const extracted = extractCosePublicKey(attestationObject);
      expect(extracted).toEqual(coseKey);
    });

    it('should handle different credential ID lengths', () => {
      const coseKey = createMockCoseKey();
      const longCredId = new Uint8Array(255).fill(0xdd);
      const authData = createMockAuthData(coseKey, longCredId);
      const attestationObject = createMockAttestationObject(authData);

      const extracted = extractCosePublicKey(attestationObject);
      expect(extracted).toEqual(coseKey);
    });

    it('should throw for invalid CBOR', () => {
      const invalidCbor = new Uint8Array([0xff, 0xff, 0xff]).buffer;
      expect(() => extractCosePublicKey(invalidCbor)).toThrow('Failed to decode attestation object');
    });

    it('should throw when authData is missing', () => {
      const encoded = cborEncode({ fmt: 'none', attStmt: {} });
      const noAuthData = encoded.buffer.slice(encoded.byteOffset, encoded.byteOffset + encoded.byteLength);
      expect(() => extractCosePublicKey(noAuthData)).toThrow('missing authData');
    });
  });

  describe('extractCosePublicKeyFromAuthData', () => {
    it('should extract COSE key from valid authData', () => {
      const coseKey = createMockCoseKey();
      const authData = createMockAuthData(coseKey);

      const extracted = extractCosePublicKeyFromAuthData(authData);
      expect(extracted).toEqual(coseKey);
    });

    it('should throw for authData without AT flag', () => {
      const authData = new Uint8Array(37).fill(0);
      authData[32] = 0x01; // Only UP flag, no AT flag
      
      expect(() => extractCosePublicKeyFromAuthData(authData)).toThrow('does not contain attested credential data');
    });

    it('should throw for authData too short', () => {
      const shortAuthData = new Uint8Array(30);
      expect(() => extractCosePublicKeyFromAuthData(shortAuthData)).toThrow('too short');
    });
  });

  describe('parseCoseKey', () => {
    it('should parse ES256 (P-256) key', () => {
      const coseKey = createMockCoseKey({
        kty: COSE_KTY.EC2,
        alg: COSE_ALG.ES256,
        crv: COSE_CRV.P256,
      });

      const parsed = parseCoseKey(coseKey);

      expect(parsed.kty).toBe(COSE_KTY.EC2);
      expect(parsed.alg).toBe(COSE_ALG.ES256);
      expect(parsed.crv).toBe(COSE_CRV.P256);
      expect(parsed.x).toBeDefined();
      expect(parsed.x?.length).toBe(32);
      expect(parsed.y).toBeDefined();
      expect(parsed.y?.length).toBe(32);
      expect(parsed.raw).toEqual(coseKey);
    });

    it('should parse key with different algorithm', () => {
      const coseKey = createMockCoseKey({
        alg: COSE_ALG.ES384,
        crv: COSE_CRV.P384,
      });

      const parsed = parseCoseKey(coseKey);

      expect(parsed.alg).toBe(COSE_ALG.ES384);
      expect(parsed.crv).toBe(COSE_CRV.P384);
    });

    it('should throw for invalid CBOR', () => {
      const invalid = new Uint8Array([0xff, 0xff]);
      expect(() => parseCoseKey(invalid)).toThrow('Failed to decode COSE key');
    });

    it('should throw for missing kty', () => {
      const noCoseMap = new Map<number, unknown>();
      noCoseMap.set(3, -7);  // Only alg, no kty
      const encoded = new Uint8Array(cborEncode(noCoseMap));
      
      expect(() => parseCoseKey(encoded)).toThrow('missing kty');
    });
  });

  describe('isSupportedKey', () => {
    it('should return true for ES256 (P-256) key', () => {
      const coseKey = createMockCoseKey({
        kty: COSE_KTY.EC2,
        alg: COSE_ALG.ES256,
        crv: COSE_CRV.P256,
      });
      const parsed = parseCoseKey(coseKey);

      expect(isSupportedKey(parsed)).toBe(true);
    });

    it('should return false for unsupported curve', () => {
      const coseKey = createMockCoseKey({
        kty: COSE_KTY.EC2,
        alg: COSE_ALG.ES384,
        crv: COSE_CRV.P384,
      });
      const parsed = parseCoseKey(coseKey);

      expect(isSupportedKey(parsed)).toBe(false);
    });

    it('should return false for EdDSA', () => {
      // Create OKP key structure
      const coseMap = new Map<number, unknown>();
      coseMap.set(1, COSE_KTY.OKP);
      coseMap.set(3, COSE_ALG.EdDSA);
      coseMap.set(-1, COSE_CRV.Ed25519);
      coseMap.set(-2, new Uint8Array(32).fill(0x11));
      const coseKey = new Uint8Array(cborEncode(coseMap));
      const parsed = parseCoseKey(coseKey);

      expect(isSupportedKey(parsed)).toBe(false);
    });
  });

  describe('getAlgorithmName', () => {
    it('should return correct names', () => {
      expect(getAlgorithmName(COSE_ALG.ES256)).toBe('ES256');
      expect(getAlgorithmName(COSE_ALG.ES384)).toBe('ES384');
      expect(getAlgorithmName(COSE_ALG.ES512)).toBe('ES512');
      expect(getAlgorithmName(COSE_ALG.RS256)).toBe('RS256');
      expect(getAlgorithmName(COSE_ALG.EdDSA)).toBe('EdDSA');
      expect(getAlgorithmName(999)).toBe('Unknown(999)');
    });
  });

  describe('getCurveName', () => {
    it('should return correct names', () => {
      expect(getCurveName(COSE_CRV.P256)).toBe('P-256');
      expect(getCurveName(COSE_CRV.P384)).toBe('P-384');
      expect(getCurveName(COSE_CRV.P521)).toBe('P-521');
      expect(getCurveName(COSE_CRV.Ed25519)).toBe('Ed25519');
      expect(getCurveName(999)).toBe('Unknown(999)');
    });
  });
});

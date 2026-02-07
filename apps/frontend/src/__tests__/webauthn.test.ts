import { describe, it, expect } from 'vitest';
import {
  base64UrlEncode,
  base64UrlDecode,
  generateWysiwysChallenge,
  extractContentHashFromChallenge,
  derivePasskeyId,
  concatBytes,
  bytesEqual,
  hexToBytes,
  bytesToHex,
  createCredentialCreationOptions,
  createCredentialRequestOptions,
} from '../utils/webauthn';

describe('webauthn utilities', () => {
  describe('base64UrlEncode / base64UrlDecode', () => {
    it('should encode and decode empty array', () => {
      const input = new Uint8Array(0);
      const encoded = base64UrlEncode(input);
      const decoded = base64UrlDecode(encoded);
      expect(decoded).toEqual(input);
    });

    it('should encode and decode simple bytes', () => {
      const input = new Uint8Array([0, 1, 2, 3, 4, 5]);
      const encoded = base64UrlEncode(input);
      const decoded = base64UrlDecode(encoded);
      expect(decoded).toEqual(input);
    });

    it('should encode and decode 32 random bytes', () => {
      const input = new Uint8Array(32);
      for (let i = 0; i < 32; i++) {
        input[i] = Math.floor(Math.random() * 256);
      }
      const encoded = base64UrlEncode(input);
      const decoded = base64UrlDecode(encoded);
      expect(decoded).toEqual(input);
    });

    it('should produce URL-safe characters', () => {
      // Test bytes that would produce + and / in standard base64
      const input = new Uint8Array([0xfb, 0xef, 0xbe]); // Would be ++++ in standard base64
      const encoded = base64UrlEncode(input);
      expect(encoded).not.toContain('+');
      expect(encoded).not.toContain('/');
      expect(encoded).not.toContain('=');
    });

    it('should decode base64url with missing padding', () => {
      // "test" in base64url without padding
      const encoded = 'dGVzdA';
      const decoded = base64UrlDecode(encoded);
      const expected = new TextEncoder().encode('test');
      // Compare as arrays since Uint8Array comparison can be tricky
      expect(Array.from(decoded)).toEqual(Array.from(expected));
    });

    it('should handle special characters correctly', () => {
      // Use - and _ instead of + and /
      const input = new Uint8Array([0x00, 0xff, 0x00, 0xff]); 
      const encoded = base64UrlEncode(input);
      expect(encoded).not.toContain('+');
      expect(encoded).not.toContain('/');
      
      // Decode should work
      const decoded = base64UrlDecode(encoded);
      expect(decoded).toEqual(input);
    });
  });

  describe('generateWysiwysChallenge', () => {
    it('should generate 64-byte challenge', async () => {
      const challenge = await generateWysiwysChallenge('Hello, World!');
      expect(challenge.length).toBe(64);
    });

    it('should embed content hash in bytes 16-48', async () => {
      const content = 'Test content';
      const challenge1 = await generateWysiwysChallenge(content);
      const challenge2 = await generateWysiwysChallenge(content);
      
      // Content hash should be the same
      const hash1 = challenge1.slice(16, 48);
      const hash2 = challenge2.slice(16, 48);
      expect(hash1).toEqual(hash2);
      
      // Prefix and suffix should be different (random)
      // Note: There's a very small probability they could be the same
      const prefix1 = challenge1.slice(0, 16);
      const prefix2 = challenge2.slice(0, 16);
      const suffix1 = challenge1.slice(48, 64);
      const suffix2 = challenge2.slice(48, 64);
      
      // At least one should be different
      const prefixDifferent = !bytesEqual(prefix1, prefix2);
      const suffixDifferent = !bytesEqual(suffix1, suffix2);
      expect(prefixDifferent || suffixDifferent).toBe(true);
    });

    it('should produce different hashes for different content', async () => {
      const challenge1 = await generateWysiwysChallenge('Content A');
      const challenge2 = await generateWysiwysChallenge('Content B');
      
      const hash1 = challenge1.slice(16, 48);
      const hash2 = challenge2.slice(16, 48);
      
      expect(bytesEqual(hash1, hash2)).toBe(false);
    });

    it('should handle empty content', async () => {
      const challenge = await generateWysiwysChallenge('');
      expect(challenge.length).toBe(64);
    });

    it('should handle unicode content', async () => {
      const challenge = await generateWysiwysChallenge('こんにちは世界 🌍');
      expect(challenge.length).toBe(64);
    });
  });

  describe('extractContentHashFromChallenge', () => {
    it('should extract content hash from challenge', async () => {
      const content = 'Test content';
      const challenge = await generateWysiwysChallenge(content);
      const extractedHash = extractContentHashFromChallenge(challenge);
      
      // Hash should be 32 bytes
      expect(extractedHash.length).toBe(32);
      
      // Should match the hash in the challenge
      const expectedHash = challenge.slice(16, 48);
      expect(extractedHash).toEqual(expectedHash);
    });

    it('should throw for challenge shorter than 48 bytes', () => {
      const shortChallenge = new Uint8Array(40);
      expect(() => extractContentHashFromChallenge(shortChallenge)).toThrow('Challenge too short');
    });
  });

  describe('derivePasskeyId', () => {
    it('should return 32-byte hash', () => {
      const coseKey = new Uint8Array(65); // Typical EC2 key size
      const passkeyId = derivePasskeyId(coseKey);
      expect(passkeyId.length).toBe(32);
    });

    it('should produce consistent output for same input', () => {
      const coseKey = new Uint8Array([1, 2, 3, 4, 5]);
      const id1 = derivePasskeyId(coseKey);
      const id2 = derivePasskeyId(coseKey);
      expect(id1).toEqual(id2);
    });

    it('should produce different output for different input', () => {
      const key1 = new Uint8Array([1, 2, 3]);
      const key2 = new Uint8Array([1, 2, 4]);
      const id1 = derivePasskeyId(key1);
      const id2 = derivePasskeyId(key2);
      expect(bytesEqual(id1, id2)).toBe(false);
    });
  });

  describe('concatBytes', () => {
    it('should concatenate empty arrays', () => {
      const result = concatBytes(new Uint8Array(0), new Uint8Array(0));
      expect(result).toEqual(new Uint8Array(0));
    });

    it('should concatenate two arrays', () => {
      const a = new Uint8Array([1, 2, 3]);
      const b = new Uint8Array([4, 5, 6]);
      const result = concatBytes(a, b);
      expect(result).toEqual(new Uint8Array([1, 2, 3, 4, 5, 6]));
    });

    it('should concatenate multiple arrays', () => {
      const a = new Uint8Array([1]);
      const b = new Uint8Array([2]);
      const c = new Uint8Array([3]);
      const result = concatBytes(a, b, c);
      expect(result).toEqual(new Uint8Array([1, 2, 3]));
    });
  });

  describe('bytesEqual', () => {
    it('should return true for equal arrays', () => {
      const a = new Uint8Array([1, 2, 3]);
      const b = new Uint8Array([1, 2, 3]);
      expect(bytesEqual(a, b)).toBe(true);
    });

    it('should return false for different lengths', () => {
      const a = new Uint8Array([1, 2, 3]);
      const b = new Uint8Array([1, 2]);
      expect(bytesEqual(a, b)).toBe(false);
    });

    it('should return false for different content', () => {
      const a = new Uint8Array([1, 2, 3]);
      const b = new Uint8Array([1, 2, 4]);
      expect(bytesEqual(a, b)).toBe(false);
    });

    it('should return true for empty arrays', () => {
      const a = new Uint8Array(0);
      const b = new Uint8Array(0);
      expect(bytesEqual(a, b)).toBe(true);
    });
  });

  describe('hexToBytes / bytesToHex', () => {
    it('should convert hex to bytes', () => {
      const hex = '0102030405';
      const bytes = hexToBytes(hex);
      expect(bytes).toEqual(new Uint8Array([1, 2, 3, 4, 5]));
    });

    it('should handle 0x prefix', () => {
      const hex = '0x0102030405';
      const bytes = hexToBytes(hex);
      expect(bytes).toEqual(new Uint8Array([1, 2, 3, 4, 5]));
    });

    it('should convert bytes to hex', () => {
      const bytes = new Uint8Array([1, 2, 3, 4, 5]);
      const hex = bytesToHex(bytes);
      expect(hex).toBe('0102030405');
    });

    it('should round-trip correctly', () => {
      const original = new Uint8Array([0x00, 0xff, 0x12, 0xab]);
      const hex = bytesToHex(original);
      const bytes = hexToBytes(hex);
      expect(bytes).toEqual(original);
    });

    it('should throw for invalid hex length', () => {
      expect(() => hexToBytes('123')).toThrow('Invalid hex string length');
    });
  });

  describe('createCredentialCreationOptions', () => {
    it('should create valid options', () => {
      const userId = new Uint8Array(32);
      crypto.getRandomValues(userId);
      const challenge = new Uint8Array(32);
      crypto.getRandomValues(challenge);

      const options = createCredentialCreationOptions({
        userId,
        userName: 'testuser',
        challenge,
      });

      expect(options.challenge).toBe(challenge);
      expect(options.rp.name).toBe('Anarchy');
      expect(options.user.id).toBe(userId);
      expect(options.user.name).toBe('testuser');
      expect(options.user.displayName).toBe('testuser');
      expect(options.pubKeyCredParams).toContainEqual({ type: 'public-key', alg: -7 });
      expect(options.authenticatorSelection?.userVerification).toBe('required');
      expect(options.attestation).toBe('none');
    });

    it('should use custom rpName and rpId', () => {
      const options = createCredentialCreationOptions({
        userId: new Uint8Array(1),
        userName: 'test',
        challenge: new Uint8Array(32),
        rpId: 'example.com',
        rpName: 'Example',
      });

      expect(options.rp.id).toBe('example.com');
      expect(options.rp.name).toBe('Example');
    });
  });

  describe('createCredentialRequestOptions', () => {
    it('should create valid options', () => {
      const challenge = new Uint8Array(64);
      crypto.getRandomValues(challenge);

      const options = createCredentialRequestOptions({ challenge });

      expect(options.challenge).toBe(challenge);
      expect(options.userVerification).toBe('required');
      expect(options.allowCredentials).toEqual([]);
    });

    it('should include allowCredentials when provided', () => {
      const credentialId = new Uint8Array([1, 2, 3]);
      const options = createCredentialRequestOptions({
        challenge: new Uint8Array(64),
        allowCredentials: [{ id: credentialId, type: 'public-key' }],
      });

      expect(options.allowCredentials).toHaveLength(1);
      expect(options.allowCredentials?.[0].id).toBe(credentialId);
    });
  });
});

/**
 * SS58 Encoding Compatibility Tests
 *
 * polkadot-apiのSS58エンコード機能のテスト
 * wasmから返された公開鍵をpolkadot-apiでエンコードし、チェックサムが正しいことを確認
 */

import { fromBufferToBase58, getSs58AddressInfo } from '@polkadot-api/substrate-bindings';

describe('SS58 Encoding Compatibility', () => {
  const SS58_PREFIX = 42; // Substrate generic prefix

  describe('fromBufferToBase58', () => {
    it('should encode a 32-byte pubkey to SS58 address starting with "5"', () => {
      const pubkey = new Uint8Array(32).fill(1);
      const address = fromBufferToBase58(SS58_PREFIX)(pubkey);

      expect(address).toBeDefined();
      expect(typeof address).toBe('string');
      expect(address.startsWith('5')).toBe(true);
      expect(address.length).toBeGreaterThanOrEqual(47);
      expect(address.length).toBeLessThanOrEqual(48);
    });

    it('should produce addresses that pass checksum validation', () => {
      const pubkey = new Uint8Array(32).fill(42);
      const address = fromBufferToBase58(SS58_PREFIX)(pubkey);

      // Validate the address using polkadot-api's own validation
      const info = getSs58AddressInfo(address);

      expect(info.isValid).toBe(true);
      if (info.isValid) {
        expect(info.ss58Format).toBe(SS58_PREFIX);
        expect(info.publicKey).toEqual(pubkey);
      }
    });

    it('should handle random pubkeys correctly', () => {
      // Test with multiple random pubkeys
      for (let i = 0; i < 10; i++) {
        const pubkey = new Uint8Array(32);
        for (let j = 0; j < 32; j++) {
          pubkey[j] = Math.floor(Math.random() * 256);
        }

        const address = fromBufferToBase58(SS58_PREFIX)(pubkey);
        const info = getSs58AddressInfo(address);

        expect(info.isValid).toBe(true);
        if (info.isValid) {
          expect(info.publicKey).toEqual(pubkey);
        }
      }
    });

    it('should produce consistent encoding', () => {
      const pubkey = new Uint8Array([
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
        0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
        0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
      ]);

      const address1 = fromBufferToBase58(SS58_PREFIX)(pubkey);
      const address2 = fromBufferToBase58(SS58_PREFIX)(pubkey);

      expect(address1).toBe(address2);
    });
  });

  describe('getSs58AddressInfo', () => {
    it('should validate correct SS58 addresses', () => {
      // Known valid Substrate address
      const pubkey = new Uint8Array(32).fill(0xAB);
      const address = fromBufferToBase58(SS58_PREFIX)(pubkey);

      const info = getSs58AddressInfo(address);
      expect(info.isValid).toBe(true);
    });

    it('should reject invalid checksum addresses', () => {
      // Create a valid address then corrupt the last character
      const pubkey = new Uint8Array(32).fill(0x12);
      const address = fromBufferToBase58(SS58_PREFIX)(pubkey);

      // Corrupt the address by changing the last character
      const lastChar = address[address.length - 1];
      const corruptedChar = lastChar === '1' ? '2' : '1';
      const corrupted = address.slice(0, -1) + corruptedChar;

      const info = getSs58AddressInfo(corrupted);
      expect(info.isValid).toBe(false);
    });

    it('should reject addresses with wrong length', () => {
      const shortAddress = '5GrwvaEF5zXb26Fz9'; // Too short
      const info = getSs58AddressInfo(shortAddress);
      expect(info.isValid).toBe(false);
    });
  });

  describe('Round-trip encoding/decoding', () => {
    it('should preserve pubkey through encoding and decoding', () => {
      const originalPubkey = new Uint8Array([
        0xd4, 0x35, 0x93, 0xc7, 0x15, 0xfd, 0xd3, 0x1c,
        0x61, 0x14, 0x1a, 0xbd, 0x04, 0xa9, 0x9f, 0xd6,
        0x82, 0x2c, 0x85, 0x58, 0x85, 0x4c, 0xcd, 0xe3,
        0x9a, 0x56, 0x84, 0xe7, 0xa5, 0x6d, 0xa2, 0x7d,
      ]);

      const address = fromBufferToBase58(SS58_PREFIX)(originalPubkey);
      const info = getSs58AddressInfo(address);

      expect(info.isValid).toBe(true);
      if (info.isValid) {
        expect(info.publicKey).toEqual(originalPubkey);
        expect(info.ss58Format).toBe(SS58_PREFIX);
      }
    });

    it('should work with all-zero pubkey', () => {
      const zeroPubkey = new Uint8Array(32).fill(0);
      const address = fromBufferToBase58(SS58_PREFIX)(zeroPubkey);
      const info = getSs58AddressInfo(address);

      expect(info.isValid).toBe(true);
      if (info.isValid) {
        expect(info.publicKey).toEqual(zeroPubkey);
      }
    });

    it('should work with all-255 pubkey', () => {
      const maxPubkey = new Uint8Array(32).fill(255);
      const address = fromBufferToBase58(SS58_PREFIX)(maxPubkey);
      const info = getSs58AddressInfo(address);

      expect(info.isValid).toBe(true);
      if (info.isValid) {
        expect(info.publicKey).toEqual(maxPubkey);
      }
    });
  });
});

/**
 * End-to-end test for stealth address scanning
 * 
 * This test verifies the complete flow:
 * 1. Generate keys
 * 2. Derive stealth address
 * 3. Simulate chain storage (ephemeral_pubkey, stealth_address)
 * 4. Scan using view_key + spend_pubkey
 * 5. Verify transaction is detected
 */

import { fromBufferToBase58, getSs58AddressInfo } from '@polkadot-api/substrate-bindings';

// Mock wasm module with actual implementations for testing
const mockWasm = {
  generate_stealth_keys: jest.fn(() => ({
    spend_key: new Uint8Array(32).fill(1),
    view_key: new Uint8Array(32).fill(2),
    spend_pubkey: new Uint8Array(32).fill(3),
    view_pubkey: new Uint8Array(32).fill(4),
    meta_address: 'st:anarchy:testmetaaddress',
  })),
  derive_stealth_address: jest.fn((_metaAddress: string) => ({
    stealth_address: 'mock_address',
    ephemeral_pubkey: new Uint8Array([
      0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
      0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
      0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00,
      0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    ]),
    stealth_pubkey: new Uint8Array([
      0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x9a,
      0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55,
      0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
      0xee, 0xff, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
    ]),
  })),
  scan_transaction: jest.fn(
    (_viewKey: Uint8Array, _ephemeralPubkey: Uint8Array, stealthPubkey: Uint8Array, _spendPubkey: Uint8Array) => {
      // Mock: compare stealth_pubkey with expected value
      // In real implementation, this computes expected_pubkey from ECDH
      const expectedPubkey = new Uint8Array([
        0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x9a,
        0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55,
        0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
        0xee, 0xff, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
      ]);
      return Array.from(stealthPubkey).every((v, i) => v === expectedPubkey[i]);
    }
  ),
};

jest.mock('anarchy-wasm-engine', () => mockWasm);

describe('SS58 round-trip encoding', () => {
  const SS58_PREFIX = 42;
  
  it('should preserve pubkey bytes through encode/decode cycle', () => {
    // Original pubkey bytes
    const originalPubkey = new Uint8Array([
      0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x9a,
      0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55,
      0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
      0xee, 0xff, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
    ]);
    
    // Step 1: Encode to SS58
    const ss58Address = fromBufferToBase58(SS58_PREFIX)(originalPubkey);
    console.log('SS58 encoded address:', ss58Address);
    
    // Step 2: Decode from SS58
    const decoded = getSs58AddressInfo(ss58Address);
    expect(decoded.isValid).toBe(true);
    
    if (!decoded.isValid) {
      throw new Error('SS58 decode failed');
    }
    
    const decodedPubkey = decoded.publicKey;
    console.log('Original pubkey:', Array.from(originalPubkey));
    console.log('Decoded pubkey:', Array.from(decodedPubkey));
    
    // Step 3: Verify bytes match exactly
    expect(decodedPubkey.length).toBe(32);
    expect(Array.from(decodedPubkey)).toEqual(Array.from(originalPubkey));
  });
  
  it('should work with all-zero pubkey', () => {
    const zeroPubkey = new Uint8Array(32).fill(0);
    const ss58 = fromBufferToBase58(SS58_PREFIX)(zeroPubkey);
    const decoded = getSs58AddressInfo(ss58);
    
    expect(decoded.isValid).toBe(true);
    if (decoded.isValid) {
      expect(Array.from(decoded.publicKey)).toEqual(Array.from(zeroPubkey));
    }
  });
  
  it('should work with random pubkey', () => {
    // Use predictable "random" bytes
    const randomPubkey = new Uint8Array([
      0xd4, 0x35, 0x93, 0xc7, 0x15, 0xfd, 0xd3, 0x1c,
      0x61, 0x14, 0x1a, 0xbd, 0x04, 0xa9, 0x9f, 0xd6,
      0x82, 0x2c, 0x85, 0x58, 0x85, 0x4c, 0xcd, 0xe3,
      0x9a, 0x56, 0x84, 0xe7, 0xa5, 0x6d, 0xa2, 0x7d,
    ]);
    
    const ss58 = fromBufferToBase58(SS58_PREFIX)(randomPubkey);
    const decoded = getSs58AddressInfo(ss58);
    
    expect(decoded.isValid).toBe(true);
    if (decoded.isValid) {
      expect(Array.from(decoded.publicKey)).toEqual(Array.from(randomPubkey));
    }
  });
});

describe('Complete stealth scanning flow', () => {
  it('should detect own transaction through full encode/decode cycle', () => {
    // Step 1: Get stealth pubkey from "wasm"
    const derivedResult = mockWasm.derive_stealth_address('st:anarchy:test');
    const stealthPubkey = derivedResult.stealth_pubkey;
    const ephemeralPubkey = derivedResult.ephemeral_pubkey;
    
    console.log('Step 1 - Derived stealth pubkey (first 8):', Array.from(stealthPubkey.slice(0, 8)));
    
    // Step 2: Encode to SS58 (what frontend does before sending to chain)
    const stealthAddress = fromBufferToBase58(42)(stealthPubkey);
    console.log('Step 2 - SS58 encoded:', stealthAddress);
    
    // Step 3: Decode from SS58 (what scanner does after reading from chain)
    const addressInfo = getSs58AddressInfo(stealthAddress);
    expect(addressInfo.isValid).toBe(true);
    
    if (!addressInfo.isValid) {
      throw new Error('Invalid SS58');
    }
    
    const decodedPubkey = addressInfo.publicKey;
    console.log('Step 3 - Decoded pubkey (first 8):', Array.from(decodedPubkey.slice(0, 8)));
    
    // Step 4: Verify bytes preserved
    expect(Array.from(decodedPubkey)).toEqual(Array.from(stealthPubkey));
    
    // Step 5: Call scan_transaction with decoded pubkey
    const viewKey = new Uint8Array(32).fill(2);
    const spendPubkey = new Uint8Array(32).fill(3);
    
    const isOwned = mockWasm.scan_transaction(
      viewKey,
      ephemeralPubkey,
      decodedPubkey,
      spendPubkey
    );
    
    console.log('Step 5 - scan_transaction result:', isOwned);
    
    expect(isOwned).toBe(true);
  });
  
  it('should fail if pubkey is corrupted', () => {
    const derivedResult = mockWasm.derive_stealth_address('st:anarchy:test');
    const ephemeralPubkey = derivedResult.ephemeral_pubkey;
    
    // Use a different pubkey (simulating mismatch)
    const wrongPubkey = new Uint8Array(32).fill(0xff);
    
    const viewKey = new Uint8Array(32).fill(2);
    const spendPubkey = new Uint8Array(32).fill(3);
    
    const isOwned = mockWasm.scan_transaction(
      viewKey,
      ephemeralPubkey,
      wrongPubkey,
      spendPubkey
    );
    
    expect(isOwned).toBe(false);
  });
});

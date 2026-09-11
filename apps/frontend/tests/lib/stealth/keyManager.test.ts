/**
 * StealthKeyManager Unit Tests
 * 
 * セッション鍵管理のテスト
 */

import { StealthKeyManager } from '../../../src/lib/stealth/keyManager';
import type { StealthKeyPair } from '../../../src/lib/stealth/types';

// 永続化層はモック (本体は src/lib/account/__tests__/sessionStore.test.ts で検証)
const mockSaveStealth = jest.fn().mockResolvedValue(undefined);
const mockClearStealth = jest.fn().mockResolvedValue(undefined);
jest.mock('@/lib/account/sessionStore', () => ({
  saveStealthKeys: (...a: unknown[]) => mockSaveStealth(...a),
  clearStealthKeys: (...a: unknown[]) => mockClearStealth(...a),
}));

// Mock fetch for wasm initialization
global.fetch = jest.fn().mockResolvedValue({
  ok: true,
  arrayBuffer: () => Promise.resolve(new ArrayBuffer(0)),
});

// Mock WebAssembly.Module
global.WebAssembly = {
  ...global.WebAssembly,
  Module: class MockModule {
    constructor() {
      // Mock module
    }
  } as unknown as typeof WebAssembly.Module,
};

// Mock wasm-engine functions
jest.mock('anarchy-wasm-engine', () => ({
  initSync: jest.fn(),
  generate_stealth_keys: jest.fn(() => ({
    spend_key: new Uint8Array(32).fill(1),
    view_key: new Uint8Array(32).fill(2),
    spend_pubkey: new Uint8Array(32).fill(3),
    view_pubkey: new Uint8Array(32).fill(4),
    meta_address: 'st:anarchy:test-meta-address',
  })),
  restore_stealth_keys: jest.fn((spendPriv: Uint8Array, scanPriv: Uint8Array) => ({
    spend_key: new Uint8Array(spendPriv),
    view_key: new Uint8Array(scanPriv),
    spend_pubkey: new Uint8Array(32).fill(3),
    view_pubkey: new Uint8Array(32).fill(4),
    meta_address: 'st:anarchy:test-meta-address',
  })),
  encrypt_backup: jest.fn((spendKey: Uint8Array, viewKey: Uint8Array, password: string) => {
    // Simple mock: concatenate with password marker
    const marker = new TextEncoder().encode('ENCRYPTED');
    const result = new Uint8Array(marker.length + 64);
    result.set(marker, 0);
    result.set(spendKey, marker.length);
    result.set(viewKey, marker.length + 32);
    return result;
  }),
  decrypt_backup: jest.fn((encrypted: Uint8Array, password: string) => {
    // Mock: return a key pair-like object with properties (not methods)
    const marker = new TextEncoder().encode('ENCRYPTED');
    if (encrypted.slice(0, marker.length).every((v, i) => v === marker[i])) {
      return {
        spend_key: encrypted.slice(marker.length, marker.length + 32),
        view_key: encrypted.slice(marker.length + 32, marker.length + 64),
        spend_pubkey: new Uint8Array(32).fill(3),
        view_pubkey: new Uint8Array(32).fill(4),
        meta_address: 'st:anarchy:test-meta-address',
      };
    }
    throw new Error('Decryption failed: invalid password');
  }),
}));

describe('StealthKeyManager', () => {
  let manager: StealthKeyManager;

  beforeEach(() => {
    jest.clearAllMocks();
    manager = new StealthKeyManager();
  });

  afterEach(() => {
    manager.destroy();
  });

  describe('generateKeys', () => {
    it('should generate new stealth key pair', async () => {
      const keyPair = await manager.generateKeys();

      expect(keyPair).toBeDefined();
      expect(keyPair.spendKey).toHaveLength(32);
      expect(keyPair.viewKey).toHaveLength(32);
      expect(keyPair.spendPubkey).toHaveLength(32);
      expect(keyPair.viewPubkey).toHaveLength(32);
      expect(keyPair.metaAddress).toBe('st:anarchy:test-meta-address');
    });

    it('should store keys in memory after generation', async () => {
      await manager.generateKeys();

      expect(manager.hasKeys()).toBe(true);
      expect(manager.getMetaAddress()).toBe('st:anarchy:test-meta-address');
    });

    it('should not regenerate if keys already exist', async () => {
      const first = await manager.generateKeys();
      const second = await manager.generateKeys();

      // Should return same keys (cached)
      expect(first.metaAddress).toBe(second.metaAddress);
    });
  });

  describe('exportBackup', () => {
    it('should export encrypted backup', async () => {
      await manager.generateKeys();
      
      const encrypted = await manager.exportBackup('test-password');

      expect(encrypted).toBeInstanceOf(Uint8Array);
      expect(encrypted.length).toBeGreaterThan(0);
    });

    it('should throw if no keys loaded', async () => {
      await expect(manager.exportBackup('password')).rejects.toThrow(
        'No key pair loaded'
      );
    });
  });

  describe('importFromBackup', () => {
    it('should import keys from valid backup', async () => {
      // Generate keys on first manager
      await manager.generateKeys();
      const encrypted = await manager.exportBackup('test-password');

      // Create new manager and import
      const newManager = new StealthKeyManager();
      await newManager.importFromBackup(encrypted, 'test-password');

      expect(newManager.hasKeys()).toBe(true);
      expect(newManager.getMetaAddress()).toBe('st:anarchy:test-meta-address');
      
      newManager.destroy();
    });

    it('should throw on invalid password', async () => {
      await manager.generateKeys();
      const encrypted = await manager.exportBackup('correct-password');

      const newManager = new StealthKeyManager();
      
      // Mock will throw for wrong password
      const { decrypt_backup } = jest.requireMock('anarchy-wasm-engine');
      decrypt_backup.mockImplementationOnce(() => {
        throw new Error('Decryption failed');
      });

      await expect(
        newManager.importFromBackup(encrypted, 'wrong-password')
      ).rejects.toThrow();
      
      newManager.destroy();
    });
  });

  describe('destroy', () => {
    it('should clear keys from memory', async () => {
      await manager.generateKeys();
      expect(manager.hasKeys()).toBe(true);

      manager.destroy();
      
      expect(manager.hasKeys()).toBe(false);
      expect(manager.getMetaAddress()).toBeNull();
    });

    it('should be safe to call multiple times', () => {
      manager.destroy();
      manager.destroy();
      // Should not throw
      expect(manager.hasKeys()).toBe(false);
    });
  });

  describe('persistence (bindAccount)', () => {
    const ALICE = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY';

    it('account 未 bind なら生成しても保存しない', async () => {
      await manager.generateKeys();
      expect(mockSaveStealth).not.toHaveBeenCalled();
    });

    it('bind 済みなら generateKeys で scan/spend 秘密鍵を account 名義で保存する', async () => {
      manager.bindAccount(ALICE);
      const kp = await manager.generateKeys();
      expect(mockSaveStealth).toHaveBeenCalledWith(ALICE, {
        scanPriv: kp.viewKey,
        spendPriv: kp.spendKey,
      });
    });

    it('bind 済みなら loadFromBackup でも保存する (backup import / 復帰)', async () => {
      manager.bindAccount(ALICE);
      const scan = new Uint8Array(32).fill(9);
      const spend = new Uint8Array(32).fill(8);
      await manager.loadFromBackup(scan, spend);
      expect(mockSaveStealth).toHaveBeenCalledWith(ALICE, expect.objectContaining({ scanPriv: expect.any(Uint8Array), spendPriv: expect.any(Uint8Array) }));
    });

    it('destroy はメモリだけ消して保存分は残す (beforeunload 用)', async () => {
      manager.bindAccount(ALICE);
      await manager.generateKeys();
      manager.destroy();
      expect(manager.hasKeys()).toBe(false);
      expect(mockClearStealth).not.toHaveBeenCalled();
    });

    it('discard はメモリと保存分の両方を消す (鍵破棄ボタン)', async () => {
      manager.bindAccount(ALICE);
      await manager.generateKeys();
      await manager.discard();
      expect(manager.hasKeys()).toBe(false);
      expect(mockClearStealth).toHaveBeenCalledWith(ALICE);
    });

    it('bindAccount(null) 以降は保存しない', async () => {
      manager.bindAccount(ALICE);
      manager.bindAccount(null);
      await manager.generateKeys();
      expect(mockSaveStealth).not.toHaveBeenCalled();
    });
  });

  describe('getViewKey', () => {
    it('should return view key when loaded', async () => {
      await manager.generateKeys();
      
      const viewKey = manager.getViewKey();
      expect(viewKey).toHaveLength(32);
    });

    it('should return null when no keys loaded', () => {
      expect(manager.getViewKey()).toBeNull();
    });
  });

  describe('getSpendKey', () => {
    it('should return spend key when loaded', async () => {
      await manager.generateKeys();
      
      const spendKey = manager.getSpendKey();
      expect(spendKey).toHaveLength(32);
    });

    it('should return null when no keys loaded', () => {
      expect(manager.getSpendKey()).toBeNull();
    });
  });
});

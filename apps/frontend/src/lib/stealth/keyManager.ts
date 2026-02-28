/**
 * StealthKeyManager - Session Key Management
 * 
 * ステルス鍵のセッション管理。
 * 鍵はメモリにのみ保持され、ブラウザ/localStorage/IndexedDBには保存しない。
 * セッション終了時（beforeunload）に自動破棄。
 */

import type { StealthKeyPair } from './types';

// Wasm functions will be imported dynamically to support both sync and async loading
type WasmModule = typeof import('anarchy-wasm-engine');
let wasmModule: WasmModule | null = null;

/**
 * Load and initialize wasm module lazily
 * 
 * Next.jsのバンドリングではimport.meta.urlが正しく解決されない場合があるため、
 * public/wasmフォルダにコピーされたWASMファイルを明示的にfetchして初期化する
 */
async function getWasm(): Promise<WasmModule> {
  if (wasmModule) {
    return wasmModule;
  }
  
  try {
    const module = await import('anarchy-wasm-engine');
    
    // public/wasm/に配置されたWASMファイルを使用
    const wasmUrl = new URL('/wasm/anarchy_wasm_engine_bg.wasm', window.location.origin);
    const response = await fetch(wasmUrl);
    
    if (!response.ok) {
      throw new Error(`Failed to fetch WASM: ${response.status} ${response.statusText}`);
    }
    
    const wasmBytes = await response.arrayBuffer();
    module.initSync({ module: new WebAssembly.Module(wasmBytes) });
    
    wasmModule = module;
    console.log('[StealthKeyManager] Wasm module initialized');
    return wasmModule;
  } catch (error) {
    console.error('[StealthKeyManager] Failed to initialize Wasm:', error);
    throw error;
  }
}

/**
 * Secure wipe utility - overwrite buffer with zeros
 */
function secureWipe(buffer: Uint8Array): void {
  if (buffer) {
    buffer.fill(0);
  }
}

/**
 * StealthKeyManager - manages stealth keys in session memory
 */
export class StealthKeyManager {
  private keyPair: StealthKeyPair | null = null;
  private beforeUnloadHandler: (() => void) | null = null;

  /**
   * Generate new stealth key pair
   * @returns Generated key pair
   */
  async generateKeys(): Promise<StealthKeyPair> {
    // Return existing keys if already generated
    if (this.keyPair) {
      return this.keyPair;
    }

    const wasm = await getWasm();
    const wasmKeys = wasm.generate_stealth_keys();

    this.keyPair = {
      spendKey: new Uint8Array(wasmKeys.spend_key),
      viewKey: new Uint8Array(wasmKeys.view_key),
      spendPubkey: new Uint8Array(wasmKeys.spend_pubkey),
      viewPubkey: new Uint8Array(wasmKeys.view_pubkey),
      metaAddress: wasmKeys.meta_address,
      createdAt: Date.now(),
    };

    // Register cleanup handler
    this.registerCleanupHandler();

    return this.keyPair;
  }

  /**
   * Import keys from encrypted backup
   * @param encryptedBackup - Encrypted backup data
   * @param password - User password
   */
  async importFromBackup(
    encryptedBackup: Uint8Array,
    password: string
  ): Promise<void> {
    const wasm = await getWasm();
    
    // Decrypt backup
    const wasmKeys = wasm.decrypt_backup(encryptedBackup, password);

    this.keyPair = {
      spendKey: new Uint8Array(wasmKeys.spend_key),
      viewKey: new Uint8Array(wasmKeys.view_key),
      spendPubkey: new Uint8Array(wasmKeys.spend_pubkey),
      viewPubkey: new Uint8Array(wasmKeys.view_pubkey),
      metaAddress: wasmKeys.meta_address,
      createdAt: Date.now(),
    };

    // Register cleanup handler
    this.registerCleanupHandler();
  }

  /**
   * Export keys as encrypted backup
   * @param password - User password for encryption
   * @returns Encrypted backup data
   */
  async exportBackup(password: string): Promise<Uint8Array> {
    if (!this.keyPair) {
      throw new Error('No key pair loaded');
    }

    const wasm = await getWasm();
    
    const encrypted = wasm.encrypt_backup(
      this.keyPair.spendKey,
      this.keyPair.viewKey,
      password
    );

    return new Uint8Array(encrypted);
  }

  /**
   * Check if keys are loaded
   */
  hasKeys(): boolean {
    return this.keyPair !== null;
  }

  /**
   * Get meta address
   */
  getMetaAddress(): string | null {
    return this.keyPair?.metaAddress ?? null;
  }

  /**
   * Get view key (for scanning)
   */
  getViewKey(): Uint8Array | null {
    return this.keyPair?.viewKey ?? null;
  }

  /**
   * Get spend key (for claiming)
   */
  getSpendKey(): Uint8Array | null {
    return this.keyPair?.spendKey ?? null;
  }

  /**
   * Get spend public key (for verification)
   */
  getSpendPubkey(): Uint8Array | null {
    return this.keyPair?.spendPubkey ?? null;
  }

  /**
   * Get view public key
   */
  getViewPubkey(): Uint8Array | null {
    return this.keyPair?.viewPubkey ?? null;
  }

  /**
   * Get full key pair (for internal use only)
   */
  getKeyPair(): StealthKeyPair | null {
    return this.keyPair;
  }

  /**
   * Destroy keys from memory with secure wipe
   */
  destroy(): void {
    if (this.keyPair) {
      // Secure wipe all sensitive data
      secureWipe(this.keyPair.spendKey);
      secureWipe(this.keyPair.viewKey);
      secureWipe(this.keyPair.spendPubkey);
      secureWipe(this.keyPair.viewPubkey);
      this.keyPair = null;
    }

    // Unregister cleanup handler
    this.unregisterCleanupHandler();
  }

  /**
   * Register beforeunload handler for key destruction
   */
  private registerCleanupHandler(): void {
    if (typeof window !== 'undefined' && !this.beforeUnloadHandler) {
      this.beforeUnloadHandler = () => this.destroy();
      window.addEventListener('beforeunload', this.beforeUnloadHandler);
    }
  }

  /**
   * Unregister beforeunload handler
   */
  private unregisterCleanupHandler(): void {
    if (typeof window !== 'undefined' && this.beforeUnloadHandler) {
      window.removeEventListener('beforeunload', this.beforeUnloadHandler);
      this.beforeUnloadHandler = null;
    }
  }
}

/**
 * Singleton instance for global use
 */
export const stealthKeyManager = new StealthKeyManager();

/**
 * Derive stealth address from a meta-address
 * This function properly initializes the wasm module before use
 * Uses polkadot-api's SS58 encoding for compatibility with the chain
 */
export async function deriveStealthAddress(metaAddress: string): Promise<{
  stealthAddress: string;
  ephemeralPubkey: Uint8Array;
  stealthPubkey: Uint8Array;
}> {
  console.log('[deriveStealthAddress] Input:', metaAddress);
  const wasm = await getWasm();
  try {
    // Parse the metaAddress to log the pubkeys being used
    const parsed = wasm.parse_meta_address(metaAddress);
    console.log('[deriveStealthAddress] Parsed spend_pubkey (first 8):', Array.from(new Uint8Array(parsed.spend_pubkey).slice(0, 8)));
    console.log('[deriveStealthAddress] Parsed view_pubkey (first 8):', Array.from(new Uint8Array(parsed.view_pubkey).slice(0, 8)));
    
    const result = wasm.derive_stealth_address(metaAddress);
    
    // Get stealth pubkey from wasm result
    const stealthPubkey = new Uint8Array(result.stealth_pubkey);
    const ephemeralPubkey = new Uint8Array(result.ephemeral_pubkey);
    console.log('[deriveStealthAddress] Ephemeral pubkey (first 8):', Array.from(ephemeralPubkey.slice(0, 8)));
    console.log('[deriveStealthAddress] Stealth pubkey (first 8):', Array.from(stealthPubkey.slice(0, 8)));
    
    // Use polkadot-api's SS58 encoder for compatibility
    const { fromBufferToBase58, getSs58AddressInfo } = await import('@polkadot-api/substrate-bindings');
    const stealthAddress = fromBufferToBase58(42)(stealthPubkey);
    
    console.log('[deriveStealthAddress] Stealth address (SS58):', stealthAddress);
    
    // Verify the address is valid before returning
    const validation = getSs58AddressInfo(stealthAddress);
    if (!validation.isValid) {
      console.error('[deriveStealthAddress] Generated invalid SS58 address!');
      throw new Error('Generated invalid SS58 address');
    }
    
    return {
      stealthAddress,
      ephemeralPubkey,
      stealthPubkey,
    };
  } catch (error) {
    console.error('[deriveStealthAddress] Error:', error);
    throw error;
  }
}

/**
 * Stealth Signer
 * 
 * T068 [US4] Create stealth signer using derived private key
 * 
 * ステルスアドレスから導出した秘密鍵で署名するための機能
 * wasm-engine の Ed25519 署名機能を直接使用
 */

import { DetectedStealthBalance } from './types';
import type { PolkadotSigner } from 'polkadot-api/signer';

// デフォルトのwasm-engineモジュールをインポート
let wasmModule: typeof import('anarchy-wasm-engine') | null = null;

/**
 * wasmモジュールを動的にロード
 */
async function getWasmModule() {
  if (!wasmModule) {
    wasmModule = await import('anarchy-wasm-engine');
  }
  return wasmModule;
}

/**
 * ステルス秘密鍵の導出パラメータ
 */
export interface DeriveKeyParams {
  /** Spend秘密鍵 */
  spendKey: Uint8Array;
  /** View秘密鍵 */
  viewKey: Uint8Array;
  /** エフェメラル公開鍵 */
  ephemeralPubkey: Uint8Array;
}

/**
 * ステルス秘密鍵を導出
 */
export async function deriveStealthPrivateKey(params: DeriveKeyParams): Promise<Uint8Array> {
  const wasm = await getWasmModule();
  return wasm.derive_stealth_private_key(
    params.spendKey,
    params.viewKey,
    params.ephemeralPubkey
  );
}

/**
 * 検出済み残高からステルス秘密鍵を導出
 */
export async function deriveKeyFromBalance(
  balance: DetectedStealthBalance,
  spendKey: Uint8Array,
  viewKey: Uint8Array
): Promise<Uint8Array> {
  return deriveStealthPrivateKey({
    spendKey,
    viewKey,
    ephemeralPubkey: balance.ephemeralPubkey,
  });
}

/**
 * ステルス署名者を作成
 * 
 * 注意: この関数は署名に必要な秘密鍵をメモリに保持します
 * 使用後は適切にメモリをクリアしてください
 */
export async function createStealthSigner(
  privateKey: Uint8Array
): Promise<StealthSigner> {
  return new StealthSigner(privateKey);
}

/**
 * ステルス署名者クラス
 * 
 * wasm-engine の Ed25519 署名機能を使用して署名を実行
 * 拡張秘密鍵フォーマット (64バイト: scalar || nonce_prefix) をサポート
 */
export class StealthSigner {
  private expandedKey: Uint8Array;
  private destroyed: boolean = false;
  private cachedPublicKey: Uint8Array | null = null;
  private cachedAddress: string | null = null;

  constructor(expandedKey: Uint8Array) {
    if (expandedKey.length !== 64) {
      throw new Error(`Invalid expanded key length: expected 64, got ${expandedKey.length}`);
    }
    this.expandedKey = new Uint8Array(expandedKey);
  }

  /**
   * メッセージに署名
   */
  async sign(message: Uint8Array): Promise<Uint8Array> {
    if (this.destroyed) {
      throw new Error('Signer has been destroyed');
    }
    
    const wasm = await getWasmModule();
    return wasm.stealth_sign(this.expandedKey, message);
  }

  /**
   * 公開鍵を取得
   */
  async getPublicKey(): Promise<Uint8Array> {
    if (this.destroyed) {
      throw new Error('Signer has been destroyed');
    }
    
    if (!this.cachedPublicKey) {
      const wasm = await getWasmModule();
      this.cachedPublicKey = wasm.stealth_get_public_key(this.expandedKey);
    }
    return this.cachedPublicKey;
  }

  /**
   * SS58アドレスを取得
   */
  async getAddress(): Promise<string> {
    if (this.destroyed) {
      throw new Error('Signer has been destroyed');
    }
    
    if (!this.cachedAddress) {
      const publicKey = await this.getPublicKey();
      // wasmのSS58エンコード関数を使用（一貫性のため）
      const wasm = await getWasmModule();
      this.cachedAddress = wasm.pubkey_to_ss58_address(publicKey);
    }
    return this.cachedAddress;
  }

  /**
   * Polkadot API互換のSignerを取得
   * 
   * polkadot-apiのトランザクション送信に使用可能
   */
  async getPolkadotSigner(): Promise<PolkadotSigner> {
    if (this.destroyed) {
      throw new Error('Signer has been destroyed');
    }
    
    // Dynamic import to avoid ESM issues in Jest
    const { getPolkadotSigner } = await import('polkadot-api/signer');
    
    const publicKey = await this.getPublicKey();
    return getPolkadotSigner(
      publicKey,
      'Ed25519',
      async (input: Uint8Array) => {
        return this.sign(input);
      }
    );
  }

  /**
   * 署名者を破棄し、秘密鍵をメモリからクリア
   */
  destroy(): void {
    if (!this.destroyed) {
      // 秘密鍵をゼロで上書き
      this.expandedKey.fill(0);
      this.cachedPublicKey = null;
      this.cachedAddress = null;
      this.destroyed = true;
    }
  }

  /**
   * 署名者が有効かどうか
   */
  isValid(): boolean {
    return !this.destroyed;
  }
}

/**
 * 複数残高用の署名者マップを作成
 */
export async function createSignerMap(
  balances: DetectedStealthBalance[],
  spendKey: Uint8Array,
  viewKey: Uint8Array
): Promise<Map<string, StealthSigner>> {
  const map = new Map<string, StealthSigner>();
  
  for (const balance of balances) {
    const privateKey = await deriveKeyFromBalance(balance, spendKey, viewKey);
    const signer = await createStealthSigner(privateKey);
    map.set(balance.stealthAddress, signer);
  }
  
  return map;
}

/**
 * 署名者マップをクリーンアップ
 */
export function destroySignerMap(map: Map<string, StealthSigner>): void {
  for (const signer of map.values()) {
    signer.destroy();
  }
  map.clear();
}

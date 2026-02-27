/**
 * Stealth Signer
 * 
 * T068 [US4] Create stealth signer using derived private key
 * 
 * ステルスアドレスから導出した秘密鍵で署名するための機能
 */

import { DetectedStealthBalance } from './types';

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
  // ed25519署名用のキーペアを生成
  // Polkadot SDKはsr25519を使用するが、ステルスアドレスではed25519を使用
  const wasm = await getWasmModule();
  
  return new StealthSigner(privateKey, wasm);
}

/**
 * ステルス署名者クラス
 */
export class StealthSigner {
  private privateKey: Uint8Array;
  private wasmModule: typeof import('anarchy-wasm-engine');
  private destroyed: boolean = false;

  constructor(
    privateKey: Uint8Array,
    wasmModule: typeof import('anarchy-wasm-engine')
  ) {
    this.privateKey = new Uint8Array(privateKey);
    this.wasmModule = wasmModule;
  }

  /**
   * メッセージに署名
   */
  async sign(message: Uint8Array): Promise<Uint8Array> {
    if (this.destroyed) {
      throw new Error('Signer has been destroyed');
    }
    
    // wasm-engineのed25519_sign関数を呼び出す
    // TODO: wasm-engineにed25519_sign関数を追加する必要あり
    // 暫定的にplaceholder実装
    const wasmAny = this.wasmModule as Record<string, unknown>;
    if (typeof wasmAny.ed25519_sign === 'function') {
      return (wasmAny.ed25519_sign as (key: Uint8Array, msg: Uint8Array) => Uint8Array)(
        this.privateKey, message
      );
    }
    // Fallback: 署名機能が未実装の場合
    console.warn('[StealthSigner] ed25519_sign not available in wasm-engine');
    return new Uint8Array(64);
  }

  /**
   * 公開鍵を取得
   */
  async getPublicKey(): Promise<Uint8Array> {
    if (this.destroyed) {
      throw new Error('Signer has been destroyed');
    }
    
    // ed25519の公開鍵を導出
    // TODO: wasm-engineにed25519_pubkey関数を追加する必要あり
    const wasmAny = this.wasmModule as Record<string, unknown>;
    if (typeof wasmAny.ed25519_pubkey === 'function') {
      return (wasmAny.ed25519_pubkey as (key: Uint8Array) => Uint8Array)(this.privateKey);
    }
    // Fallback: 公開鍵導出が未実装の場合
    console.warn('[StealthSigner] ed25519_pubkey not available in wasm-engine');
    return new Uint8Array(32);
  }

  /**
   * 署名者を破棄し、秘密鍵をメモリからクリア
   */
  destroy(): void {
    if (!this.destroyed) {
      // 秘密鍵をゼロで上書き
      this.privateKey.fill(0);
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

/**
 * Stealth Module - Web Worker Entry Point
 * 
 * メインスレッドをブロックせずに暗号処理を実行するWeb Worker
 */

import type { WorkerMessage, WorkerResponse, StealthKeyPair, DetectedStealthBalance, ScanProgress } from './types';

// Wasm モジュールの動的インポート
let wasmModule: typeof import('anarchy-wasm-engine') | null = null;

/**
 * Wasmモジュールを初期化
 * 
 * Worker内ではimport.meta.urlが正しく解決されないため、
 * public/wasmフォルダにコピーされたWASMファイルを使用
 */
async function initWasm(): Promise<void> {
  if (wasmModule) return;
  
  try {
    const module = await import('anarchy-wasm-engine');
    
    // public/wasm/に配置されたWASMファイルを使用
    const wasmUrl = new URL('/wasm/anarchy_wasm_engine_bg.wasm', self.location.origin);
    const response = await fetch(wasmUrl);
    
    if (!response.ok) {
      throw new Error(`Failed to fetch WASM: ${response.status} ${response.statusText}`);
    }
    
    const wasmBytes = await response.arrayBuffer();
    module.initSync({ module: new WebAssembly.Module(wasmBytes) });
    
    wasmModule = module;
    console.log('[StealthWorker] Wasm module initialized');
  } catch (error) {
    console.error('[StealthWorker] Failed to initialize Wasm:', error);
    throw new Error(`Failed to load Wasm module: ${error}`);
  }
}

/**
 * メッセージハンドラー
 */
self.onmessage = async (event: MessageEvent<WorkerMessage>) => {
  const message = event.data;
  
  try {
    switch (message.type) {
      case 'init':
        await initWasm();
        postResponse({ type: 'initialized' });
        break;
        
      case 'generateKeys':
        await initWasm();
        const keys = generateKeys();
        postResponse({ type: 'keysGenerated', keys });
        break;
        
      case 'deriveAddress':
        await initWasm();
        const result = deriveAddress(message.metaAddress);
        postResponse({ type: 'addressDerived', result });
        break;
        
      case 'scan':
        await initWasm();
        await scanBlocks(
          message.viewKey,
          message.spendPubkey,
          message.startBlock,
          message.endBlock
        );
        break;
        
      case 'encryptBackup':
        await initWasm();
        const encrypted = encryptBackup(
          message.spendKey,
          message.viewKey,
          message.password
        );
        postResponse({ type: 'backupEncrypted', data: encrypted });
        break;
        
      case 'decryptBackup':
        await initWasm();
        const decryptedKeys = decryptBackup(message.encrypted, message.password);
        postResponse({ type: 'backupDecrypted', keys: decryptedKeys });
        break;
        
      default:
        postResponse({ type: 'error', message: `Unknown message type` });
    }
  } catch (error) {
    postResponse({ 
      type: 'error', 
      message: error instanceof Error ? error.message : 'Unknown error' 
    });
  }
};

/**
 * レスポンスを送信
 */
function postResponse(response: WorkerResponse): void {
  self.postMessage(response);
}

/**
 * 鍵ペアを生成
 */
function generateKeys(): StealthKeyPair {
  if (!wasmModule) throw new Error('Wasm not initialized');
  
  const wasmKeys = wasmModule.generate_stealth_keys();
  
  return {
    spendKey: new Uint8Array(wasmKeys.spend_key),
    viewKey: new Uint8Array(wasmKeys.view_key),
    spendPubkey: new Uint8Array(wasmKeys.spend_pubkey),
    viewPubkey: new Uint8Array(wasmKeys.view_pubkey),
    metaAddress: wasmKeys.meta_address,
    createdAt: Date.now(),
  };
}

/**
 * ステルスアドレスを導出
 */
function deriveAddress(metaAddress: string): { stealthAddress: string; ephemeralPubkey: Uint8Array } {
  if (!wasmModule) throw new Error('Wasm not initialized');
  
  const result = wasmModule.derive_stealth_address(metaAddress);
  
  return {
    stealthAddress: result.stealth_address,
    ephemeralPubkey: new Uint8Array(result.ephemeral_pubkey),
  };
}

/**
 * ブロックをスキャンして自分宛のトランザクションを検出
 * 
 * Note: PAPI はメインスレッドでのみ使用可能なため、
 * エフェメラル公開鍵データはメッセージ経由で受け取る必要がある。
 * このワーカー関数は、メインスレッドから渡されたエフェメラル公開鍵を処理する。
 */
async function scanBlocks(
  viewKey: Uint8Array,
  spendPubkey: Uint8Array,
  startBlock: number,
  endBlock: number
): Promise<void> {
  if (!wasmModule) throw new Error('Wasm not initialized');
  
  // Worker内では直接ブロックチェーンにアクセスできないため、
  // 進捗のみを報告し、実際のスキャンはメインスレッドで行う
  const progress: ScanProgress = {
    currentBlock: startBlock,
    targetBlock: endBlock,
    percentage: 0,
    detectedCount: 0,
  };
  postResponse({ type: 'scanProgress', progress });
  
  // メインスレッドにスキャン開始を通知
  postResponse({ 
    type: 'scanReady', 
    viewKey: Array.from(viewKey),
    spendPubkey: Array.from(spendPubkey),
    startBlock,
    endBlock 
  });
}

/**
 * バックアップを暗号化
 */
function encryptBackup(
  spendKey: Uint8Array,
  viewKey: Uint8Array,
  password: string
): Uint8Array {
  if (!wasmModule) throw new Error('Wasm not initialized');
  
  const encrypted = wasmModule.encrypt_backup(spendKey, viewKey, password);
  return new Uint8Array(encrypted);
}

/**
 * バックアップを復号
 */
function decryptBackup(encrypted: Uint8Array, password: string): StealthKeyPair {
  if (!wasmModule) throw new Error('Wasm not initialized');
  
  const wasmKeys = wasmModule.decrypt_backup(encrypted, password);
  
  return {
    spendKey: new Uint8Array(wasmKeys.spend_key),
    viewKey: new Uint8Array(wasmKeys.view_key),
    spendPubkey: new Uint8Array(wasmKeys.spend_pubkey),
    viewPubkey: new Uint8Array(wasmKeys.view_pubkey),
    metaAddress: wasmKeys.meta_address,
    createdAt: Date.now(),
  };
}

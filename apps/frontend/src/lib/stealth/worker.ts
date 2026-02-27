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
 */
async function initWasm(): Promise<void> {
  if (wasmModule) return;
  
  try {
    wasmModule = await import('anarchy-wasm-engine');
    // Wasm初期化が必要な場合はここで実行
  } catch (error) {
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
 * TODO: Phase 5 (US3) で実装
 */
async function scanBlocks(
  viewKey: Uint8Array,
  spendPubkey: Uint8Array,
  startBlock: number,
  endBlock: number
): Promise<void> {
  if (!wasmModule) throw new Error('Wasm not initialized');
  
  const BATCH_SIZE = 1000;
  const detectedBalances: DetectedStealthBalance[] = [];
  
  for (let block = startBlock; block <= endBlock; block += BATCH_SIZE) {
    const batchEnd = Math.min(block + BATCH_SIZE - 1, endBlock);
    
    // TODO: PAPI経由でエフェメラル公開鍵を取得
    // const entries = await fetchEphemeralKeys(block, batchEnd);
    
    // 進捗を報告
    const progress: ScanProgress = {
      currentBlock: batchEnd,
      targetBlock: endBlock,
      percentage: Math.round((batchEnd - startBlock) / (endBlock - startBlock) * 100),
      detectedCount: detectedBalances.length,
    };
    postResponse({ type: 'scanProgress', progress });
  }
  
  postResponse({ type: 'scanComplete', balances: detectedBalances });
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

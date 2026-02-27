/**
 * Stealth Address TypeScript Type Definitions
 * 
 * ステルスアドレス関連の型定義
 */

/**
 * ステルス鍵ペア
 */
export interface StealthKeyPair {
  /** Spend秘密鍵 (32 bytes) */
  spendKey: Uint8Array;
  
  /** View秘密鍵 (32 bytes) */
  viewKey: Uint8Array;
  
  /** Spend公開鍵 (32 bytes) */
  spendPubkey: Uint8Array;
  
  /** View公開鍵 (32 bytes) */
  viewPubkey: Uint8Array;
  
  /** メタアドレス文字列 (st:anarchy:...) */
  metaAddress: string;
  
  /** 生成タイムスタンプ */
  createdAt: number;
}

/**
 * ステルスアドレス導出結果
 */
export interface StealthAddressResult {
  /** ステルスアドレス (SS58形式) */
  stealthAddress: string;
  
  /** エフェメラル公開鍵 (32 bytes) */
  ephemeralPubkey: Uint8Array;
}

/**
 * 検出されたステルス残高
 */
export interface DetectedStealthBalance {
  /** ステルスアドレス (SS58) */
  stealthAddress: string;
  
  /** 残高 (MORAL単位、12桁精度) */
  balance: bigint;
  
  /** 受信ブロック番号 */
  receivedAt: number;
  
  /** 送金トランザクションハッシュ */
  txHash: Uint8Array;
  
  /** 支出済みフラグ */
  spent: boolean;
  
  /** エフェメラル公開鍵 (秘密鍵導出用に保持) */
  ephemeralPubkey: Uint8Array;
}

/**
 * スキャナー設定
 */
export interface ScannerSettings {
  /** スキャン頻度 */
  frequency: 'realtime' | '1min' | '5min' | 'manual';
  
  /** スキャン開始ブロック */
  startBlock: number;
  
  /** バックグラウンドスキャン有効 */
  backgroundEnabled: boolean;
  
  /** バッテリー節約モード */
  batterySaver: boolean;
}

/**
 * スキャン進捗
 */
export interface ScanProgress {
  /** 現在のブロック */
  currentBlock: number;
  
  /** 目標ブロック */
  targetBlock: number;
  
  /** 進捗率 (0-100) */
  percentage: number;
  
  /** 検出された送金数 */
  detectedCount: number;
}

/**
 * エフェメラル公開鍵エントリ (オンチェーンから取得)
 */
export interface EphemeralKeyEntry {
  /** エフェメラル公開鍵 (32 bytes) */
  ephemeralPubkey: Uint8Array;
  
  /** ステルスアドレス */
  stealthAddress: string;
  
  /** ブロック番号 */
  blockNumber: number;
}

/**
 * バックアップファイル構造
 */
export interface StealthBackup {
  /** フォーマットバージョン */
  version: 1;
  
  /** 暗号化メタデータ */
  crypto: {
    algorithm: 'AES-256-GCM';
    kdf: 'PBKDF2-SHA256';
    iterations: number;
    salt: string;
    nonce: string;
  };
  
  /** 暗号化されたペイロード (base64) */
  ciphertext: string;
}

/**
 * Web Worker メッセージタイプ
 */
export type WorkerMessage = 
  | { type: 'init' }
  | { type: 'generateKeys' }
  | { type: 'deriveAddress'; metaAddress: string }
  | { type: 'scan'; viewKey: Uint8Array; spendPubkey: Uint8Array; startBlock: number; endBlock: number }
  | { type: 'encryptBackup'; spendKey: Uint8Array; viewKey: Uint8Array; password: string }
  | { type: 'decryptBackup'; encrypted: Uint8Array; password: string };

/**
 * Web Worker レスポンスタイプ
 */
export type WorkerResponse =
  | { type: 'initialized' }
  | { type: 'keysGenerated'; keys: StealthKeyPair }
  | { type: 'addressDerived'; result: StealthAddressResult }
  | { type: 'scanProgress'; progress: ScanProgress }
  | { type: 'scanReady'; viewKey: number[]; spendPubkey: number[]; startBlock: number; endBlock: number }
  | { type: 'scanComplete'; balances: DetectedStealthBalance[] }
  | { type: 'backupEncrypted'; data: Uint8Array }
  | { type: 'backupDecrypted'; keys: StealthKeyPair }
  | { type: 'error'; message: string };

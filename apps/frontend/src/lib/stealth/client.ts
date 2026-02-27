/**
 * Stealth Worker Client
 * 
 * Web Workerとの通信を管理するクライアントラッパー
 */

import type { 
  StealthKeyPair, 
  StealthAddressResult, 
  DetectedStealthBalance,
  ScanProgress,
  WorkerMessage, 
  WorkerResponse 
} from './types';

/**
 * スキャンコールバック
 */
export interface ScanCallbacks {
  onProgress?: (progress: ScanProgress) => void;
  onComplete?: (balances: DetectedStealthBalance[]) => void;
  onError?: (error: Error) => void;
}

/**
 * Stealth Worker クライアント
 */
export class StealthWorkerClient {
  private worker: Worker | null = null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private pendingRequests: Map<string, {
    resolve: (value: any) => void;
    reject: (error: Error) => void;
  }> = new Map();
  private scanCallbacks: ScanCallbacks | null = null;
  private requestId = 0;

  /**
   * Workerを初期化
   */
  async init(): Promise<void> {
    if (this.worker) return;

    // Web Worker をサポートしているか確認
    if (typeof Worker === 'undefined') {
      throw new Error('Web Workers are not supported in this environment');
    }

    this.worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
    this.worker.onmessage = this.handleMessage.bind(this);
    this.worker.onerror = this.handleError.bind(this);

    // Wasm初期化を待つ
    await this.sendRequest({ type: 'init' });
  }

  /**
   * Workerを終了
   */
  terminate(): void {
    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
    }
    this.pendingRequests.clear();
  }

  /**
   * 鍵ペアを生成
   */
  async generateKeys(): Promise<StealthKeyPair> {
    const response = await this.sendRequest({ type: 'generateKeys' }) as { keys: StealthKeyPair };
    return response.keys;
  }

  /**
   * ステルスアドレスを導出
   */
  async deriveAddress(metaAddress: string): Promise<StealthAddressResult> {
    const response = await this.sendRequest({ type: 'deriveAddress', metaAddress }) as { result: StealthAddressResult };
    return response.result;
  }

  /**
   * ブロックをスキャン
   */
  async scan(
    viewKey: Uint8Array,
    spendPubkey: Uint8Array,
    startBlock: number,
    endBlock: number,
    callbacks?: ScanCallbacks
  ): Promise<DetectedStealthBalance[]> {
    this.scanCallbacks = callbacks || null;

    return new Promise((resolve, reject) => {
      const id = this.generateRequestId();
      this.pendingRequests.set(id, { 
        resolve: (value) => resolve(value as DetectedStealthBalance[]), 
        reject 
      });

      this.postMessage({ 
        type: 'scan', 
        viewKey, 
        spendPubkey, 
        startBlock, 
        endBlock 
      });
    });
  }

  /**
   * バックアップを暗号化
   */
  async encryptBackup(
    spendKey: Uint8Array,
    viewKey: Uint8Array,
    password: string
  ): Promise<Uint8Array> {
    const response = await this.sendRequest({ 
      type: 'encryptBackup', 
      spendKey, 
      viewKey, 
      password 
    }) as { data: Uint8Array };
    return response.data;
  }

  /**
   * バックアップを復号
   */
  async decryptBackup(encrypted: Uint8Array, password: string): Promise<StealthKeyPair> {
    const response = await this.sendRequest({ 
      type: 'decryptBackup', 
      encrypted, 
      password 
    }) as { keys: StealthKeyPair };
    return response.keys;
  }

  /**
   * リクエストを送信して応答を待つ
   */
  private sendRequest(message: WorkerMessage): Promise<WorkerResponse> {
    return new Promise((resolve, reject) => {
      if (!this.worker) {
        reject(new Error('Worker not initialized'));
        return;
      }

      const id = this.generateRequestId();
      this.pendingRequests.set(id, { resolve, reject });
      this.postMessage(message);
    });
  }

  /**
   * Workerにメッセージを送信
   */
  private postMessage(message: WorkerMessage): void {
    if (!this.worker) {
      throw new Error('Worker not initialized');
    }
    this.worker.postMessage(message);
  }

  /**
   * Workerからのメッセージを処理
   */
  private handleMessage(event: MessageEvent<WorkerResponse>): void {
    const response = event.data;

    switch (response.type) {
      case 'initialized':
      case 'keysGenerated':
      case 'addressDerived':
      case 'backupEncrypted':
      case 'backupDecrypted':
        this.resolvePending(response);
        break;

      case 'scanProgress':
        this.scanCallbacks?.onProgress?.(response.progress);
        break;

      case 'scanComplete':
        this.scanCallbacks?.onComplete?.(response.balances);
        this.resolvePending(response.balances);
        break;

      case 'error':
        this.rejectPending(new Error(response.message));
        this.scanCallbacks?.onError?.(new Error(response.message));
        break;
    }
  }

  /**
   * Workerエラーを処理
   */
  private handleError(event: ErrorEvent): void {
    const error = new Error(event.message);
    this.rejectPending(error);
    this.scanCallbacks?.onError?.(error);
  }

  /**
   * 保留中のリクエストを解決
   */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private resolvePending(value: any): void {
    const pending = this.pendingRequests.values().next().value;
    if (pending) {
      pending.resolve(value);
      this.pendingRequests.delete(this.pendingRequests.keys().next().value!);
    }
  }

  /**
   * 保留中のリクエストを拒否
   */
  private rejectPending(error: Error): void {
    const pending = this.pendingRequests.values().next().value;
    if (pending) {
      pending.reject(error);
      this.pendingRequests.delete(this.pendingRequests.keys().next().value!);
    }
  }

  /**
   * リクエストIDを生成
   */
  private generateRequestId(): string {
    return `req-${++this.requestId}`;
  }
}

/**
 * シングルトンインスタンス
 */
export const stealthWorker = new StealthWorkerClient();

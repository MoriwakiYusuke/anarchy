/**
 * StealthModal Component
 * 
 * ステルスアドレスの生成・送金・受取管理を行うモーダル
 * TransferFormから秘密鍵とsignerを受け取って動作
 */

'use client';

import React, { useState, useCallback, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { PolkadotSigner } from 'polkadot-api';
import { StealthAddressGenerator } from './StealthAddressGenerator';
import { BackupImportDialog } from './BackupImportDialog';
import { StealthSendForm } from './StealthSendForm';
import { StealthSpendForm, type SpendFormValues } from './StealthSpendForm';
import StealthBalanceList from './StealthBalanceList';
import { stealthKeyManager } from '../../lib/stealth/keyManager';
import { sendToStealth } from '../../lib/stealth/api';
import { StealthScanner } from '../../lib/stealth/scanner';
import { createBalanceStore, type BalanceStore } from '../../lib/stealth/balanceStore';
import { deriveKeyFromBalance, createStealthSigner } from '../../lib/stealth/signer';
import type { StealthKeyPair, ScanProgress, DetectedStealthBalance } from '../../lib/stealth/types';
import * as wasm from 'anarchy-wasm-engine';
import styles from './StealthModal.module.css';

export interface StealthModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** PAPI client instance */
  unsafeApi: any;
  /** Current user's signer */
  signer: PolkadotSigner | null;
  /** Current user's account address */
  accountAddress: string | null;
  /** Connection status */
  isConnected: boolean;
  /** Current block number */
  blockNumber?: number;
}

export function StealthModal({
  isOpen,
  onClose,
  unsafeApi,
  signer,
  accountAddress,
  isConnected,
  blockNumber,
}: StealthModalProps) {
  const [mounted, setMounted] = useState(false);
  const [metaAddress, setMetaAddress] = useState<string | null>(null);
  const [isImportDialogOpen, setImportDialogOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<'receive' | 'send' | 'balance'>('receive');
  
  // Scanner state
  const [isScanning, setIsScanning] = useState(false);
  const [scanProgress, setScanProgress] = useState<ScanProgress | null>(null);
  const [balances, setBalances] = useState<DetectedStealthBalance[]>([]);
  const [scanner, setScanner] = useState<StealthScanner | null>(null);
  const [balanceStore, setBalanceStore] = useState<BalanceStore | null>(null);
  
  // Spend state
  const [showSpendForm, setShowSpendForm] = useState(false);
  
  // Export state
  const [isExporting, setIsExporting] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  // 既存の鍵があるか確認
  useEffect(() => {
    if (isOpen) {
      const existingKeys = stealthKeyManager.getKeyPair();
      if (existingKeys) {
        setMetaAddress(existingKeys.metaAddress);
      }
      
      // Initialize balance store (in-memory)
      const store = createBalanceStore();
      setBalanceStore(store);
      setBalances([]);
    }
  }, [isOpen]);

  const handleGenerated = useCallback((keyPair: StealthKeyPair) => {
    setMetaAddress(keyPair.metaAddress);
  }, []);

  const handleImportSuccess = useCallback(() => {
    const keys = stealthKeyManager.getKeyPair();
    if (keys) {
      setMetaAddress(keys.metaAddress);
    }
    setImportDialogOpen(false);
  }, []);

  const handleExportKeys = useCallback(async () => {
    const password = prompt('バックアップのパスワードを入力してください（8文字以上推奨）:');
    if (!password) return;
    
    if (password.length < 8) {
      alert('セキュリティのため、8文字以上のパスワードを使用することをお勧めします。');
    }
    
    setIsExporting(true);
    try {
      const encrypted = await stealthKeyManager.exportBackup(password);
      
      // Uint8ArrayをArrayBufferに変換してBlobを作成
      const buffer = encrypted.buffer.slice(
        encrypted.byteOffset,
        encrypted.byteOffset + encrypted.byteLength
      ) as ArrayBuffer;
      const blob = new Blob([buffer], { type: 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `stealth-backup-${Date.now()}.bin`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      
      alert('バックアップが保存されました。このファイルを安全な場所に保管してください。');
    } catch (error) {
      console.error('Export error:', error);
      alert('エクスポートに失敗しました: ' + (error instanceof Error ? error.message : 'Unknown error'));
    } finally {
      setIsExporting(false);
    }
  }, []);

  const handleClearKeys = useCallback(() => {
    if (confirm('本当に鍵を破棄しますか？バックアップがない場合、資金を失う可能性があります。')) {
      stealthKeyManager.destroy();
      setMetaAddress(null);
      setBalances([]);
    }
  }, []);

  const handleStartScan = useCallback(async () => {
    const keyPair = stealthKeyManager.getKeyPair();
    if (!keyPair || !unsafeApi) {
      console.error('[StealthModal] Cannot start scan: keyPair or unsafeApi missing');
      return;
    }

    // Debug: Log the keys being used for scanning and the metaAddress
    console.log('[StealthModal] Starting scan with keys:', {
      metaAddress: keyPair.metaAddress,
      viewKeyFirst8: Array.from(keyPair.viewKey.slice(0, 8)),
      spendPubkeyFirst8: Array.from(keyPair.spendPubkey.slice(0, 8)),
      viewPubkeyFirst8: Array.from(keyPair.viewPubkey.slice(0, 8)),
    });

    // Key consistency verification: parse metaAddress and compare with keyPair pubkeys
    try {
      const parsed = wasm.parse_meta_address(keyPair.metaAddress);
      const parsedSpendPubkey = new Uint8Array(parsed.spend_pubkey);
      const parsedViewPubkey = new Uint8Array(parsed.view_pubkey);
      
      const spendMatch = parsedSpendPubkey.every((v, i) => v === keyPair.spendPubkey[i]);
      const viewMatch = parsedViewPubkey.every((v, i) => v === keyPair.viewPubkey[i]);
      
      console.log('[StealthModal] Key consistency check:', {
        parsedSpendPubkeyFirst8: Array.from(parsedSpendPubkey.slice(0, 8)),
        keyPairSpendPubkeyFirst8: Array.from(keyPair.spendPubkey.slice(0, 8)),
        spendMatch,
        parsedViewPubkeyFirst8: Array.from(parsedViewPubkey.slice(0, 8)),
        keyPairViewPubkeyFirst8: Array.from(keyPair.viewPubkey.slice(0, 8)),
        viewMatch,
      });
      
      if (!spendMatch || !viewMatch) {
        console.error('[StealthModal] CRITICAL: Key mismatch detected! metaAddress does not match keyPair pubkeys.');
        alert('鍵の整合性エラー: metaAddressとキーペアが一致しません。鍵が破損している可能性があります。');
        setIsScanning(false);
        return;
      }
    } catch (parseError) {
      console.error('[StealthModal] Failed to parse metaAddress for consistency check:', parseError);
    }

    setIsScanning(true);
    setScanProgress({
      currentBlock: 0,
      targetBlock: blockNumber ?? 100,
      percentage: 0,
      detectedCount: 0,
    });

    try {
      const newScanner = new StealthScanner(
        keyPair.viewKey,
        keyPair.spendPubkey,
        unsafeApi
      );
      setScanner(newScanner);

      const startBlock = 0;
      const endBlock = blockNumber ?? 100;

      console.log(`[StealthModal] Scanning blocks ${startBlock} to ${endBlock}`);

      const results = await newScanner.scanBlockRange(
        startBlock,
        endBlock,
        (progress) => setScanProgress(progress),
        { batchSize: 1000, delayBetweenBatches: 100 }
      );

      console.log(`[StealthModal] Scan completed. Total results: ${results.length}`);
      console.log(`[StealthModal] Results:`, results);

      if (balanceStore && results.length > 0) {
        console.log(`[StealthModal] Found ${results.length} stealth addresses, fetching balances...`);
        
        for (const result of results) {
          console.log(`[StealthModal] Processing result:`, {
            stealthAddress: result.stealthAddress,
            isOwned: result.isOwned,
            blockNumber: result.blockNumber,
          });
          
          if (result.isOwned) {
            // Fetch actual balance from chain
            let balance = BigInt(0);
            try {
              console.log(`[StealthModal] Fetching balance for: ${result.stealthAddress}`);
              const accountInfo = await unsafeApi.query.System.Account.getValue(result.stealthAddress);
              console.log(`[StealthModal] Raw accountInfo:`, accountInfo);
              console.log(`[StealthModal] accountInfo.data:`, accountInfo?.data);
              balance = accountInfo?.data?.free ?? BigInt(0);
              console.log(`[StealthModal] Balance for ${result.stealthAddress}: ${balance.toString()}`);
            } catch (balanceError) {
              console.error(`[StealthModal] Failed to fetch balance for ${result.stealthAddress}:`, balanceError);
            }
            
            balanceStore.add({
              stealthAddress: result.stealthAddress,
              balance,
              blockNumber: result.blockNumber,
              ephemeralPubkey: result.ephemeralPubkey,
              txHash: new Uint8Array(32),
            });
          }
        }
        const allBalances = balanceStore.getAllAsStealthBalance();
        console.log(`[StealthModal] Final balances to display:`, allBalances);
        setBalances(allBalances);
      } else {
        console.log(`[StealthModal] No results to process. balanceStore exists: ${!!balanceStore}, results.length: ${results.length}`);
      }
    } catch (error) {
      console.error('Scan error:', error);
    } finally {
      setIsScanning(false);
      setScanner(null);
    }
  }, [unsafeApi, blockNumber, balanceStore]);

  const handleStopScan = useCallback(() => {
    if (scanner) {
      scanner.stop();
      setIsScanning(false);
      setScanner(null);
    }
  }, [scanner]);

  const handleSend = useCallback(async (stealthAddress: string, amount: string, ephemeralPubkey: Uint8Array) => {
    if (!unsafeApi || !signer) {
      throw new Error('Not connected');
    }
    
    // stealthAddress and ephemeralPubkey are already derived together in StealthSendForm
    // using the same ephemeral keypair, so they match correctly
    await sendToStealth(unsafeApi, signer, {
      stealthAddress,
      ephemeralPubkey,
      amount: BigInt(amount),
    });
  }, [unsafeApi, signer]);

  const handleSpend = useCallback(async (values: SpendFormValues) => {
    if (!signer || !unsafeApi) {
      throw new Error('Not connected');
    }
    
    console.log('Spending from stealth:', values);
    // TODO: Implement actual spending with derived stealth signer
    setShowSpendForm(false);
  }, [signer, unsafeApi]);

  const isSendDisabled = !isConnected || !signer;

  if (!isOpen || !mounted) return null;

  return createPortal(
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <header className={styles.header}>
          <h2>ステルスアドレス</h2>
          <button type="button" className={styles.closeBtn} onClick={onClose}>✕</button>
        </header>

        <nav className={styles.tabNav}>
          <button
            type="button"
            className={`${styles.tabButton} ${activeTab === 'receive' ? styles.tabButtonActive : ''}`}
            onClick={() => setActiveTab('receive')}
          >
            受け取り設定
          </button>
          <button
            type="button"
            className={`${styles.tabButton} ${activeTab === 'send' ? styles.tabButtonActive : ''}`}
            onClick={() => setActiveTab('send')}
          >
            ステルス送金
          </button>
          <button
            type="button"
            className={`${styles.tabButton} ${activeTab === 'balance' ? styles.tabButtonActive : ''}`}
            onClick={() => setActiveTab('balance')}
          >
            残高確認
          </button>
        </nav>

        <main className={styles.content}>
          {activeTab === 'receive' && (
            <>
              {!metaAddress ? (
                <div className={styles.setupSection}>
                  <StealthAddressGenerator onGenerated={handleGenerated} />
                  <div className={styles.divider}><span>または</span></div>
                  <div className={styles.importSection}>
                    <button
                      type="button"
                      className={styles.importButton}
                      onClick={() => setImportDialogOpen(true)}
                    >
                      バックアップからインポート
                    </button>
                  </div>
                </div>
              ) : (
                <div className={styles.activeSection}>
                  <div className={styles.metaAddressDisplay}>
                    <label>あなたのメタアドレス</label>
                    <code className={styles.addressCode}>{metaAddress}</code>
                    <p className={styles.helpText}>
                      このアドレスを送金者に共有してください。
                    </p>
                  </div>
                  <div className={styles.actionsSection}>
                    <h3>管理</h3>
                    <button
                      type="button"
                      className={styles.secondaryButton}
                      onClick={handleExportKeys}
                      disabled={isExporting}
                    >
                      {isExporting ? 'エクスポート中...' : '鍵をエクスポート'}
                    </button>
                    <button
                      type="button"
                      className={styles.dangerButton}
                      onClick={handleClearKeys}
                    >
                      鍵を破棄
                    </button>
                  </div>
                </div>
              )}
            </>
          )}

          {activeTab === 'send' && (
            <div className={styles.sendSection}>
              <p className={styles.description}>
                受取人のステルスメタアドレスを入力して、プライバシーを保護した送金を行います。
              </p>
              {isSendDisabled && (
                <div className={styles.warningBox}>
                  <p>送金には接続とサインイン名が必要です。</p>
                </div>
              )}
              <StealthSendForm
                onSend={handleSend}
                disabled={isSendDisabled}
              />
            </div>
          )}

          {activeTab === 'balance' && (
            <div className={styles.balanceSection}>
              {!metaAddress ? (
                <div className={styles.warningBox}>
                  <p>残高を確認するには、まず受け取り設定で鍵を生成してください。</p>
                </div>
              ) : (
                <>
                  <StealthBalanceList
                    balances={balances}
                    isScanning={isScanning}
                    scanProgress={scanProgress ?? undefined}
                    onStartScan={handleStartScan}
                    onStopScan={handleStopScan}
                  />
                  {balances.length > 0 && !showSpendForm && (
                    <button
                      type="button"
                      onClick={() => setShowSpendForm(true)}
                      className={styles.spendButton}
                    >
                      残高を送金
                    </button>
                  )}
                  {showSpendForm && (
                    <StealthSpendForm
                      balances={balances}
                      onSpend={handleSpend}
                      onCancel={() => setShowSpendForm(false)}
                      isProcessing={false}
                      defaultRecipientAddress={accountAddress ?? ''}
                    />
                  )}
                </>
              )}
            </div>
          )}
        </main>

        <BackupImportDialog
          isOpen={isImportDialogOpen}
          onClose={() => setImportDialogOpen(false)}
          onImportSuccess={handleImportSuccess}
        />
      </div>
    </div>,
    document.body
  );
}

export default StealthModal;

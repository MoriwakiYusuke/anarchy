/**
 * Stealth Address Page
 * 
 * ステルスアドレスの生成・管理ページ
 */

'use client';

import React, { useState, useCallback, useEffect } from 'react';
import { useLocale } from '../../i18n/context';
import { StealthAddressGenerator } from '../../components/stealth/StealthAddressGenerator';
import { BackupImportDialog } from '../../components/stealth/BackupImportDialog';
import { StealthSendForm } from '../../components/stealth/StealthSendForm';
import { StealthSpendForm, type SpendFormValues } from '../../components/stealth/StealthSpendForm';
import { StealthBalanceList } from '../../components/stealth';
import { stealthKeyManager } from '../../lib/stealth/keyManager';
import { sendToStealth } from '../../lib/stealth/api';
import { StealthScanner } from '../../lib/stealth/scanner';
import { createBalanceStore, type BalanceStore } from '../../lib/stealth/balanceStore';
import { deriveKeyFromBalance, createStealthSigner } from '../../lib/stealth/signer';
import type { StealthKeyPair, ScanProgress, DetectedStealthBalance } from '../../lib/stealth/types';
import { useChain } from '../../hooks/useChain';

export default function StealthPage() {
  const { t } = useLocale();
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
  const [isSpending, setIsSpending] = useState(false);
  const [showSpendForm, setShowSpendForm] = useState(false);
  
  // Network state (T081)
  const [wasDisconnected, setWasDisconnected] = useState(false);
  const [lastScannedBlock, setLastScannedBlock] = useState<number>(0);
  
  // Hooks for blockchain interaction
  const { unsafeApi, connectionState } = useChain();
  // Note: Signer integration requires useApi hook from main page context
  // For now, send form will show "coming soon" message
  const signer = null;
  const isAuthenticated = false;

  // 既存の鍵があるか確認
  useEffect(() => {
    const existing = stealthKeyManager.getMetaAddress();
    if (existing) {
      setMetaAddress(existing);
    }
    
    // Initialize balance store
    const store = createBalanceStore();
    setBalanceStore(store);
    setBalances(store.getAllAsStealthBalance());
    
    // Load last scanned block from localStorage
    const savedBlock = localStorage.getItem('stealth_last_scanned_block');
    if (savedBlock) {
      setLastScannedBlock(parseInt(savedBlock, 10));
    }
  }, []);

  // T081: Network disconnection handling
  useEffect(() => {
    // Check for non-connected states (error, syncing, initializing)
    const isDisconnected = connectionState.status !== 'connected';
    
    if (isDisconnected && connectionState.status === 'error') {
      setWasDisconnected(true);
      // Stop any ongoing scan
      if (scanner && isScanning) {
        scanner.stop();
        setIsScanning(false);
      }
    } else if (connectionState.status === 'connected' && wasDisconnected) {
      // Reconnected after disconnection
      setWasDisconnected(false);
      console.log('[StealthPage] Reconnected, ready for catch-up scan');
    }
  }, [connectionState.status, scanner, isScanning, wasDisconnected]);

  // T085: Catch-up scan on app foreground return
  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible' && metaAddress && !isScanning) {
        // App returned to foreground, check if we need to catch up
        const currentBlock = connectionState.blockNumber ?? 0;
        if (currentBlock > lastScannedBlock + 10) {
          console.log(`[StealthPage] Catch-up: ${lastScannedBlock} → ${currentBlock}`);
          // Auto-start catch-up scan for balance tab
          if (activeTab === 'balance') {
            // Don't auto-start, just show notification
            console.log('[StealthPage] New blocks available, user can start scan');
          }
        }
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => document.removeEventListener('visibilitychange', handleVisibilityChange);
  }, [metaAddress, isScanning, connectionState.blockNumber, lastScannedBlock, activeTab]);

  /**
   * Start scanning for stealth payments
   * T082: Optimized with 1000 blocks/batch
   * T084: Progress indicator for full scan
   */
  const handleStartScan = useCallback(async () => {
    const keyPair = stealthKeyManager.getKeyPair();
    if (!keyPair || !unsafeApi) {
      console.warn('Cannot start scan: missing keys or API');
      return;
    }

    // T081: Check network connection
    if (connectionState.status !== 'connected') {
      console.warn('Cannot start scan: not connected to network');
      return;
    }

    setIsScanning(true);
    setScanProgress({
      currentBlock: 0,
      targetBlock: 0,
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

      // T084: Use lastScannedBlock for incremental scan
      const startBlock = lastScannedBlock > 0 ? lastScannedBlock + 1 : 0;
      const endBlock = connectionState.blockNumber ?? 100;

      // T082: Use 1000 blocks/batch for performance
      const results = await newScanner.scanBlockRange(
        startBlock,
        endBlock,
        (progress) => setScanProgress(progress),
        { batchSize: 1000, delayBetweenBatches: 100 }
      );

      // Add detected balances to store
      if (balanceStore) {
        for (const result of results) {
          if (result.isOwned) {
            // Note: In real implementation, we'd need to query the actual balance
            // For now, we add with placeholder balance
            balanceStore.add({
              stealthAddress: result.stealthAddress,
              balance: BigInt(0), // Would need to query actual balance
              blockNumber: result.blockNumber,
              ephemeralPubkey: result.ephemeralPubkey,
              txHash: new Uint8Array(32), // Would need actual tx hash
            });
          }
        }
        setBalances(balanceStore.getAllAsStealthBalance());
      }

      // Save last scanned block for incremental scan
      setLastScannedBlock(endBlock);
      localStorage.setItem('stealth_last_scanned_block', endBlock.toString());
    } catch (error) {
      console.error('Scan error:', error);
    } finally {
      setIsScanning(false);
      setScanner(null);
    }
  }, [unsafeApi, connectionState.blockNumber, connectionState.status, balanceStore, lastScannedBlock]);

  /**
   * Stop scanning
   */
  const handleStopScan = useCallback(() => {
    if (scanner) {
      scanner.stop();
    }
    setIsScanning(false);
    setScanner(null);
  }, [scanner]);

  /**
   * Handle balance selection - show spend form
   */
  const handleBalanceSelect = useCallback((balance: DetectedStealthBalance) => {
    console.log('Selected balance:', balance.stealthAddress);
    setShowSpendForm(true);
  }, []);

  /**
   * Handle spend from stealth balance
   */
  const handleSpend = useCallback(async (values: SpendFormValues) => {
    if (!unsafeApi) {
      throw new Error('ブロックチェーンに接続していません');
    }

    const keyPair = stealthKeyManager.getKeyPair();
    if (!keyPair) {
      throw new Error('鍵が設定されていません');
    }

    setIsSpending(true);
    try {
      // 各選択残高に対してステルス秘密鍵を導出してトランザクションを作成
      for (const balance of values.selectedBalances) {
        const privateKey = await deriveKeyFromBalance(
          balance,
          keyPair.spendKey,
          keyPair.viewKey
        );
        const stealthSigner = await createStealthSigner(privateKey);
        
        try {
          // Polkadot互換のSignerを取得
          const polkadotSigner = await stealthSigner.getPolkadotSigner();
          
          console.log('[handleSpend] Spending from:', balance.stealthAddress);
          console.log('[handleSpend] To:', values.recipientAddress);
          console.log('[handleSpend] Amount:', values.amount.toString());
          
          // Balances.transfer_allow_death トランザクションを作成・送信
          // transfer_allow_deathは全額転送でアカウント削除を許可
          const tx = unsafeApi.tx.Balances.transfer_allow_death({
            dest: { type: 'Id', value: values.recipientAddress },
            value: values.amount,
          });
          
          const result = await tx.signAndSubmit(polkadotSigner);
          console.log('[handleSpend] Transaction result:', result);
          
          // 残高を更新（部分送金対応）
          if (balanceStore) {
            const currentBalance = balance.balance;
            const newBalance = currentBalance - values.amount;
            balanceStore.updateBalance(balance.stealthAddress, newBalance);
          }
        } finally {
          stealthSigner.destroy();
        }
      }

      // 残高リストを更新
      if (balanceStore) {
        setBalances(balanceStore.getAllAsStealthBalance());
      }
      
      setShowSpendForm(false);
      alert('送金が完了しました');
    } catch (error) {
      console.error('Spend error:', error);
      throw error;
    } finally {
      setIsSpending(false);
    }
  }, [unsafeApi, balanceStore]);

  /**
   * 鍵生成完了ハンドラ
   */
  const handleGenerated = useCallback((keyPair: StealthKeyPair) => {
    setMetaAddress(keyPair.metaAddress);
  }, []);

  /**
   * インポート成功ハンドラ
   */
  const handleImportSuccess = useCallback((address: string) => {
    setMetaAddress(address);
  }, []);

  /**
   * 鍵を破棄してリセット
   */
  const handleReset = useCallback(() => {
    if (window.confirm('現在の鍵を破棄しますか？バックアップがない場合、資金を失う可能性があります。')) {
      stealthKeyManager.destroy();
      setMetaAddress(null);
    }
  }, []);

  /**
   * ステルス送金ハンドラ
   */
  const handleSend = useCallback(async (
    _metaAddress: string,
    amount: string,
    ephemeralPubkey: Uint8Array
  ) => {
    if (!unsafeApi || !signer) {
      throw new Error('ブロックチェーンに接続していません');
    }

    // Import wasm to derive stealth address
    const wasm = await import('anarchy-wasm-engine');
    const derivation = wasm.derive_stealth_address(_metaAddress);

    await sendToStealth(unsafeApi, signer, {
      stealthAddress: derivation.stealth_address,
      ephemeralPubkey,
      amount: BigInt(amount),
    });
  }, [unsafeApi, signer]);

  const isSendDisabled = connectionState.status !== 'connected' || !isAuthenticated;

  return (
    <div className="stealth-page">
      <header className="page-header">
        <h1>ステルスアドレス</h1>
        <p>
          プライバシーを保護した送受金のためのワンタイムアドレスを管理します。
        </p>
      </header>

      {/* Tab navigation */}
      <nav className="tab-nav">
        <button
          type="button"
          className={`tab-button ${activeTab === 'receive' ? 'active' : ''}`}
          onClick={() => setActiveTab('receive')}
        >
          {t('stealth.receive')}
        </button>
        <button
          type="button"
          className={`tab-button ${activeTab === 'send' ? 'active' : ''}`}
          onClick={() => setActiveTab('send')}
        >
          {t('stealth.send')}
        </button>
        <button
          type="button"
          className={`tab-button ${activeTab === 'balance' ? 'active' : ''}`}
          onClick={() => setActiveTab('balance')}
        >
          {t('stealth.balance')}
        </button>
      </nav>

      <main className="page-content">
        {activeTab === 'receive' && (
          <>
            {!metaAddress ? (
              <div className="setup-section">
                <StealthAddressGenerator onGenerated={handleGenerated} />

                <div className="divider">
                  <span>または</span>
                </div>

                <div className="import-section">
                  <button 
                    onClick={() => setImportDialogOpen(true)}
                    type="button"
                    className="import-button"
                  >
                    バックアップからインポート
                  </button>
                </div>
              </div>
            ) : (
              <div className="active-section">
                <StealthAddressGenerator existingMetaAddress={metaAddress} />

                <div className="actions-section">
                  <h3>操作</h3>
                  <button
                    onClick={() => setImportDialogOpen(true)}
                    type="button"
                  >
                    別のバックアップをインポート
                  </button>
                  <button
                    onClick={handleReset}
                    type="button"
                    className="danger-button"
                  >
                    鍵を破棄
              </button>
            </div>

            <div className="info-section">
              <h3>使い方</h3>
              <ol>
                <li>上記のメタアドレスを送金者に共有してください</li>
                <li>送金者がメタアドレス宛に送金すると、ワンタイムアドレスが使用されます</li>
                <li>「受信確認」ページでスキャンすると、あなた宛の送金が検出されます</li>
              </ol>
            </div>
              </div>
            )}
          </>
        )}

        {activeTab === 'send' && (
          <div className="send-section">
            <h2>{t('stealth.sendForm.pageTitle')}</h2>
            <p className="send-description">
              {t('stealth.sendForm.description')}
            </p>

            {isSendDisabled && (
              <div className="warning-box">
                {connectionState.status !== 'connected' && (
                  <p>{t('stealth.sendForm.connecting')}</p>
                )}
                {connectionState.status === 'connected' && !isAuthenticated && (
                  <p>{t('stealth.sendForm.notReady')}</p>
                )}
              </div>
            )}

            <StealthSendForm
              onSend={handleSend}
              disabled={isSendDisabled}
            />
          </div>
        )}

        {activeTab === 'balance' && (
          <div className="balance-section">
            <h2>{t('stealth.balance.title')}</h2>
            <p className="balance-description">
              {t('stealth.balance.description')}
            </p>

            {!metaAddress ? (
              <div className="warning-box">
                <p>{t('stealth.balance.noMetaAddress')}</p>
              </div>
            ) : showSpendForm ? (
              <div className="spend-form-section">
                <button
                  type="button"
                  onClick={() => setShowSpendForm(false)}
                  className="back-button"
                >
                  {t('stealth.balance.backToList')}
                </button>
                <StealthSpendForm
                  balances={balances}
                  onSpend={handleSpend}
                  onCancel={() => setShowSpendForm(false)}
                  isProcessing={isSpending}
                />
              </div>
            ) : (
              <>
                <StealthBalanceList
                  balances={balances}
                  isScanning={isScanning}
                  scanProgress={scanProgress ?? undefined}
                  onSelect={handleBalanceSelect}
                  onStartScan={handleStartScan}
                  onStopScan={handleStopScan}
                />
                {balances.length > 0 && (
                  <button
                    type="button"
                    onClick={() => setShowSpendForm(true)}
                    className="spend-button"
                  >
                    {t('stealth.spend.sendBalance')}
                  </button>
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

      <style jsx>{`
        .stealth-page {
          max-width: 600px;
          margin: 0 auto;
          padding: 24px;
          position: relative;
          z-index: 1;
          min-height: 100vh;
        }

        .page-header {
          margin-bottom: 32px;
        }

        .page-header h1 {
          margin: 0 0 8px;
          color: var(--text-primary, #fff);
        }

        .page-header p {
          color: var(--text-secondary, #888);
          margin: 0;
        }

        .setup-section {
          background: var(--bg-secondary, #141414);
          border-radius: 8px;
          padding: 24px;
          border: 1px solid var(--border, #2a2a2a);
        }

        .divider {
          text-align: center;
          margin: 24px 0;
          color: var(--text-secondary, #888);
        }

        .import-section {
          text-align: center;
        }

        .import-button {
          background: var(--accent, #ff4444);
          color: #fff;
          border: none;
          padding: 12px 24px;
          border-radius: 4px;
          cursor: pointer;
        }

        .active-section {
          display: flex;
          flex-direction: column;
          gap: 24px;
        }

        .actions-section {
          background: var(--bg-secondary, #141414);
          border-radius: 8px;
          padding: 16px;
          border: 1px solid var(--border, #2a2a2a);
        }

        .actions-section h3 {
          margin: 0 0 12px;
          font-size: 14px;
          color: var(--text-primary, #fff);
        }

        .actions-section button {
          margin-right: 8px;
          margin-bottom: 8px;
        }

        .danger-button {
          background: #dc3545;
          color: #fff;
          border: none;
          padding: 8px 16px;
          border-radius: 4px;
          cursor: pointer;
        }

        .info-section {
          background: var(--bg-secondary, #141414);
          border-radius: 8px;
          padding: 16px;
          border: 1px solid var(--border, #2a2a2a);
        }

        .info-section h3 {
          margin: 0 0 12px;
          font-size: 14px;
          color: var(--text-primary, #fff);
        }

        .info-section ol {
          margin: 0;
          padding-left: 20px;
          color: var(--text-secondary, #888);
        }

        .info-section li {
          margin-bottom: 8px;
        }

        .tab-nav {
          display: flex;
          gap: 8px;
          margin-bottom: 24px;
          border-bottom: 1px solid var(--border, #2a2a2a);
          padding-bottom: 8px;
        }

        .tab-button {
          background: none;
          border: none;
          padding: 8px 16px;
          cursor: pointer;
          font-size: 14px;
          color: var(--text-secondary, #888);
          border-radius: 4px 4px 0 0;
          transition: background 0.2s, color 0.2s;
        }

        .tab-button:hover {
          background: var(--bg-secondary, #141414);
        }

        .tab-button.active {
          color: var(--text-primary, #fff);
          font-weight: 600;
          border-bottom: 2px solid var(--accent, #ff4444);
        }

        .send-section {
          background: var(--bg-secondary, #141414);
          border-radius: 8px;
          padding: 24px;
          border: 1px solid var(--border, #2a2a2a);
        }

        .send-section h2 {
          margin: 0 0 8px;
          font-size: 18px;
          color: var(--text-primary, #fff);
        }

        .send-description {
          color: var(--text-secondary, #888);
          margin: 0 0 16px;
          font-size: 14px;
        }

        .warning-box {
          background: rgba(255, 193, 7, 0.1);
          border: 1px solid #ffc107;
          border-radius: 4px;
          padding: 12px;
          margin-bottom: 16px;
        }

        .warning-box p {
          margin: 0;
          color: #ffc107;
          font-size: 14px;
        }

        .balance-section {
          background: var(--bg-secondary, #141414);
          border-radius: 8px;
          padding: 24px;
          border: 1px solid var(--border, #2a2a2a);
        }

        .balance-section h2 {
          margin: 0 0 8px;
          font-size: 18px;
          color: var(--text-primary, #fff);
        }

        .balance-description {
          color: var(--text-secondary, #888);
          margin: 0 0 16px;
          font-size: 14px;
        }

        .spend-button {
          background: var(--accent, #ff4444);
          color: #fff;
          border: none;
          padding: 8px 16px;
          border-radius: 4px;
          cursor: pointer;
          margin-top: 16px;
        }

        .page-content {
          color: var(--text-primary, #fff);
        }
      `}</style>
    </div>
  );
}

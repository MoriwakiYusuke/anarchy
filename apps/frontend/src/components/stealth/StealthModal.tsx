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
import { useLocale } from '../../i18n/context';
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
import { debugLog, debugError } from '../../lib/debugLog';
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
  const { t } = useLocale();
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
  const [isSpending, setIsSpending] = useState(false);
  const [spendSuccessMessage, setSpendSuccessMessage] = useState<string | null>(null);
  const [spendErrorMessage, setSpendErrorMessage] = useState<string | null>(null);
  
  // Export state
  const [isExporting, setIsExporting] = useState(false);
  const [showExportForm, setShowExportForm] = useState(false);
  const [exportPassword, setExportPassword] = useState('');
  const [exportConfirmPassword, setExportConfirmPassword] = useState('');
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportSuccess, setExportSuccess] = useState(false);

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

  // バックアップエクスポート (StealthAddressGenerator と同じ パスワード + 確認 + 8文字以上 のフロー)
  const handleExportKeys = useCallback(async () => {
    // 8文字未満は中断する (推奨ではなく必須)
    if (exportPassword.length < 8) {
      setExportError(t('stealth.backup.passwordTooShort'));
      return;
    }
    if (exportPassword !== exportConfirmPassword) {
      setExportError(t('stealth.backup.passwordMismatch'));
      return;
    }

    setExportError(null);
    setExportSuccess(false);
    setIsExporting(true);
    try {
      const encrypted = await stealthKeyManager.exportBackup(exportPassword);

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

      setExportSuccess(true);
      setShowExportForm(false);
      setExportPassword('');
      setExportConfirmPassword('');
    } catch (error) {
      debugError('[StealthModal] Export error:', error);
      setExportError(error instanceof Error ? error.message : t('stealth.backup.createError'));
    } finally {
      setIsExporting(false);
    }
  }, [exportPassword, exportConfirmPassword, t]);

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
      debugError('[StealthModal] Cannot start scan: keyPair or unsafeApi missing');
      return;
    }

    // 鍵の整合性検証: metaAddress をパースし keyPair の公開鍵と一致するか確認する。
    // セキュリティ: 鍵素材 (viewKey 等) や metaAddress はログに出さない (匿名性原則)。
    try {
      const parsed = wasm.parse_meta_address(keyPair.metaAddress);
      const parsedSpendPubkey = new Uint8Array(parsed.spend_pubkey);
      const parsedViewPubkey = new Uint8Array(parsed.view_pubkey);

      const spendMatch = parsedSpendPubkey.every((v, i) => v === keyPair.spendPubkey[i]);
      const viewMatch = parsedViewPubkey.every((v, i) => v === keyPair.viewPubkey[i]);

      debugLog('[StealthModal] Key consistency check:', { spendMatch, viewMatch });

      if (!spendMatch || !viewMatch) {
        debugError('[StealthModal] CRITICAL: Key mismatch detected! metaAddress does not match keyPair pubkeys.');
        alert('鍵の整合性エラー: metaAddressとキーペアが一致しません。鍵が破損している可能性があります。');
        setIsScanning(false);
        return;
      }
    } catch (parseError) {
      debugError('[StealthModal] Failed to parse metaAddress for consistency check:', parseError);
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

      debugLog(`[StealthModal] Scanning blocks ${startBlock} to ${endBlock}`);

      const results = await newScanner.scanBlockRange(
        startBlock,
        endBlock,
        (progress) => setScanProgress(progress),
        { batchSize: 1000, delayBetweenBatches: 100 }
      );

      // セキュリティ: 検出された stealth address / 残高はユーザーの相関情報なので
      // ログには件数のみ出す (イベントレベル)。
      debugLog(`[StealthModal] Scan completed. Total results: ${results.length}`);

      if (balanceStore && results.length > 0) {
        for (const result of results) {
          if (result.isOwned) {
            // Fetch actual balance from chain
            let balance = BigInt(0);
            try {
              const accountInfo = await unsafeApi.query.System.Account.getValue(result.stealthAddress);
              balance = accountInfo?.data?.free ?? BigInt(0);
            } catch (balanceError) {
              debugError('[StealthModal] Failed to fetch balance for a detected stealth address:', balanceError);
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
        setBalances(balanceStore.getAllAsStealthBalance());
      }
    } catch (error) {
      debugError('[StealthModal] Scan error:', error);
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
    if (!unsafeApi) {
      setSpendErrorMessage('ブロックチェーンに接続していません');
      throw new Error('ブロックチェーンに接続していません');
    }
    
    const keyPair = stealthKeyManager.getKeyPair();
    if (!keyPair) {
      setSpendErrorMessage('鍵が設定されていません');
      throw new Error('鍵が設定されていません');
    }

    // セキュリティ: 送金先 / 金額 / stealth address の相関情報はログに出さない
    debugLog('[StealthModal] Spend requested:', { utxoCount: values.selectedBalances.length });

    setIsSpending(true);
    setSpendErrorMessage(null);
    setSpendSuccessMessage(null);

    try {
      // multi-UTXO 送金: 要求合計額 (values.amount) を選択残高に分割する。
      // 各 UTXO からは min(残り必要額, その残高) のみ送り、残りが 0 になったら終了。
      let remaining = values.amount;
      let totalSpent = BigInt(0);

      for (const balance of values.selectedBalances) {
        if (remaining <= BigInt(0)) break;

        // この UTXO から送る額 (残高を超えない / 必要額を超えない)
        const sendValue = remaining < balance.balance ? remaining : balance.balance;
        if (sendValue <= BigInt(0)) continue;

        const privateKey = await deriveKeyFromBalance(
          balance,
          keyPair.spendKey,
          keyPair.viewKey
        );
        const stealthSigner = await createStealthSigner(privateKey);

        try {
          // Polkadot互換のSignerを取得
          const polkadotSigner = await stealthSigner.getPolkadotSigner();

          // Balances.transfer_allow_death トランザクションを作成・送信
          const tx = unsafeApi.tx.Balances.transfer_allow_death({
            dest: { type: 'Id', value: values.recipientAddress },
            value: sendValue,
          });

          await tx.signAndSubmit(polkadotSigner);

          remaining -= sendValue;
          totalSpent += sendValue;

          // 残高を更新（部分送金対応）
          if (balanceStore) {
            balanceStore.updateBalance(balance.stealthAddress, balance.balance - sendValue);
          }
        } finally {
          stealthSigner.destroy();
        }
      }

      // 残高リストを更新
      if (balanceStore) {
        setBalances(balanceStore.getAllAsStealthBalance());
      }

      // 成功メッセージを設定 (12桁精度でMORAL表示) — 実際に送れた合計を表示
      const moralAmount = Number(totalSpent) / 1_000_000_000_000;
      setSpendSuccessMessage(t('stealth.spend.success', { amount: moralAmount.toFixed(4) }));

      // フォームリセット後に少し待ってからフォームを閉じる
      setTimeout(() => {
        setShowSpendForm(false);
        setSpendSuccessMessage(null);
      }, 3000);

    } catch (error) {
      debugError('[StealthModal] Spend error:', error);
      const errorMsg = error instanceof Error ? error.message : 'Unknown error';
      setSpendErrorMessage(t('stealth.spend.failed', { error: errorMsg }));
      throw error;
    } finally {
      setIsSpending(false);
    }
  }, [unsafeApi, balanceStore, t]);

  const isSendDisabled = !isConnected || !signer;

  if (!isOpen || !mounted) return null;

  return createPortal(
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <header className={styles.header}>
          <h2>{t('stealth.title')}</h2>
          <button type="button" className={styles.closeBtn} onClick={onClose}>✕</button>
        </header>

        <nav className={styles.tabNav}>
          <button
            type="button"
            className={`${styles.tabButton} ${activeTab === 'receive' ? styles.tabButtonActive : ''}`}
            onClick={() => setActiveTab('receive')}
          >
            {t('stealth.receive')}
          </button>
          <button
            type="button"
            className={`${styles.tabButton} ${activeTab === 'send' ? styles.tabButtonActive : ''}`}
            onClick={() => setActiveTab('send')}
          >
            {t('stealth.send')}
          </button>
          <button
            type="button"
            className={`${styles.tabButton} ${activeTab === 'balance' ? styles.tabButtonActive : ''}`}
            onClick={() => setActiveTab('balance')}
          >
            {t('stealth.balance')}
          </button>
        </nav>

        <main className={styles.content}>
          {activeTab === 'receive' && (
            <>
              {!metaAddress ? (
                <div className={styles.setupSection}>
                  <StealthAddressGenerator onGenerated={handleGenerated} />
                  <div className={styles.divider}><span>{t('stealth.manage.or')}</span></div>
                  <div className={styles.importSection}>
                    <button
                      type="button"
                      className={styles.importButton}
                      onClick={() => setImportDialogOpen(true)}
                    >
                      {t('stealth.import.button')}
                    </button>
                  </div>
                </div>
              ) : (
                <div className={styles.activeSection}>
                  <div className={styles.metaAddressDisplay}>
                    <label>{t('stealth.metaAddress.yourAddress')}</label>
                    <code className={styles.addressCode}>{metaAddress}</code>
                    <p className={styles.helpText}>
                      {t('stealth.metaAddress.shareHint')}
                    </p>
                  </div>
                  <div className={styles.actionsSection}>
                    <h3>{t('stealth.manage.title')}</h3>
                    {!showExportForm ? (
                      <button
                        type="button"
                        className={styles.secondaryButton}
                        onClick={() => {
                          setShowExportForm(true);
                          setExportError(null);
                          setExportSuccess(false);
                        }}
                      >
                        {t('stealth.manage.export')}
                      </button>
                    ) : (
                      <div className={styles.exportForm}>
                        {/* バックアップ暗号化パスワード (8文字以上 + 確認入力) */}
                        <input
                          type="password"
                          className={styles.exportInput}
                          placeholder={t('stealth.backup.passwordPlaceholder')}
                          value={exportPassword}
                          onChange={(e) => setExportPassword(e.target.value)}
                          minLength={8}
                          disabled={isExporting}
                        />
                        <input
                          type="password"
                          className={styles.exportInput}
                          placeholder={t('stealth.backup.confirmPlaceholder')}
                          value={exportConfirmPassword}
                          onChange={(e) => setExportConfirmPassword(e.target.value)}
                          disabled={isExporting}
                        />
                        <button
                          type="button"
                          className={styles.secondaryButton}
                          onClick={handleExportKeys}
                          disabled={isExporting || exportPassword.length < 8}
                        >
                          {isExporting ? t('stealth.manage.exporting') : t('stealth.backup.download')}
                        </button>
                        <button
                          type="button"
                          className={styles.secondaryButton}
                          onClick={() => {
                            setShowExportForm(false);
                            setExportPassword('');
                            setExportConfirmPassword('');
                            setExportError(null);
                          }}
                          disabled={isExporting}
                        >
                          {t('stealth.import.cancel')}
                        </button>
                      </div>
                    )}
                    {exportError && (
                      <p className={styles.exportError}>{exportError}</p>
                    )}
                    {exportSuccess && (
                      <p className={styles.exportSuccess}>{t('stealth.backup.complete')}</p>
                    )}
                    <button
                      type="button"
                      className={styles.dangerButton}
                      onClick={handleClearKeys}
                    >
                      {t('stealth.manage.clear')}
                    </button>
                  </div>
                </div>
              )}
            </>
          )}

          {activeTab === 'send' && (
            <div className={styles.sendSection}>
              <p className={styles.description}>
                {t('stealth.sendForm.description')}
              </p>
              {isSendDisabled && (
                <div className={styles.warningBox}>
                  <p>{t('stealth.sendForm.requireSignIn')}</p>
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
                  <p>{t('stealth.scan.needKeys')}</p>
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
                      {t('stealth.spend.sendBalance')}
                    </button>
                  )}
                  {showSpendForm && (
                    <StealthSpendForm
                      balances={balances}
                      onSpend={handleSpend}
                      onCancel={() => setShowSpendForm(false)}
                      isProcessing={isSpending}
                      defaultRecipientAddress={accountAddress ?? ''}
                      successMessage={spendSuccessMessage}
                      errorMessage={spendErrorMessage}
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

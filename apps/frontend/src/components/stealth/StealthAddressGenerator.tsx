/**
 * StealthAddressGenerator Component
 * 
 * ステルスメタアドレスの生成とバックアップUI
 */

'use client';

import React, { useState, useCallback } from 'react';
import { useLocale } from '../../i18n/context';
import { stealthKeyManager } from '../../lib/stealth/keyManager';
import type { StealthKeyPair } from '../../lib/stealth/types';
import styles from './StealthAddressGenerator.module.css';

export interface StealthAddressGeneratorProps {
  /** 生成完了時のコールバック */
  onGenerated?: (keyPair: StealthKeyPair) => void;
  /** 生成済みの場合の表示用メタアドレス */
  existingMetaAddress?: string;
}

export function StealthAddressGenerator({
  onGenerated,
  existingMetaAddress,
}: StealthAddressGeneratorProps) {
  const { t } = useLocale();
  const [isGenerating, setIsGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [keyPair, setKeyPair] = useState<StealthKeyPair | null>(null);
  const [backupPassword, setBackupPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [isBackedUp, setIsBackedUp] = useState(false);

  /**
   * 鍵を生成
   */
  const handleGenerate = useCallback(async () => {
    setIsGenerating(true);
    setError(null);

    try {
      const generated = await stealthKeyManager.generateKeys();
      setKeyPair(generated);
      onGenerated?.(generated);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('stealth.generate.error'));
    } finally {
      setIsGenerating(false);
    }
  }, [onGenerated]);

  /**
   * バックアップをダウンロード
   */
  const handleDownloadBackup = useCallback(async () => {
    if (!keyPair) return;
    if (backupPassword.length < 8) {
      setError(t('stealth.backup.passwordTooShort'));
      return;
    }
    if (backupPassword !== confirmPassword) {
      setError(t('stealth.backup.passwordMismatch'));
      return;
    }

    setError(null);

    try {
      const encrypted = await stealthKeyManager.exportBackup(backupPassword);
      
      // Blob作成とダウンロード
      // Note: ArrayBufferへキャストしてBlobPartの互換性問題を回避
      const buffer = encrypted.buffer.slice(
        encrypted.byteOffset,
        encrypted.byteOffset + encrypted.byteLength
      ) as ArrayBuffer;
      const blob = new Blob([buffer], { type: 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `anarchy-stealth-backup-${Date.now()}.bin`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);

      setIsBackedUp(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('stealth.backup.createError'));
    }
  }, [keyPair, backupPassword, confirmPassword, t]);

  /**
   * メタアドレスをコピー
   */
  const handleCopyAddress = useCallback(async () => {
    const address = keyPair?.metaAddress || existingMetaAddress;
    if (address) {
      await navigator.clipboard.writeText(address);
    }
  }, [keyPair, existingMetaAddress]);

  // 既存のメタアドレスがある場合は表示のみ
  if (existingMetaAddress && !keyPair) {
    return (
      <div className={styles.stealthGenerator}>
        <h3>{t('stealth.metaAddress.title')}</h3>
        <div className={styles.metaAddressDisplay}>
          <code>{existingMetaAddress}</code>
          <button onClick={handleCopyAddress} type="button">
            {t('stealth.metaAddress.copy')}
          </button>
        </div>
        <p className={styles.infoText}>
          {t('stealth.metaAddress.shareHint')}
        </p>
      </div>
    );
  }

  return (
    <div className={styles.stealthGenerator}>
      <h3>{t('stealth.generate.title')}</h3>

      {!keyPair ? (
        <div className={styles.generateSection}>
          <p>
            {t('stealth.generate.description')}
          </p>
          <button
            onClick={handleGenerate}
            disabled={isGenerating}
            type="button"
          >
            {isGenerating ? t('stealth.generate.generating') : t('stealth.generate.button')}
          </button>
        </div>
      ) : (
        <div className={styles.keyGeneratedSection}>
          <div className={styles.metaAddressDisplay}>
            <label>{t('stealth.metaAddress.label')}</label>
            <div className={styles.addressRow}>
              <code>{keyPair.metaAddress}</code>
              <button onClick={handleCopyAddress} type="button">
                {t('stealth.metaAddress.copy')}
              </button>
            </div>
          </div>

          {!isBackedUp && (
            <div className={styles.backupSection}>
              <h4>{t('stealth.backup.title')}</h4>
              <p className={styles.warningText}>
                {t('stealth.backup.warning')}
              </p>

              <div className={styles.passwordInputs}>
                <input
                  type="password"
                  placeholder={t('stealth.backup.passwordPlaceholder')}
                  value={backupPassword}
                  onChange={(e) => setBackupPassword(e.target.value)}
                  minLength={8}
                />
                <input
                  type="password"
                  placeholder={t('stealth.backup.confirmPlaceholder')}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                />
              </div>

              <button
                onClick={handleDownloadBackup}
                disabled={backupPassword.length < 8}
                type="button"
              >
                {t('stealth.backup.download')}
              </button>
            </div>
          )}

          {isBackedUp && (
            <div className={styles.backupComplete}>
              <p className={styles.successText}>
                {t('stealth.backup.complete')}
              </p>
              <p>
                {t('stealth.metaAddress.shareHint')}
              </p>
            </div>
          )}
        </div>
      )}

      {error && (
        <div className={styles.errorMessage}>
          {error}
        </div>
      )}
    </div>
  );
}

export default StealthAddressGenerator;

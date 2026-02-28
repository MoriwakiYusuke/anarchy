/**
 * BackupImportDialog Component
 * 
 * バックアップファイルからステルス鍵をインポートするダイアログ
 */

'use client';

import React, { useState, useCallback, useRef } from 'react';
import { useLocale } from '../../i18n/context';
import { stealthKeyManager } from '../../lib/stealth/keyManager';
import styles from './BackupImportDialog.module.css';

export interface BackupImportDialogProps {
  /** ダイアログの開閉状態 */
  isOpen: boolean;
  /** 閉じる時のコールバック */
  onClose: () => void;
  /** インポート成功時のコールバック */
  onImportSuccess?: (metaAddress: string) => void;
}

export function BackupImportDialog({
  isOpen,
  onClose,
  onImportSuccess,
}: BackupImportDialogProps) {
  const { t } = useLocale();
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  /**
   * ファイル選択ハンドラ
   */
  const handleFileSelect = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      setSelectedFile(file);
      setError(null);
    }
  }, []);

  /**
   * ファイル選択ボタンクリック
   */
  const handleSelectClick = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  /**
   * インポート実行
   */
  const handleImport = useCallback(async () => {
    if (!selectedFile) {
      setError(t('stealth.import.noFile'));
      return;
    }
    if (!password) {
      setError(t('stealth.import.noPassword'));
      return;
    }

    setIsImporting(true);
    setError(null);

    try {
      // ファイルを読み込む
      const arrayBuffer = await selectedFile.arrayBuffer();
      const encrypted = new Uint8Array(arrayBuffer);

      // インポート実行
      await stealthKeyManager.importFromBackup(encrypted, password);

      const metaAddress = stealthKeyManager.getMetaAddress();
      if (metaAddress) {
        onImportSuccess?.(metaAddress);
      }
      onClose();
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error('Import error:', err);
      setError(t('stealth.import.error'));
    } finally {
      setIsImporting(false);
    }
  }, [selectedFile, password, onImportSuccess, onClose]);

  /**
   * ダイアログを閉じる
   */
  const handleClose = useCallback(() => {
    setPassword('');
    setError(null);
    setSelectedFile(null);
    onClose();
  }, [onClose]);

  if (!isOpen) {
    return null;
  }

  return (
    <div className={styles.overlay} onClick={handleClose}>
      <div 
        className={styles.dialog} 
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-dialog-title"
      >
        <h2 id="import-dialog-title">{t('stealth.import.title')}</h2>

        <div className={styles.form}>
          <div className={styles.fileSection}>
            <input
              ref={fileInputRef}
              type="file"
              accept=".bin,.backup"
              onChange={handleFileSelect}
              style={{ display: 'none' }}
            />
            <button 
              className={styles.fileButton}
              onClick={handleSelectClick} 
              type="button"
            >
              {t('stealth.import.selectFile')}
            </button>
            {selectedFile && (
              <span className={styles.fileName}>{selectedFile.name}</span>
            )}
          </div>

          <div className={styles.passwordSection}>
            <label htmlFor="import-password">{t('stealth.import.password')}</label>
            <input
              id="import-password"
              type="password"
              className={styles.passwordInput}
              placeholder={t('stealth.import.passwordPlaceholder')}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  handleImport();
                }
              }}
            />
          </div>

          {error && (
            <div className={styles.errorMessage}>
              {error}
            </div>
          )}

          <div className={styles.actions}>
            <button 
              className={styles.cancelButton}
              onClick={handleClose} 
              type="button"
              disabled={isImporting}
            >
              {t('stealth.import.cancel')}
            </button>
            <button
              className={styles.importButton}
              onClick={handleImport}
              disabled={isImporting || !selectedFile || !password}
              type="button"
            >
              {isImporting ? t('stealth.import.importing') : t('stealth.import.importButton')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default BackupImportDialog;

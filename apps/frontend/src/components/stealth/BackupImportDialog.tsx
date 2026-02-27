/**
 * BackupImportDialog Component
 * 
 * バックアップファイルからステルス鍵をインポートするダイアログ
 */

'use client';

import React, { useState, useCallback, useRef } from 'react';
import { stealthKeyManager } from '../../lib/stealth/keyManager';

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
      setError('バックアップファイルを選択してください');
      return;
    }
    if (!password) {
      setError('パスワードを入力してください');
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
      setError(
        'インポートに失敗しました。パスワードが正しいか確認してください。'
      );
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
    <div className="backup-import-overlay" onClick={handleClose}>
      <div 
        className="backup-import-dialog" 
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-dialog-title"
      >
        <h2 id="import-dialog-title">バックアップからインポート</h2>

        <div className="import-form">
          <div className="file-section">
            <input
              ref={fileInputRef}
              type="file"
              accept=".bin,.backup"
              onChange={handleFileSelect}
              style={{ display: 'none' }}
            />
            <button onClick={handleSelectClick} type="button">
              ファイルを選択
            </button>
            {selectedFile && (
              <span className="file-name">{selectedFile.name}</span>
            )}
          </div>

          <div className="password-section">
            <label htmlFor="import-password">パスワード</label>
            <input
              id="import-password"
              type="password"
              placeholder="バックアップ作成時のパスワード"
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
            <div className="error-message">
              {error}
            </div>
          )}

          <div className="dialog-actions">
            <button 
              onClick={handleClose} 
              type="button"
              disabled={isImporting}
            >
              キャンセル
            </button>
            <button
              onClick={handleImport}
              disabled={isImporting || !selectedFile || !password}
              type="button"
            >
              {isImporting ? 'インポート中...' : 'インポート'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default BackupImportDialog;

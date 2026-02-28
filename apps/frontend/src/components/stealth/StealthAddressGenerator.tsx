/**
 * StealthAddressGenerator Component
 * 
 * ステルスメタアドレスの生成とバックアップUI
 */

'use client';

import React, { useState, useCallback } from 'react';
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
      setError(err instanceof Error ? err.message : '鍵の生成に失敗しました');
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
      setError('パスワードは8文字以上必要です');
      return;
    }
    if (backupPassword !== confirmPassword) {
      setError('パスワードが一致しません');
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
      setError(err instanceof Error ? err.message : 'バックアップの作成に失敗しました');
    }
  }, [keyPair, backupPassword, confirmPassword]);

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
        <h3>ステルスメタアドレス</h3>
        <div className={styles.metaAddressDisplay}>
          <code>{existingMetaAddress}</code>
          <button onClick={handleCopyAddress} type="button">
            コピー
          </button>
        </div>
        <p className={styles.infoText}>
          このアドレスを送金者に共有してください。
        </p>
      </div>
    );
  }

  return (
    <div className={styles.stealthGenerator}>
      <h3>ステルスアドレス生成</h3>

      {!keyPair ? (
        <div className={styles.generateSection}>
          <p>
            ステルスアドレスを使用すると、送金を受け取る際に
            ワンタイムアドレスが使用され、プライバシーが保護されます。
          </p>
          <button
            onClick={handleGenerate}
            disabled={isGenerating}
            type="button"
          >
            {isGenerating ? '生成中...' : 'ステルス鍵を生成'}
          </button>
        </div>
      ) : (
        <div className={styles.keyGeneratedSection}>
          <div className={styles.metaAddressDisplay}>
            <label>メタアドレス</label>
            <div className={styles.addressRow}>
              <code>{keyPair.metaAddress}</code>
              <button onClick={handleCopyAddress} type="button">
                コピー
              </button>
            </div>
          </div>

          {!isBackedUp && (
            <div className={styles.backupSection}>
              <h4>⚠️ バックアップを作成してください</h4>
              <p className={styles.warningText}>
                このバックアップなしでは資金を回復できません。
                安全な場所に保管してください。
              </p>

              <div className={styles.passwordInputs}>
                <input
                  type="password"
                  placeholder="バックアップパスワード（8文字以上）"
                  value={backupPassword}
                  onChange={(e) => setBackupPassword(e.target.value)}
                  minLength={8}
                />
                <input
                  type="password"
                  placeholder="パスワード確認"
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                />
              </div>

              <button
                onClick={handleDownloadBackup}
                disabled={backupPassword.length < 8}
                type="button"
              >
                バックアップをダウンロード
              </button>
            </div>
          )}

          {isBackedUp && (
            <div className={styles.backupComplete}>
              <p className={styles.successText}>
                ✓ バックアップ完了
              </p>
              <p>
                このアドレスを送金者に共有してください。
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

/**
 * StealthAddressGenerator Component
 * 
 * ステルスメタアドレスの生成とバックアップUI
 */

'use client';

import React, { useState, useCallback } from 'react';
import { stealthKeyManager } from '../../lib/stealth/keyManager';
import type { StealthKeyPair } from '../../lib/stealth/types';

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
      <div className="stealth-generator">
        <h3>ステルスメタアドレス</h3>
        <div className="meta-address-display">
          <code>{existingMetaAddress}</code>
          <button onClick={handleCopyAddress} type="button">
            コピー
          </button>
        </div>
        <p className="info-text">
          このアドレスを送金者に共有してください。
        </p>
      </div>
    );
  }

  return (
    <div className="stealth-generator">
      <h3>ステルスアドレス生成</h3>

      {!keyPair ? (
        <div className="generate-section">
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
        <div className="key-generated-section">
          <div className="meta-address-display">
            <label>メタアドレス</label>
            <div className="address-row">
              <code>{keyPair.metaAddress}</code>
              <button onClick={handleCopyAddress} type="button">
                コピー
              </button>
            </div>
          </div>

          {!isBackedUp && (
            <div className="backup-section">
              <h4>⚠️ バックアップを作成してください</h4>
              <p className="warning-text">
                このバックアップなしでは資金を回復できません。
                安全な場所に保管してください。
              </p>

              <div className="password-inputs">
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
            <div className="backup-complete">
              <p className="success-text">
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
        <div className="error-message">
          {error}
        </div>
      )}
    </div>
  );
}

export default StealthAddressGenerator;

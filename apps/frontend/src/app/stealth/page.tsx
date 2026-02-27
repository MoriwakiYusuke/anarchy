/**
 * Stealth Address Page
 * 
 * ステルスアドレスの生成・管理ページ
 */

'use client';

import React, { useState, useCallback, useEffect } from 'react';
import { StealthAddressGenerator } from '../../components/stealth/StealthAddressGenerator';
import { BackupImportDialog } from '../../components/stealth/BackupImportDialog';
import { StealthSendForm } from '../../components/stealth/StealthSendForm';
import { stealthKeyManager } from '../../lib/stealth/keyManager';
import { sendToStealth } from '../../lib/stealth/api';
import type { StealthKeyPair } from '../../lib/stealth/types';
import { useSmoldot } from '../../hooks/useSmoldot';

export default function StealthPage() {
  const [metaAddress, setMetaAddress] = useState<string | null>(null);
  const [isImportDialogOpen, setImportDialogOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<'receive' | 'send'>('receive');
  
  // Hooks for blockchain interaction
  const { unsafeApi, connectionState } = useSmoldot();
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
  }, []);

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
          受け取り設定
        </button>
        <button
          type="button"
          className={`tab-button ${activeTab === 'send' ? 'active' : ''}`}
          onClick={() => setActiveTab('send')}
        >
          ステルス送金
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
            <h2>ステルスアドレスへ送金</h2>
            <p className="send-description">
              受取人のメタアドレスを入力して、プライベートな送金を行います。
            </p>

            {isSendDisabled && (
              <div className="warning-box">
                {connectionState.status !== 'connected' && (
                  <p>ブロックチェーンに接続中です...</p>
                )}
                {connectionState.status === 'connected' && !isAuthenticated && (
                  <p>送金機能は現在準備中です。</p>
                )}
              </div>
            )}

            <StealthSendForm
              onSend={handleSend}
              disabled={isSendDisabled}
            />
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
        }

        .page-header {
          margin-bottom: 32px;
        }

        .page-header h1 {
          margin: 0 0 8px;
        }

        .page-header p {
          color: #666;
          margin: 0;
        }

        .setup-section {
          background: #f5f5f5;
          border-radius: 8px;
          padding: 24px;
        }

        .divider {
          text-align: center;
          margin: 24px 0;
          color: #999;
        }

        .import-section {
          text-align: center;
        }

        .import-button {
          background: #333;
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
          background: #f5f5f5;
          border-radius: 8px;
          padding: 16px;
        }

        .actions-section h3 {
          margin: 0 0 12px;
          font-size: 14px;
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
          background: #e3f2fd;
          border-radius: 8px;
          padding: 16px;
        }

        .info-section h3 {
          margin: 0 0 12px;
          font-size: 14px;
        }

        .info-section ol {
          margin: 0;
          padding-left: 20px;
        }

        .info-section li {
          margin-bottom: 8px;
        }

        .tab-nav {
          display: flex;
          gap: 8px;
          margin-bottom: 24px;
          border-bottom: 1px solid #e0e0e0;
          padding-bottom: 8px;
        }

        .tab-button {
          background: none;
          border: none;
          padding: 8px 16px;
          cursor: pointer;
          font-size: 14px;
          color: #666;
          border-radius: 4px 4px 0 0;
          transition: background 0.2s, color 0.2s;
        }

        .tab-button:hover {
          background: #f0f0f0;
        }

        .tab-button.active {
          color: #333;
          font-weight: 600;
          border-bottom: 2px solid #333;
        }

        .send-section {
          background: #f5f5f5;
          border-radius: 8px;
          padding: 24px;
        }

        .send-section h2 {
          margin: 0 0 8px;
          font-size: 18px;
        }

        .send-description {
          color: #666;
          margin: 0 0 16px;
          font-size: 14px;
        }

        .warning-box {
          background: #fff3cd;
          border: 1px solid #ffc107;
          border-radius: 4px;
          padding: 12px;
          margin-bottom: 16px;
        }

        .warning-box p {
          margin: 0;
          color: #856404;
          font-size: 14px;
        }
      `}</style>
    </div>
  );
}

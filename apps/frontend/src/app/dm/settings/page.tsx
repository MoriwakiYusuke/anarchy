/**
 * /dm/settings ページ (T067)。
 *
 * 役割: DM 鍵の publish/revoke (DmKeyManager) と block リスト管理 (BlockListManager) を
 * 1 枚に集約。バックアップのエクスポート/インポートも扱う (FR-022)。
 * stealth 鍵が未ロードの場合、ここで直接生成も可能にして /stealth への遷移を不要にする。
 */

'use client';

import { useCallback, useEffect, useState } from 'react';
import Link from 'next/link';
import { useSmoldot } from '@/hooks/useSmoldot';
import { useApi } from '@/hooks/useApi';
import { DmKeyManager } from '@/components/dm/DmKeyManager';
import { BlockListManager } from '@/components/dm/BlockListManager';
import { exportDmBackup, importDmBackup } from '@/lib/dm/keyManager';
import { stealthKeyManager } from '@/lib/stealth/keyManager';
import type { PolkadotSigner } from 'polkadot-api/signer';
import styles from './page.module.css';

export default function DmSettingsPage(): JSX.Element {
  const { unsafeApi } = useSmoldot();
  const { createSigner } = useApi();
  const [signer, setSigner] = useState<PolkadotSigner | null>(null);
  const [password, setPassword] = useState('');
  const [status, setStatus] = useState<
    { kind: 'idle' } | { kind: 'ok'; message: string } | { kind: 'error'; message: string }
  >({ kind: 'idle' });
  const [hasStealthKey, setHasStealthKey] = useState(false);
  const [isGenerating, setIsGenerating] = useState(false);

  useEffect(() => {
    void (async () => {
      const s: PolkadotSigner | null = await createSigner('//Alice');
      if (s) setSigner(s);
    })();
  }, [createSigner]);

  useEffect(() => {
    const check = (): void => setHasStealthKey(stealthKeyManager.getMetaAddress() !== null);
    check();
    const id = window.setInterval(check, 500);
    return () => window.clearInterval(id);
  }, []);

  const handleGenerateStealth = useCallback(async () => {
    setIsGenerating(true);
    setStatus({ kind: 'idle' });
    try {
      await stealthKeyManager.generateKeys();
      setHasStealthKey(true);
      setStatus({ kind: 'ok', message: 'stealth 鍵を生成しました。続けて DM 鍵を公開できます。' });
    } catch (err) {
      setStatus({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setIsGenerating(false);
    }
  }, []);

  const handleExport = async (): Promise<void> => {
    try {
      if (!password) {
        setStatus({ kind: 'error', message: 'パスワードを入力してください。' });
        return;
      }
      const bytes = await exportDmBackup(password);
      const blob = new Blob([bytes.buffer as ArrayBuffer], {
        type: 'application/octet-stream',
      });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `anarchy-dm-backup-${Date.now()}.bin`;
      a.click();
      URL.revokeObjectURL(url);
      setStatus({ kind: 'ok', message: 'バックアップをダウンロードしました。' });
    } catch (err) {
      setStatus({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    }
  };

  const handleImport = async (file: File): Promise<void> => {
    try {
      if (!password) {
        setStatus({ kind: 'error', message: 'パスワードを入力してください。' });
        return;
      }
      const buf = new Uint8Array(await file.arrayBuffer());
      await importDmBackup(buf, password);
      setStatus({ kind: 'ok', message: 'バックアップをインポートしました。' });
    } catch (err) {
      setStatus({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    }
  };

  return (
    <main className={styles.main}>
      <header className={styles.header}>
        <Link href="/dm" className={styles.backLink}>
          ← インボックスへ戻る
        </Link>
        <h1 className={styles.title}>DM 設定</h1>
      </header>

      {!hasStealthKey && (
        <section className={styles.section} aria-label="stealth 鍵生成">
          <h3 className={styles.sectionTitle}>まず stealth 鍵を生成</h3>
          <p className={styles.description}>
            DM の暗号化にはステルスアドレス用の鍵ペアが必要です。このブラウザセッションの
            メモリ内だけで鍵を管理します (ページを閉じると破棄)。
          </p>
          <button
            type="button"
            className={styles.primaryBtn}
            onClick={() => void handleGenerateStealth()}
            disabled={isGenerating}
          >
            {isGenerating ? '生成中…' : 'stealth 鍵を生成する'}
          </button>
        </section>
      )}

      <section className={styles.section} aria-label="DM 鍵管理">
        {unsafeApi && signer ? (
          <DmKeyManager api={unsafeApi} signer={signer} initialPublished={false} />
        ) : (
          <p className={styles.muted}>接続中…</p>
        )}
      </section>

      <section className={styles.section} aria-label="バックアップ">
        <h3 className={styles.sectionTitle}>バックアップ</h3>
        <p className={styles.description}>
          DM 鍵と会話履歴をパスワードで暗号化してエクスポート / インポートします (FR-022)。
        </p>
        <label className={styles.field}>
          <span className={styles.label}>パスワード</span>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className={styles.input}
            placeholder="8 文字以上を推奨"
          />
        </label>
        <div className={styles.actions}>
          <button
            type="button"
            className={styles.primaryBtn}
            onClick={() => void handleExport()}
            disabled={!password}
          >
            エクスポート
          </button>
          <label className={styles.fileLabel}>
            <span>インポート</span>
            <input
              type="file"
              accept=".bin,application/octet-stream"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) void handleImport(f);
              }}
              className={styles.fileInput}
            />
          </label>
        </div>
        {status.kind === 'ok' && (
          <p role="status" aria-live="polite" className={styles.success}>
            {status.message}
          </p>
        )}
        {status.kind === 'error' && (
          <p role="alert" className={styles.error}>
            エラー: {status.message}
          </p>
        )}
      </section>

      <section className={styles.section} aria-label="ブロックリスト">
        <BlockListManager />
      </section>
    </main>
  );
}

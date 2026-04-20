/**
 * /dm/settings ページ (T067)。
 *
 * 役割: DM 鍵の publish/revoke (DmKeyManager) と block リスト管理 (BlockListManager) を
 * 1 枚に集約。バックアップのエクスポート/インポートも扱う (FR-022)。
 */

'use client';

import { useEffect, useState } from 'react';
import Link from 'next/link';
import { useSmoldot } from '@/hooks/useSmoldot';
import { useApi } from '@/hooks/useApi';
import { DmKeyManager } from '@/components/dm/DmKeyManager';
import { BlockListManager } from '@/components/dm/BlockListManager';
import { exportDmBackup, importDmBackup } from '@/lib/dm/keyManager';
import type { PolkadotSigner } from 'polkadot-api/signer';

export default function DmSettingsPage(): JSX.Element {
  const { unsafeApi } = useSmoldot();
  const { createSigner } = useApi();
  const [signer, setSigner] = useState<PolkadotSigner | null>(null);
  const [password, setPassword] = useState('');
  const [status, setStatus] = useState<
    { kind: 'idle' } | { kind: 'ok'; message: string } | { kind: 'error'; message: string }
  >({ kind: 'idle' });

  useEffect(() => {
    void (async () => {
      const s: PolkadotSigner | null = await createSigner('//Alice');
      if (s) setSigner(s);
    })();
  }, [createSigner]);

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
    <main>
      <header>
        <Link href="/dm">← インボックスへ戻る</Link>
        <h1>DM 設定</h1>
      </header>

      {unsafeApi && signer ? (
        <DmKeyManager api={unsafeApi} signer={signer} initialPublished={false} />
      ) : (
        <p>接続中…</p>
      )}

      <section aria-label="バックアップ">
        <h3>バックアップ</h3>
        <label>
          パスワード:
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
        <div>
          <button type="button" onClick={() => void handleExport()} disabled={!password}>
            エクスポート
          </button>
          <label>
            インポート:
            <input
              type="file"
              accept=".bin,application/octet-stream"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) void handleImport(f);
              }}
            />
          </label>
        </div>
        {status.kind === 'ok' && (
          <p role="status" aria-live="polite">
            {status.message}
          </p>
        )}
        {status.kind === 'error' && (
          <p role="alert" style={{ color: '#c00' }}>
            エラー: {status.message}
          </p>
        )}
      </section>

      <BlockListManager />
    </main>
  );
}

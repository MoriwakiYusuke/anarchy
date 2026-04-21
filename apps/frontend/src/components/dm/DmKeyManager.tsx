'use client';

/**
 * DmKeyManager (T048) — DM 受信鍵を pallet_messaging に publish/revoke する UI。
 *
 * Contract: contracts/frontend-ui.md §2.4.
 *
 * Backup の export/import は T063 (US2) のため本 MVP には含めない。
 * `<BackupImportDialog />` を再利用する placeholder は後続実装で追加する。
 */

import { useCallback, useEffect, useState } from 'react';
import {
  getDmMetaAddressFromStealth,
  publishDmKey,
  revokeDmKey,
} from '@/lib/dm/keyManager';
import { useDmStore } from '@/lib/dm/store';
import type { PolkadotSigner } from 'polkadot-api/signer';
import type { DmMetaAddress } from '@/lib/dm/types';
import styles from './DmKeyManager.module.css';

export interface DmKeyManagerProps {
  /** PAPI unsafeApi。`tx.Messaging.publish_dm_key` / `revoke_dm_key` を持つ。 */
  api: unknown;
  /** メインアカウントの signer。 */
  signer: PolkadotSigner;
  /** 現在の公開状態 (親側で `DmReceptionKeys` を query した結果)。
   *  null = 未公開, true = 公開中。 */
  initialPublished?: boolean;
}

type ActionState =
  | { kind: 'idle' }
  | { kind: 'busy'; action: 'publish' | 'revoke' }
  | { kind: 'error'; message: string }
  | { kind: 'ok'; message: string };

export function DmKeyManager({ api, signer, initialPublished = false }: DmKeyManagerProps): JSX.Element {
  const [published, setPublished] = useState<boolean>(initialPublished);
  const [state, setState] = useState<ActionState>({ kind: 'idle' });
  const [meta, setMeta] = useState<DmMetaAddress | null>(null);
  const receiptOptOut = useDmStore((s: { receiptOptOut: boolean }) => s.receiptOptOut);
  const setReceiptOptOut = useDmStore(
    (s: { setReceiptOptOut: (v: boolean) => void }) => s.setReceiptOptOut,
  );

  // stealth 鍵のロード状態を 500ms 毎に確認。/dm/settings で生成した直後にも追従する。
  useEffect(() => {
    const check = (): void => {
      try {
        setMeta(getDmMetaAddressFromStealth());
      } catch {
        setMeta(null);
      }
    };
    check();
    const id = window.setInterval(check, 500);
    return () => window.clearInterval(id);
  }, []);

  const onPublish = useCallback(async () => {
    if (!meta) {
      setState({ kind: 'error', message: 'まず stealth 鍵を生成してください。' });
      return;
    }
    setState({ kind: 'busy', action: 'publish' });
    try {
      await publishDmKey(api, signer);
      setPublished(true);
      setState({ kind: 'ok', message: '公開しました。' });
    } catch (err) {
      setState({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }, [api, signer, meta]);

  const onRevoke = useCallback(async () => {
    setState({ kind: 'busy', action: 'revoke' });
    try {
      await revokeDmKey(api, signer);
      setPublished(false);
      setState({ kind: 'ok', message: '取り消しました。' });
    } catch (err) {
      setState({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }, [api, signer]);

  const isBusy = state.kind === 'busy';

  return (
    <div role="region" aria-label="DM key manager" className={styles.region}>
      <h3 className={styles.title}>DM 受信鍵</h3>
      <p className={styles.statusLine}>
        状態:{' '}
        <span className={published ? styles.published : styles.unpublished}>
          {published ? '公開中' : '未公開'}
        </span>
      </p>

      {!meta && (
        <p role="alert" className={styles.warning}>
          stealth 鍵が読み込まれていません。上のセクションで生成するか、バックアップをインポートしてください。
        </p>
      )}

      <div className={styles.actions}>
        <button
          type="button"
          onClick={() => void onPublish()}
          disabled={isBusy || !meta || published}
          className={styles.primaryBtn}
        >
          {state.kind === 'busy' && state.action === 'publish' ? '公開中…' : '公開する'}
        </button>
        <button
          type="button"
          onClick={() => void onRevoke()}
          disabled={isBusy || !published}
          className={styles.secondaryBtn}
        >
          {state.kind === 'busy' && state.action === 'revoke' ? '取り消し中…' : '取り消す'}
        </button>
      </div>

      {state.kind === 'error' && (
        <p role="alert" className={styles.error}>
          エラー: {state.message}
        </p>
      )}
      {state.kind === 'ok' && (
        <p role="status" aria-live="polite" className={styles.ok}>
          {state.message}
        </p>
      )}

      <fieldset aria-label="受信確認設定" className={styles.fieldset}>
        <legend className={styles.legend}>受信確認 (read receipt)</legend>
        <label className={styles.checkboxLabel}>
          <input
            type="checkbox"
            checked={receiptOptOut}
            onChange={(e) => setReceiptOptOut(e.target.checked)}
          />
          既読通知を送信しない (FR-016b)
        </label>
      </fieldset>
    </div>
  );
}

export default DmKeyManager;

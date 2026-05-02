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
import { useLocale } from '@/i18n';
import type { PolkadotSigner } from 'polkadot-api/signer';
import type { DmMetaAddress } from '@/lib/dm/types';
import styles from './DmKeyManager.module.css';

export interface DmKeyManagerProps {
  /** PAPI unsafeApi。`tx.Messaging.publish_dm_key` / `revoke_dm_key` を持つ。 */
  api: unknown;
  /** メインアカウントの signer。 */
  signer: PolkadotSigner;
  /** メインアカウントの SS58。`DmReceptionKeys` を query して publish 状態を判定するために使う。 */
  accountId?: string;
}

type ActionState =
  | { kind: 'idle' }
  | { kind: 'busy'; action: 'publish' | 'revoke' }
  | { kind: 'error'; message: string }
  | { kind: 'ok'; message: string };

function metaEquals(a: DmMetaAddress | null, b: DmMetaAddress | null): boolean {
  if (!a || !b) return a === b;
  if (a.scanPub.length !== b.scanPub.length || a.spendPub.length !== b.spendPub.length) return false;
  for (let i = 0; i < a.scanPub.length; i += 1) if (a.scanPub[i] !== b.scanPub[i]) return false;
  for (let i = 0; i < a.spendPub.length; i += 1) if (a.spendPub[i] !== b.spendPub[i]) return false;
  return true;
}

function normalizeChainMeta(raw: unknown): DmMetaAddress | null {
  if (!raw || typeof raw !== 'object') return null;
  const r = raw as { scan_pub?: unknown; scanPub?: unknown; spend_pub?: unknown; spendPub?: unknown };
  const toBytes = (v: unknown): Uint8Array | null => {
    if (v instanceof Uint8Array) return v;
    if (v && typeof v === 'object' && 'asBytes' in v && typeof (v as { asBytes: unknown }).asBytes === 'function') {
      return (v as { asBytes: () => Uint8Array }).asBytes();
    }
    return null;
  };
  const scanPub = toBytes(r.scan_pub ?? r.scanPub);
  const spendPub = toBytes(r.spend_pub ?? r.spendPub);
  if (!scanPub || !spendPub) return null;
  return { scanPub, spendPub };
}

export function DmKeyManager({ api, signer, accountId }: DmKeyManagerProps): JSX.Element {
  const { t } = useLocale();
  const [chainMeta, setChainMeta] = useState<DmMetaAddress | null>(null);
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

  // チェーンの publish 状態を取得。初回マウント + accountId 変更 + 操作後リフレッシュ。
  // **重要**: chain に残った old meta と現在のセッション meta が食い違うケース
  // (例: 鍵を再生成したが publish していない) を必ず検出するために、bool ではなく
  // meta 自体を保持して比較する。
  const refreshChainMeta = useCallback(async () => {
    if (!api || !accountId) return;
    const typed = api as {
      query?: { Messaging?: { DmReceptionKeys?: { getValue: (a: string) => Promise<unknown> } } };
    };
    const query = typed.query?.Messaging?.DmReceptionKeys;
    if (!query) return;
    try {
      const res = await query.getValue(accountId);
      setChainMeta(normalizeChainMeta(res));
    } catch {
      // 取得失敗時は前回値を維持 (誤って revoke を出さないための fail-safe)
    }
  }, [api, accountId]);

  useEffect(() => {
    let cancelled = false;
    void refreshChainMeta().then(() => { if (cancelled) setChainMeta(null); });
    return () => { cancelled = true; };
  }, [refreshChainMeta]);

  const publishedAndCurrent = chainMeta !== null && metaEquals(chainMeta, meta);
  const publishedButStale = chainMeta !== null && !publishedAndCurrent && meta !== null;

  const onPublish = useCallback(async () => {
    if (!meta) {
      setState({ kind: 'error', message: t('dm.keyManager.stealthRequired') });
      return;
    }
    setState({ kind: 'busy', action: 'publish' });
    try {
      await publishDmKey(api, signer);
      await refreshChainMeta();
      setState({ kind: 'ok', message: t('dm.keyManager.publishSuccess') });
    } catch (err) {
      setState({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }, [api, signer, meta, t, refreshChainMeta]);

  const onRevoke = useCallback(async () => {
    setState({ kind: 'busy', action: 'revoke' });
    try {
      await revokeDmKey(api, signer);
      await refreshChainMeta();
      setState({ kind: 'ok', message: t('dm.keyManager.revokeSuccess') });
    } catch (err) {
      setState({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }, [api, signer, t, refreshChainMeta]);

  const isBusy = state.kind === 'busy';

  return (
    <div role="region" aria-label="DM key manager" className={styles.region}>
      <h3 className={styles.title}>{t('dm.keyManager.title')}</h3>
      <p className={styles.statusLine}>
        {t('dm.keyManager.statusLabel')}:{' '}
        <span className={publishedAndCurrent ? styles.published : styles.unpublished}>
          {publishedAndCurrent
            ? t('dm.keyManager.statusPublished')
            : t('dm.keyManager.statusUnpublished')}
        </span>
      </p>

      {publishedButStale && (
        <p role="alert" className={styles.warning}>
          {t('dm.keyManager.staleKeyWarning')}
        </p>
      )}

      {!meta && (
        <p role="alert" className={styles.warning}>
          {t('dm.keyManager.stealthMissing')}
        </p>
      )}

      <div className={styles.actions}>
        <button
          type="button"
          onClick={() => void onPublish()}
          disabled={isBusy || !meta || publishedAndCurrent}
          className={styles.primaryBtn}
        >
          {state.kind === 'busy' && state.action === 'publish'
            ? t('dm.keyManager.publishing')
            : publishedButStale
              ? t('dm.keyManager.republish')
              : t('dm.keyManager.publish')}
        </button>
        <button
          type="button"
          onClick={() => void onRevoke()}
          disabled={isBusy || chainMeta === null}
          className={styles.secondaryBtn}
        >
          {state.kind === 'busy' && state.action === 'revoke'
            ? t('dm.keyManager.revoking')
            : t('dm.keyManager.revoke')}
        </button>
      </div>

      {state.kind === 'error' && (
        <p role="alert" className={styles.error}>
          {t('dm.keyManager.errorPrefix', { detail: state.message })}
        </p>
      )}
      {state.kind === 'ok' && (
        <p role="status" aria-live="polite" className={styles.ok}>
          {state.message}
        </p>
      )}

      <fieldset aria-label={t('dm.keyManager.receiptFieldset')} className={styles.fieldset}>
        <legend className={styles.legend}>{t('dm.keyManager.receiptLegend')}</legend>
        <label className={styles.checkboxLabel}>
          <input
            type="checkbox"
            checked={receiptOptOut}
            onChange={(e) => setReceiptOptOut(e.target.checked)}
          />
          {t('dm.keyManager.receiptOptOut')}
        </label>
      </fieldset>
    </div>
  );
}

export default DmKeyManager;

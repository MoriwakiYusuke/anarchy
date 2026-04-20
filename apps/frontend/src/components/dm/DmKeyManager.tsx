'use client';

/**
 * DmKeyManager (T048) — DM 受信鍵を pallet_messaging に publish/revoke する UI。
 *
 * Contract: contracts/frontend-ui.md §2.4.
 *
 * Backup の export/import は T063 (US2) のため本 MVP には含めない。
 * `<BackupImportDialog />` を再利用する placeholder は後続実装で追加する。
 */

import { useCallback, useState } from 'react';
import {
  getDmMetaAddressFromStealth,
  publishDmKey,
  revokeDmKey,
} from '@/lib/dm/keyManager';
import { useDmStore } from '@/lib/dm/store';
import type { PolkadotSigner } from 'polkadot-api/signer';

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
  const receiptOptOut = useDmStore((s: { receiptOptOut: boolean }) => s.receiptOptOut);
  const setReceiptOptOut = useDmStore(
    (s: { setReceiptOptOut: (v: boolean) => void }) => s.setReceiptOptOut,
  );

  const meta = getDmMetaAddressFromStealth();

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
    <div role="region" aria-label="DM key manager">
      <h3>DM 受信鍵</h3>
      <p>
        状態: <strong>{published ? '公開中' : '未公開'}</strong>
      </p>

      {!meta && (
        <p role="alert">stealth 鍵が読み込まれていません。先に Backup をインポートしてください。</p>
      )}

      <div>
        <button
          type="button"
          onClick={() => void onPublish()}
          disabled={isBusy || !meta || published}
        >
          {state.kind === 'busy' && state.action === 'publish' ? '公開中…' : '公開する'}
        </button>
        <button
          type="button"
          onClick={() => void onRevoke()}
          disabled={isBusy || !published}
        >
          {state.kind === 'busy' && state.action === 'revoke' ? '取り消し中…' : '取り消す'}
        </button>
      </div>

      {state.kind === 'error' && (
        <p role="alert" style={{ color: '#c00' }}>
          エラー: {state.message}
        </p>
      )}
      {state.kind === 'ok' && (
        <p role="status" aria-live="polite">
          {state.message}
        </p>
      )}

      <fieldset aria-label="受信確認設定" style={{ marginTop: '1em' }}>
        <legend>受信確認 (read receipt)</legend>
        <label>
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

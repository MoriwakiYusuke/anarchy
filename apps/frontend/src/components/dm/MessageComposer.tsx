'use client';

/**
 * MessageComposer (T047 + T091 retry UX) — DM 送信フォーム。
 *
 * Contract: contracts/frontend-ui.md §2.3.
 * - FR-025 progress 表示 (5 ステップ)
 * - 残高不足 / 鍵未公開 / 送信失敗のエラー表示
 * - T091: TransactionDropped 時に「再試行」ボタンを残し、最後の `params` を保持
 *   (sender_stealth は pre-fund 済みなので tx2 だけリトライできる設計だが、
 *    現 MVP では sendDm 全体を再実行する単純な retry にとどめる。)
 */

import { useCallback, useState } from 'react';
import { sendDm, type SendDmContext, type SendDmProgress } from '@/lib/dm/sender';
import { useDmStore } from '@/lib/dm/store';
import { useLocale } from '@/i18n';
import type { TranslationKey } from '@/i18n';
import { DmError, type AccountId, type SendDmParams } from '@/lib/dm/types';
import styles from './MessageComposer.module.css';

export interface MessageComposerProps {
  counterparty: AccountId;
  /** 親コンポーネントから注入される送信 context (api / signer / endpoint)。
   *  rendering を testable に保つため hook を通さず props で受け取る。 */
  context: SendDmContext;
  /** 送信成功時のコールバック (任意、UI 側で list を refresh したい時に使用)。 */
  onSent?: () => void;
}

type ComposerState =
  | { kind: 'idle' }
  | { kind: 'sending'; progress: SendDmProgress }
  | { kind: 'error'; banner: ErrorBanner }
  | { kind: 'sent' };

const PROGRESS_KEY: Record<SendDmProgress['kind'], TranslationKey> = {
  encrypting: 'dm.progress.encrypting',
  uploading: 'dm.progress.uploading',
  prefunding: 'dm.progress.prefunding',
  dispatching: 'dm.progress.dispatching',
  done: 'dm.progress.done',
};

interface ErrorBanner {
  /** TranslationKey を解決する側に渡す。 */
  key: TranslationKey;
  /** key が `{detail}` パラメータを取る場合の値。 */
  detail?: string;
  canRetry: boolean;
}

function classifyError(err: unknown): ErrorBanner {
  const msg = err instanceof Error ? err.message : String(err);
  // sender.ts は `${DmError.TransactionDropped}: tx1 ...` のように prefix で補足情報を
  // 付ける場合があるので、完全一致ではなく startsWith で分類する。
  if (msg === DmError.RecipientKeyNotPublished) {
    return { key: 'dm.error.recipientKeyNotPublished', canRetry: false };
  }
  if (msg === DmError.MainAccountInsufficientBalance) {
    return { key: 'dm.error.insufficientBalance', canRetry: false };
  }
  if (msg === DmError.StorageInsufficient) {
    return { key: 'dm.error.storageInsufficient', canRetry: true };
  }
  if (msg.startsWith(DmError.TransactionDropped)) {
    return { key: 'dm.error.transactionDropped', detail: msg, canRetry: true };
  }
  if (msg === DmError.BodyTooLarge) {
    return { key: 'dm.error.bodyTooLarge', canRetry: false };
  }
  return { key: 'dm.error.generic', detail: msg, canRetry: true };
}

export function MessageComposer({ counterparty, context, onSent }: MessageComposerProps): JSX.Element {
  const { t } = useLocale();
  const [body, setBody] = useState('');
  const [state, setState] = useState<ComposerState>({ kind: 'idle' });
  const addOutgoing = useDmStore(
    (s: { addOutgoing: (m: import('@/lib/dm/types').DmMessageRecord) => void }) => s.addOutgoing,
  );

  const submit = useCallback(
    async (paramsOverride?: SendDmParams) => {
      const text = body.trim();
      if (!text && !paramsOverride) return;
      const params: SendDmParams =
        paramsOverride ?? {
          recipientAccountId: counterparty,
          body: new TextEncoder().encode(text),
        };

      setState({ kind: 'sending', progress: { kind: 'encrypting' } });

      try {
        const result = await sendDm(params, {
          ...context,
          onProgress: (progress) => setState({ kind: 'sending', progress }),
        });

        addOutgoing({
          messageId: result.messageId,
          blockNumber: result.blockNumber,
          direction: 'outgoing',
          counterparty,
          timestampMs: Date.now(),
          body: params.body,
          bodyState: 'plaintext',
          deliveryState: 'sent',
        });

        setState({ kind: 'sent' });
        setBody('');
        onSent?.();
      } catch (err) {
        setState({ kind: 'error', banner: classifyError(err) });
      }
    },
    [addOutgoing, body, context, counterparty, onSent],
  );

  const retry = useCallback(() => {
    // T091: 最後に入力していた本文をそのまま再利用 (UI からは消さない)。
    if (!body.trim()) return;
    void submit();
  }, [body, submit]);

  const isBusy = state.kind === 'sending';

  return (
    <div role="region" aria-label="DM compose form" className={styles.region}>
      <textarea
        value={body}
        onChange={(e) => setBody(e.target.value)}
        placeholder={t('dm.compose.bodyPlaceholder')}
        rows={3}
        disabled={isBusy}
        aria-label="dm message body"
        className={styles.textarea}
      />
      <div className={styles.actions}>
        <button
          type="button"
          onClick={() => void submit()}
          disabled={isBusy || !body.trim()}
          className={styles.sendBtn}
        >
          {isBusy ? t('dm.compose.sending') : t('dm.compose.send')}
        </button>
      </div>

      {state.kind === 'sending' && (
        <p role="status" aria-live="polite" className={styles.progress}>
          {state.progress.kind === 'uploading'
            ? t('dm.progress.uploadingProgress', {
                uploaded: state.progress.uploaded,
                total: state.progress.total,
              })
            : t(PROGRESS_KEY[state.progress.kind])}
        </p>
      )}

      {state.kind === 'sent' && (
        <p role="status" aria-live="polite" className={styles.sent}>
          {t('dm.compose.sent')}
        </p>
      )}

      {state.kind === 'error' && (
        <div role="alert" className={styles.error}>
          <p className={styles.errorText}>
            {state.banner.detail
              ? t(state.banner.key, { detail: state.banner.detail })
              : t(state.banner.key)}
          </p>
          {state.banner.canRetry && (
            <button type="button" onClick={retry} className={styles.retryBtn}>
              {t('dm.compose.retry')}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default MessageComposer;

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
import { DmError, type AccountId, type SendDmParams } from '@/lib/dm/types';

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
  | { kind: 'error'; message: string; canRetry: boolean }
  | { kind: 'sent' };

const PROGRESS_LABEL: Record<SendDmProgress['kind'], string> = {
  encrypting: 'コンテンツを暗号化中…',
  uploading: '分散ストレージへ断片を送信中…',
  prefunding: 'ステルスアカウントを準備中… (MORAL 前送金中)',
  dispatching: 'DM を発行中…',
  done: '送信完了',
};

function progressMessage(p: SendDmProgress): string {
  if (p.kind === 'uploading') {
    return `分散ストレージへ断片を送信中… (${p.uploaded}/${p.total} 完了)`;
  }
  return PROGRESS_LABEL[p.kind];
}

function errorBanner(err: unknown): { message: string; canRetry: boolean } {
  const msg = err instanceof Error ? err.message : String(err);
  switch (msg) {
    case DmError.RecipientKeyNotPublished:
      return {
        message: '相手はまだ DM を受け付けていません。',
        canRetry: false,
      };
    case DmError.MainAccountInsufficientBalance:
      return { message: 'MORAL 残高が不足しています。', canRetry: false };
    case DmError.StorageInsufficient:
      return {
        message: 'ストレージノードの応答が足りません。再試行できます。',
        canRetry: true,
      };
    case DmError.TransactionDropped:
      return {
        message: '送信に失敗しました。前送金は維持されているため再試行できます。',
        canRetry: true,
      };
    case DmError.BodyTooLarge:
      return { message: '本文が大きすぎます。', canRetry: false };
    default:
      return { message: `送信エラー: ${msg}`, canRetry: true };
  }
}

export function MessageComposer({ counterparty, context, onSent }: MessageComposerProps): JSX.Element {
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
        const banner = errorBanner(err);
        setState({ kind: 'error', message: banner.message, canRetry: banner.canRetry });
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
    <div role="region" aria-label="DM compose form">
      <textarea
        value={body}
        onChange={(e) => setBody(e.target.value)}
        placeholder="メッセージを入力…"
        rows={3}
        disabled={isBusy}
        aria-label="dm message body"
      />
      <button
        type="button"
        onClick={() => void submit()}
        disabled={isBusy || !body.trim()}
      >
        {isBusy ? '送信中…' : '送信'}
      </button>

      {state.kind === 'sending' && (
        <p role="status" aria-live="polite">
          {progressMessage(state.progress)}
        </p>
      )}

      {state.kind === 'sent' && (
        <p role="status" aria-live="polite">
          送信完了
        </p>
      )}

      {state.kind === 'error' && (
        <div role="alert" style={{ color: '#c00' }}>
          <p>{state.message}</p>
          {state.canRetry && (
            <button type="button" onClick={retry}>
              再試行
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default MessageComposer;

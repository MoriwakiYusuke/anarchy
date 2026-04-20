/**
 * <ConversationView /> — 単一会話の時系列描画 + 返信フォーム (T060 + T094 + T072)。
 *
 * 仕様: contracts/frontend-ui.md §2.2。
 *  - props.conversationId の counterparty に紐付くメッセージを block 昇順で描画。
 *  - bodyState='garbage_collected' のメッセージは <GarbageCollectedBubble /> プレースホルダ。
 *  - 各バブルは data-testid="dm-message-bubble" を付け、direction/state を data-* で公開。
 *  - T072 (US3): `context` prop が渡されたら末尾に <MessageComposer /> を mount して
 *    同スレッドへの返信を可能にする (optimistic addOutgoing は MessageComposer が実施)。
 *
 * パフォーマンス: SC-005 = 10k 件スクロール。MVP では一度に全件レンダリングする
 * (React 18 list virtualization は polish 段階で導入予定)。
 */

'use client';

import { useEffect, useMemo } from 'react';
import { useDmStore, receiptKey } from '@/lib/dm/store';
import { sendDmReceipt } from '@/lib/dm/receipt';
import { MessageComposer } from './MessageComposer';
import type { SendDmContext } from '@/lib/dm/sender';
import type { AccountId, ConversationState, DmMessageRecord } from '@/lib/dm/types';

export interface ConversationViewProps {
  conversationId: AccountId;
  /**
   * T072: 送信 context を受け取ると末尾に <MessageComposer /> を出す。
   * 省略時は read-only なビュー (ex: settings プレビュー)。
   */
  context?: SendDmContext;
}

export function ConversationView({
  conversationId,
  context,
}: ConversationViewProps): JSX.Element {
  const conversations = useDmStore(
    (s: { conversations: Map<AccountId, ConversationState> }) => s.conversations,
  );
  const conv = conversations.get(conversationId);

  const messages = useMemo(() => conv?.messages ?? [], [conv]);

  // T078 / FR-016: スレッドを開いた際に incoming メッセージへ read receipt を送る。
  //   - opt-out (receiptOptOut=true) は `sendDmReceipt` 側でサプレス。
  //   - idempotent: `sentReceipts` に記録済みなら skip (再マウント / hot reload に耐える)。
  //   - context 無し (読み取り専用表示) の場合は emit しない。
  useEffect(() => {
    if (!context) return;
    const store = useDmStore.getState();
    for (const m of messages) {
      if (m.direction !== 'incoming' || m.bodyState !== 'plaintext') continue;
      const key = receiptKey(conversationId, m.messageId, 'read');
      if (store.sentReceipts.has(key)) continue;
      store.rememberReceiptSent(key);
      void sendDmReceipt(
        { counterparty: conversationId, refMessageId: m.messageId, kind: 'read' },
        context,
      ).catch((err) => {
        console.error('[dm-receipt][read]', err);
      });
    }
  }, [context, conversationId, messages]);

  return (
    <section className="dm-conversation-view" aria-label={`会話: ${conversationId}`}>
      <header className="dm-conversation-view__header">
        <h2>{conversationId}</h2>
      </header>
      {messages.length === 0 ? (
        <p className="dm-conversation-view__empty">メッセージはまだありません。</p>
      ) : (
        <ol className="dm-conversation-view__messages">
          {messages.map((m) => (
            <MessageBubble key={messageKey(m)} message={m} />
          ))}
        </ol>
      )}
      {context ? (
        <div className="dm-conversation-view__composer">
          <MessageComposer counterparty={conversationId} context={context} />
        </div>
      ) : null}
    </section>
  );
}

function messageKey(m: DmMessageRecord): string {
  return `${m.blockNumber.toString()}-${m.messageId.toString()}-${m.direction}`;
}

/** T080: 配信状態バッジのラベル (日本語 UI)。 */
const DELIVERY_STATE_LABEL: Record<'sent' | 'delivered' | 'read', string> = {
  sent: '送信済み',
  delivered: '配信済み',
  read: '既読',
};

function MessageBubble({ message }: { message: DmMessageRecord }): JSX.Element {
  if (message.bodyState === 'garbage_collected') {
    return <GarbageCollectedBubble message={message} />;
  }

  const text = decodeBody(message.body);

  return (
    <li
      data-testid="dm-message-bubble"
      data-direction={message.direction}
      data-body-state={message.bodyState}
      className={`dm-message-bubble dm-message-bubble--${message.direction}`}
    >
      <p className="dm-message-bubble__body">{text}</p>
      {message.direction === 'outgoing' && message.deliveryState ? (
        <span
          aria-label="配信状態"
          data-delivery-state={message.deliveryState}
          className={`dm-message-bubble__delivery-state dm-message-bubble__delivery-state--${message.deliveryState}`}
        >
          {DELIVERY_STATE_LABEL[message.deliveryState]}
        </span>
      ) : null}
    </li>
  );
}

/**
 * Phase 3.4 で GC された DM の placeholder。spec.md Edge Cases / FR-018 / T094。
 */
function GarbageCollectedBubble({ message }: { message: DmMessageRecord }): JSX.Element {
  return (
    <li
      data-testid="dm-message-bubble"
      data-direction={message.direction}
      data-body-state="garbage_collected"
      className="dm-message-bubble dm-message-bubble--gc"
    >
      <p className="dm-message-bubble__placeholder">履歴は取得できません</p>
    </li>
  );
}

function decodeBody(body: Uint8Array): string {
  try {
    return new TextDecoder('utf-8', { fatal: false }).decode(body);
  } catch {
    return `[${body.byteLength} bytes]`;
  }
}

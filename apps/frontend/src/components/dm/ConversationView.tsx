/**
 * <ConversationView /> — 単一会話の時系列描画 (T060 + T094)。
 *
 * 仕様: contracts/frontend-ui.md §2.2。
 *  - props.conversationId の counterparty に紐付くメッセージを block 昇順で描画。
 *  - bodyState='garbage_collected' のメッセージは <GarbageCollectedBubble /> プレースホルダ。
 *  - 各バブルは data-testid="dm-message-bubble" を付け、direction/state を data-* で公開。
 *
 * パフォーマンス: SC-005 = 10k 件スクロール。MVP では一度に全件レンダリングする
 * (React 18 list virtualization は polish 段階で導入予定)。
 */

'use client';

import { useMemo } from 'react';
import { useDmStore } from '@/lib/dm/store';
import type { AccountId, ConversationState, DmMessageRecord } from '@/lib/dm/types';

export interface ConversationViewProps {
  conversationId: AccountId;
}

export function ConversationView({ conversationId }: ConversationViewProps): JSX.Element {
  const conversations = useDmStore(
    (s: { conversations: Map<AccountId, ConversationState> }) => s.conversations,
  );
  const conv = conversations.get(conversationId);

  const messages = useMemo(() => conv?.messages ?? [], [conv]);

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
    </section>
  );
}

function messageKey(m: DmMessageRecord): string {
  return `${m.blockNumber.toString()}-${m.messageId.toString()}-${m.direction}`;
}

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
        <span aria-label="配信状態" className="dm-message-bubble__delivery-state">
          {message.deliveryState}
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

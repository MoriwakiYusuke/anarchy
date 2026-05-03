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
import { useNicknameOf } from '@/hooks/useNicknameOf';
import { useAccount } from '@/lib/account/context';
import { useLocale } from '@/i18n';
import type { TranslationKey } from '@/i18n';
import { MessageComposer } from './MessageComposer';
import { DmMediaDisplay } from './DmMediaDisplay';
import type { SendDmContext } from '@/lib/dm/sender';
import { debugError } from '@/lib/debugLog';
import type { AccountId, ConversationState, DmMessageRecord } from '@/lib/dm/types';
import { decodeDmContent } from '@/lib/dm/contentCodec';
import styles from './ConversationView.module.css';

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
  const { t } = useLocale();
  const conversations = useDmStore(
    (s: { conversations: Map<AccountId, ConversationState> }) => s.conversations,
  );
  const conv = conversations.get(conversationId);
  const nickname = useNicknameOf(conversationId);
  const { account: ownAccount } = useAccount();
  const isSelf = ownAccount === conversationId;
  // 表示優先度: 1) on-chain nickname / 2) 自分宛なら "(あなた)" / 3) NicknameSettings
  // home 画面の DEFAULT_NAME と揃えて "Anarchy"。SS58 全文は常に下に表示する。
  const displayName = nickname
    ? nickname
    : isSelf
      ? t('dm.view.selfName')
      : 'Anarchy';

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
        debugError('[dm-receipt][read]', err);
      });
    }
  }, [context, conversationId, messages]);

  return (
    <section className={styles.view} aria-label={`${t('dm.view.counterpartyLabel')}: ${conversationId}`}>
      <header className={styles.header}>
        <h2 className={styles.headerTitle}>
          {isSelf ? t('dm.view.selfLabel') : t('dm.view.counterpartyLabel')}
          <span className={styles.nickname}>{displayName}</span>
          <span className={styles.counterpartySub}>{conversationId}</span>
        </h2>
      </header>
      {messages.length === 0 ? (
        <p className={styles.empty}>{t('dm.view.empty')}</p>
      ) : (
        <ol className={styles.messages}>
          {messages.map((m) => (
            <MessageBubble
              key={messageKey(m)}
              message={m}
              chainRpcEndpoint={context?.chainRpcEndpoint}
            />
          ))}
        </ol>
      )}
      {context ? (
        <div className={styles.composer}>
          <MessageComposer counterparty={conversationId} context={context} />
        </div>
      ) : null}
    </section>
  );
}

function messageKey(m: DmMessageRecord): string {
  return `${m.blockNumber.toString()}-${m.messageId.toString()}-${m.direction}`;
}

/** T080: 配信状態バッジのラベル i18n key マップ。 */
const DELIVERY_STATE_LABEL_KEY: Record<'sent' | 'delivered' | 'read', TranslationKey> = {
  sent: 'dm.view.deliveryStateSent',
  delivered: 'dm.view.deliveryStateDelivered',
  read: 'dm.view.deliveryStateRead',
};

function MessageBubble({
  message,
  chainRpcEndpoint,
}: {
  message: DmMessageRecord;
  chainRpcEndpoint?: string;
}): JSX.Element {
  const { t } = useLocale();
  // memoize して body の参照が変わるまで decoded を再評価しない (DmMediaDisplay の
  // useEffect が media 配列の identity に依存して再起動するのを防ぐ)。
  const decoded = useMemo(
    () => (message.bodyState === 'plaintext' ? decodeDmContent(message.body) : null),
    [message.body, message.bodyState],
  );
  if (message.bodyState === 'garbage_collected') {
    return <GarbageCollectedBubble message={message} />;
  }

  const text = decoded?.text ?? '';
  const media = decoded?.media ?? [];

  const bubbleClass =
    message.direction === 'outgoing'
      ? `${styles.bubble} ${styles.bubbleOutgoing}`
      : `${styles.bubble} ${styles.bubbleIncoming}`;
  const deliveryClass =
    message.deliveryState === 'read'
      ? `${styles.deliveryState} ${styles.deliveryStateRead}`
      : message.deliveryState === 'delivered'
        ? `${styles.deliveryState} ${styles.deliveryStateDelivered}`
        : `${styles.deliveryState} ${styles.deliveryStateSent}`;

  return (
    <li
      data-testid="dm-message-bubble"
      data-direction={message.direction}
      data-body-state={message.bodyState}
      className={bubbleClass}
    >
      {text ? <p className={styles.body}>{text}</p> : null}
      {media.length > 0 ? (
        <DmMediaDisplay media={media} chainRpcEndpoint={chainRpcEndpoint} />
      ) : null}
      {message.direction === 'outgoing' && message.deliveryState ? (
        <span
          aria-label={t('dm.view.deliveryStateAriaLabel')}
          data-delivery-state={message.deliveryState}
          className={deliveryClass}
        >
          {t(DELIVERY_STATE_LABEL_KEY[message.deliveryState])}
        </span>
      ) : null}
    </li>
  );
}

/**
 * Phase 3.4 で GC された DM の placeholder。spec.md Edge Cases / FR-018 / T094。
 */
function GarbageCollectedBubble({ message }: { message: DmMessageRecord }): JSX.Element {
  const { t } = useLocale();
  return (
    <li
      data-testid="dm-message-bubble"
      data-direction={message.direction}
      data-body-state="garbage_collected"
      className={`${styles.bubble} ${styles.bubbleGc}`}
    >
      <p className={styles.placeholder}>{t('dm.view.gcPlaceholder')}</p>
    </li>
  );
}


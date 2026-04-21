/**
 * <ConversationList /> — DM スレッド一覧 (T059)。
 *
 * 仕様: contracts/frontend-ui.md §2.1。
 *  - dmStore.conversations を最終アクティビティ block 降順で並べる。
 *  - blockList に含まれる counterparty は非表示。
 *  - 未読バッジ (unreadCount > 0) を `aria-label="未読件数"` で描画。
 *  - 空のときはガイダンス文言。
 *
 * MVP では route 連携 (`/dm/[conversationId]` への遷移) は親側責務。本コンポーネントは
 * onSelect コールバックを受けるだけにとどめ、Next/Link は使わない (テスト容易性)。
 */

'use client';

import { useMemo } from 'react';
import { useDmStore } from '@/lib/dm/store';
import type { AccountId, ConversationState } from '@/lib/dm/types';
import styles from './ConversationList.module.css';

export interface ConversationListProps {
  onSelect?: (counterparty: AccountId) => void;
}

export function ConversationList({ onSelect }: ConversationListProps): JSX.Element {
  const conversations = useDmStore(
    (s: { conversations: Map<AccountId, ConversationState> }) => s.conversations,
  );
  const blockList = useDmStore((s: { blockList: Set<AccountId> }) => s.blockList);

  const visibleThreads = useMemo(() => {
    const all = Array.from(conversations.values()).filter(
      (c) => !c.blocked && !blockList.has(c.counterparty),
    );
    return all.sort((a, b) => {
      if (a.lastActivityBlock === b.lastActivityBlock) return 0;
      return a.lastActivityBlock < b.lastActivityBlock ? 1 : -1;
    });
  }, [conversations, blockList]);

  if (visibleThreads.length === 0) {
    return (
      <div role="status" className={styles.empty}>
        まだメッセージはありません。新しい DM を送信するとここに表示されます。
      </div>
    );
  }

  return (
    <ul className={styles.list} role="list">
      {visibleThreads.map((thread) => (
        <li
          key={thread.counterparty}
          role="listitem"
          className={styles.item}
        >
          <button
            type="button"
            onClick={() => onSelect?.(thread.counterparty)}
            className={styles.row}
          >
            <span className={styles.counterparty}>
              {thread.counterparty}
            </span>
            <span className={styles.lastBlock}>
              #{thread.lastActivityBlock.toString()}
            </span>
            {thread.unreadCount > 0 ? (
              <span
                aria-label="未読件数"
                className={styles.unreadBadge}
              >
                {thread.unreadCount}
              </span>
            ) : null}
          </button>
        </li>
      ))}
    </ul>
  );
}

/**
 * T056: <ConversationList /> Testing Library テスト。
 *
 * 受入条件 (contracts/frontend-ui.md §2.1):
 *   - 3 人からのメッセージがストアにあるとき 3 スレッド表示。
 *   - ブロック中のアカウントは非表示。
 *   - 各スレッドに未読バッジ (unreadCount) が描画される。
 */

import { render, screen, within } from '@testing-library/react';
import { ConversationList } from '../ConversationList';
import { useDmStore } from '@/lib/dm/store';
import type { AccountId, DmMessageRecord } from '@/lib/dm/types';

function msg(counterparty: string, blockNumber: bigint, messageId: bigint): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction: 'incoming',
    counterparty: counterparty as AccountId,
    timestampMs: Number(blockNumber) * 6000,
    body: new Uint8Array([0x01]),
    bodyState: 'plaintext',
    signatureValid: true,
  };
}

beforeEach(() => {
  useDmStore.setState({
    conversations: new Map(),
    blockList: new Set(),
    lastScannedBlock: 0n,
    isScanning: false,
  });
});

describe('<ConversationList />', () => {
  it('renders 3 threads when 3 senders are present', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n));
    useDmStore.getState().addIncoming(msg('5Bob', 11n, 2n));
    useDmStore.getState().addIncoming(msg('5Charlie', 12n, 3n));

    render(<ConversationList />);
    const items = screen.getAllByRole('listitem');
    expect(items).toHaveLength(3);
  });

  it('hides blocked counterparty (FR-011)', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n));
    useDmStore.getState().addIncoming(msg('5Bob', 11n, 2n));
    useDmStore.getState().blockSender('5Bob' as AccountId);

    render(<ConversationList />);
    const items = screen.getAllByRole('listitem');
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain('5Alice');
  });

  it('renders unread badge when conversation has unread incoming messages', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n));
    useDmStore.getState().addIncoming(msg('5Alice', 11n, 2n));

    render(<ConversationList />);
    const items = screen.getAllByRole('listitem');
    const badge = within(items[0]).getByLabelText('未読件数');
    expect(badge.textContent).toBe('2');
  });

  it('does not render unread badge when count is 0', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n));
    useDmStore.getState().markAsRead('5Alice' as AccountId, 1n);

    render(<ConversationList />);
    const items = screen.getAllByRole('listitem');
    expect(within(items[0]).queryByLabelText('未読件数')).toBeNull();
  });

  it('orders threads most-recent first by lastActivityBlock desc', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 100n, 1n));
    useDmStore.getState().addIncoming(msg('5Bob', 50n, 2n));
    useDmStore.getState().addIncoming(msg('5Charlie', 200n, 3n));

    render(<ConversationList />);
    const items = screen.getAllByRole('listitem');
    expect(items[0].textContent).toContain('5Charlie');
    expect(items[1].textContent).toContain('5Alice');
    expect(items[2].textContent).toContain('5Bob');
  });

  it('renders empty state when there are no conversations', () => {
    render(<ConversationList />);
    expect(screen.getByText(/まだメッセージはありません/)).toBeInTheDocument();
  });
});

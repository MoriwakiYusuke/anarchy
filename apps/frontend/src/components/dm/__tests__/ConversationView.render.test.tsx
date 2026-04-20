/**
 * T057: <ConversationView /> 描画テスト。
 *
 * 受入条件 (contracts/frontend-ui.md §2.2):
 *   - 20 件ある会話で全件 chronological に表示 (欠落・重複なし)。
 *   - incoming / outgoing が方向別にマークされる。
 */

import { render, screen, within } from '@testing-library/react';
import { ConversationView } from '../ConversationView';
import { useDmStore } from '@/lib/dm/store';
import type { AccountId, DmMessageRecord } from '@/lib/dm/types';

function msg(
  counterparty: string,
  blockNumber: bigint,
  messageId: bigint,
  body: string,
  direction: 'incoming' | 'outgoing' = 'incoming',
): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction,
    counterparty: counterparty as AccountId,
    timestampMs: Number(blockNumber) * 6000,
    body: new TextEncoder().encode(body),
    bodyState: 'plaintext',
    signatureValid: direction === 'incoming' ? true : undefined,
    deliveryState: direction === 'outgoing' ? 'sent' : undefined,
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

describe('<ConversationView />', () => {
  it('renders 20 messages chronologically with no gaps or duplicates', () => {
    for (let i = 1; i <= 20; i += 1) {
      useDmStore.getState().addIncoming(
        msg('5Alice', BigInt(i * 10), BigInt(i), `m${i}`),
      );
    }

    render(<ConversationView conversationId={'5Alice' as AccountId} />);
    const bubbles = screen.getAllByTestId('dm-message-bubble');
    expect(bubbles).toHaveLength(20);

    // 順序が m1, m2, ..., m20 か (block 昇順 = chronological)。
    bubbles.forEach((b, idx) => {
      expect(b.textContent).toContain(`m${idx + 1}`);
    });
  });

  it('renders incoming and outgoing messages with direction-specific markers', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'in'));
    useDmStore.getState().addOutgoing(msg('5Alice', 11n, 2n, 'out', 'outgoing'));

    render(<ConversationView conversationId={'5Alice' as AccountId} />);
    const bubbles = screen.getAllByTestId('dm-message-bubble');
    expect(bubbles).toHaveLength(2);
    expect(bubbles[0].getAttribute('data-direction')).toBe('incoming');
    expect(bubbles[1].getAttribute('data-direction')).toBe('outgoing');
  });

  it('shows the counterparty header', () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'hi'));
    render(<ConversationView conversationId={'5Alice' as AccountId} />);
    expect(screen.getByRole('heading')).toHaveTextContent('5Alice');
  });

  it('renders an empty placeholder when the conversation has no messages', () => {
    render(<ConversationView conversationId={'5Unknown' as AccountId} />);
    expect(screen.getByText(/メッセージはまだありません/)).toBeInTheDocument();
  });

  it('renders delivery state badge for outgoing messages', () => {
    useDmStore.getState().addOutgoing(msg('5Alice', 10n, 1n, 'sent-msg', 'outgoing'));

    render(<ConversationView conversationId={'5Alice' as AccountId} />);
    const bubble = screen.getByTestId('dm-message-bubble');
    expect(within(bubble).getByLabelText('配信状態').textContent).toBe('sent');
  });
});

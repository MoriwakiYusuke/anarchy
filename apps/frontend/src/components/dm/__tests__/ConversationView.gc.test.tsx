/**
 * T093 (UI side): GC 済み DM の placeholder 表示テスト。
 *
 * 検証:
 *   - bodyState='garbage_collected' のメッセージは「履歴は取得できません」プレースホルダで描画。
 *   - 通常の plaintext メッセージは body 文字列を描画。
 *   - GC バブルでもクラッシュせず、空文字列も表示しない (バグ回避)。
 *
 * Contract: spec.md Edge Cases "Message garbage-collected" / FR-018。
 */

import { render, screen } from '@testing-library/react';
import { ConversationView } from '../ConversationView';
import { useDmStore } from '@/lib/dm/store';
import { encodeDmContent } from '@/lib/dm/contentCodec';
import type { AccountId, DmMessageRecord } from '@/lib/dm/types';

function gcMsg(counterparty: string, blockNumber: bigint, messageId: bigint): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction: 'incoming',
    counterparty: counterparty as AccountId,
    timestampMs: Number(blockNumber) * 6000,
    body: new Uint8Array(),
    bodyState: 'garbage_collected',
    signatureValid: false,
  };
}

function plainMsg(
  counterparty: string,
  blockNumber: bigint,
  messageId: bigint,
  body: string,
): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction: 'incoming',
    counterparty: counterparty as AccountId,
    timestampMs: Number(blockNumber) * 6000,
    body: encodeDmContent({ text: body, media: [] }),
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

describe('<ConversationView /> — GC placeholder (T093)', () => {
  it('renders the GC placeholder for garbage_collected messages', () => {
    useDmStore.getState().addIncoming(gcMsg('5Alice', 10n, 1n));

    render(<ConversationView conversationId={'5Alice' as AccountId} />);
    expect(screen.getByText(/履歴は取得できません/)).toBeInTheDocument();
  });

  it('mixes plaintext and GC bubbles in chronological order', () => {
    useDmStore.getState().addIncoming(plainMsg('5Alice', 10n, 1n, 'hello'));
    useDmStore.getState().addIncoming(gcMsg('5Alice', 11n, 2n));
    useDmStore.getState().addIncoming(plainMsg('5Alice', 12n, 3n, 'world'));

    render(<ConversationView conversationId={'5Alice' as AccountId} />);
    const bubbles = screen.getAllByTestId('dm-message-bubble');
    expect(bubbles).toHaveLength(3);
    expect(bubbles[0].textContent).toContain('hello');
    expect(bubbles[1].textContent).toMatch(/履歴は取得できません/);
    expect(bubbles[2].textContent).toContain('world');
  });

  it('does not render an empty body for GC bubbles', () => {
    useDmStore.getState().addIncoming(gcMsg('5Alice', 10n, 1n));

    render(<ConversationView conversationId={'5Alice' as AccountId} />);
    const bubble = screen.getByTestId('dm-message-bubble');
    expect(bubble.textContent?.trim().length).toBeGreaterThan(0);
  });
});

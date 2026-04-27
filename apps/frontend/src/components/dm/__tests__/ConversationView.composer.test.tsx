/**
 * T070: <MessageComposer /> embedded in <ConversationView /> テスト (US3)。
 *
 * Asserts:
 *   - ConversationView が MessageComposer を bottom に mount している。
 *   - 返信入力 → 送信ボタン押下で `sendDm` が起動される。
 *   - 成功時、optimistic に addOutgoing で store に入り、bubble が表示される。
 *   - 入力欄が空に戻る (success path clears input)。
 *
 * Contract: contracts/frontend-ui.md §2.2 / §2.3, tasks.md T070。
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

import { render, screen, waitFor, fireEvent, act } from '@testing-library/react';
import { ConversationView } from '../ConversationView';
import { useDmStore } from '@/lib/dm/store';
import type {
  AccountId,
  DmMessageRecord,
  SendDmParams,
  SendDmResult,
} from '@/lib/dm/types';
import type { SendDmContext } from '@/lib/dm/sender';

// sendDm をモック: 成功時に SendDmResult を返し、進捗コールバックを順次呼ぶ。
jest.mock('@/lib/dm/sender', () => ({
  sendDm: jest.fn(),
}));

import { sendDm } from '@/lib/dm/sender';

const mockedSendDm = sendDm as jest.MockedFunction<typeof sendDm>;

const COUNTERPARTY = '5Alice' as AccountId;

function incomingMsg(blockNumber: bigint, messageId: bigint, body: string): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction: 'incoming',
    counterparty: COUNTERPARTY,
    timestampMs: Number(blockNumber) * 6000,
    body: new TextEncoder().encode(body),
    bodyState: 'plaintext',
    signatureValid: true,
  };
}

function fakeContext(): SendDmContext {
  return {
    api: {} as any,
    mainSigner: {
      publicKey: new Uint8Array(32).fill(0xaa),
      signBytes: async () => new Uint8Array(64),
      signTx: async () => new Uint8Array(64),
    } as any,
    mainAccountPublicKey: new Uint8Array(32).fill(0xaa),
    chainRpcEndpoint: 'http://localhost:9944',
  };
}

beforeEach(() => {
  jest.clearAllMocks();
  useDmStore.setState({
    conversations: new Map(),
    blockList: new Set(),
    lastScannedBlock: 0n,
    isScanning: false,
  });
});

describe('<ConversationView /> + <MessageComposer /> (T070 / US3)', () => {
  it('mounts MessageComposer at the bottom of the conversation view', () => {
    useDmStore.getState().addIncoming(incomingMsg(10n, 1n, 'hello'));

    render(
      <ConversationView conversationId={COUNTERPARTY} context={fakeContext()} />,
    );

    // Composer region (aria-label="DM compose form") が存在する。
    expect(screen.getByRole('region', { name: 'DM compose form' })).toBeInTheDocument();
    expect(screen.getByLabelText('dm message body')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /送信/ })).toBeInTheDocument();
  });

  it('sends a reply, optimistically updates the store, and clears the input on success', async () => {
    useDmStore.getState().addIncoming(incomingMsg(10n, 1n, 'hi'));

    const result: SendDmResult = {
      messageId: 42n,
      blockNumber: 101n,
      recipientStealth: '5StealthAccountPlaceholder',
      merkleRoot: new Uint8Array(32).fill(0x12),
      paddingBucket: 1024,
      totalCostMoral: 10_000_000_000_000n,
    };
    mockedSendDm.mockResolvedValueOnce(result);

    render(
      <ConversationView conversationId={COUNTERPARTY} context={fakeContext()} />,
    );

    const textarea = screen.getByLabelText('dm message body') as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: 'reply from bob' } });
    expect(textarea.value).toBe('reply from bob');

    const sendBtn = screen.getByRole('button', { name: /送信/ });
    await act(async () => {
      fireEvent.click(sendBtn);
    });

    await waitFor(() => {
      expect(mockedSendDm).toHaveBeenCalledTimes(1);
    });

    const [params] = mockedSendDm.mock.calls[0] as [SendDmParams, SendDmContext];
    expect(params.recipientAccountId).toBe(COUNTERPARTY);
    expect(new TextDecoder().decode(params.body)).toBe('reply from bob');

    await waitFor(() => {
      const conv = useDmStore.getState().conversations.get(COUNTERPARTY);
      expect(conv?.messages.some((m) => m.direction === 'outgoing')).toBe(true);
    });

    const conv = useDmStore.getState().conversations.get(COUNTERPARTY);
    expect(conv?.messages).toHaveLength(2);
    const outgoing = conv!.messages.find((m) => m.direction === 'outgoing')!;
    expect(outgoing.messageId).toBe(42n);
    expect(outgoing.blockNumber).toBe(101n);
    expect(outgoing.counterparty).toBe(COUNTERPARTY);

    // 入力欄が success 時にクリアされる。
    await waitFor(() => {
      expect((screen.getByLabelText('dm message body') as HTMLTextAreaElement).value).toBe('');
    });
  });

  it('does not render the composer when `context` prop is omitted (read-only fallback)', () => {
    useDmStore.getState().addIncoming(incomingMsg(10n, 1n, 'hi'));

    render(<ConversationView conversationId={COUNTERPARTY} />);

    // context 無しでは compose form を描画しない (接続中表示などに任せる)。
    expect(screen.queryByRole('region', { name: 'DM compose form' })).toBeNull();
    // ただし bubble 自体は表示される。
    expect(screen.getByTestId('dm-message-bubble')).toBeInTheDocument();
  });
});

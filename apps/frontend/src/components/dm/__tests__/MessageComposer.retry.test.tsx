/**
 * <MessageComposer /> retry 時の添付再アップロード抑止テスト。
 *
 * Review finding: phase-1 (媒体 upload) が成功した後に sendDm (extrinsic) が
 * 失敗して retry すると、全ファイルを再 fragment / 再 upload (= 再課金) していた。
 *
 * Asserts:
 *   - sendDm 失敗 → retry で uploadDmMedia が同一ファイルに対して再実行されない。
 *   - retry の sendDm には初回 upload の DmMediaRef がそのまま渡される。
 *   - エラー後に添付を外して retry すると、外したファイルの ref は envelope に
 *     含まれない (キャッシュ invalidate)。
 *   - 送信成功後の次回送信ではキャッシュが効かない (クリア済み)。
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

import { render, screen, waitFor, fireEvent, act } from '@testing-library/react';
import { MessageComposer } from '../MessageComposer';
import { decodeDmContent } from '@/lib/dm/contentCodec';
import type { AccountId, DmMediaRef, SendDmParams, SendDmResult } from '@/lib/dm/types';
import type { SendDmContext } from '@/lib/dm/sender';

jest.mock('@/lib/dm/sender', () => ({
  sendDm: jest.fn(),
  estimateDmCostFromInputs: () => 3_500_000_000_000n,
  formatMoral: (v: bigint) => `${v / 1_000_000_000_000n}.00`,
}));

jest.mock('@/lib/dm/media', () => ({
  uploadDmMedia: jest.fn(),
}));

import { sendDm } from '@/lib/dm/sender';
import { uploadDmMedia } from '@/lib/dm/media';

const mockedSendDm = sendDm as jest.MockedFunction<typeof sendDm>;
const mockedUpload = uploadDmMedia as jest.MockedFunction<typeof uploadDmMedia>;

const COUNTERPARTY = '5Alice' as AccountId;

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

function fakeMediaRef(name: string): DmMediaRef {
  return {
    root: '11'.repeat(32),
    key: '22'.repeat(32),
    mime: 'application/octet-stream',
    size: 4,
    k: 3,
    n: 5,
    ciphertextLen: 32,
    // name は型に無いが root を変えて区別する
  } as DmMediaRef & { root: string };
}

const SEND_RESULT: SendDmResult = {
  messageId: 1n,
  blockNumber: 100n,
  recipientStealth: '5Stealth',
  merkleRoot: new Uint8Array(32),
  paddingBucket: 1024,
  totalCostMoral: 10_000_000_000_000n,
};

/** kind 'file' (画像/動画以外) にして jsdom 未実装の URL.createObjectURL を回避する。 */
function attachFile(container: HTMLElement, name = 'doc.bin'): void {
  const input = container.querySelector('input[type="file"]') as HTMLInputElement;
  const file = new File([new Uint8Array([1, 2, 3, 4])], name, {
    type: 'application/octet-stream',
  });
  fireEvent.change(input, { target: { files: [file] } });
}

async function clickSend(): Promise<void> {
  const btn = screen.getByRole('button', { name: /送信/ });
  await act(async () => {
    fireEvent.click(btn);
  });
}

async function clickRetry(): Promise<void> {
  const btn = await screen.findByRole('button', { name: /再試行|リトライ|retry/i });
  await act(async () => {
    fireEvent.click(btn);
  });
}

beforeEach(() => {
  jest.clearAllMocks();
});

describe('<MessageComposer /> retry upload cache', () => {
  it('does not re-upload attachments when retrying after a failed sendDm', async () => {
    const ref = fakeMediaRef('doc.bin');
    mockedUpload.mockResolvedValue(ref);
    mockedSendDm
      .mockRejectedValueOnce(new Error('TransactionDropped: dropped'))
      .mockResolvedValueOnce(SEND_RESULT);

    const { container } = render(
      <MessageComposer counterparty={COUNTERPARTY} context={fakeContext()} />,
    );

    fireEvent.change(screen.getByLabelText('dm message body'), {
      target: { value: 'with attachment' },
    });
    attachFile(container);

    await clickSend();
    await waitFor(() => expect(mockedSendDm).toHaveBeenCalledTimes(1));
    expect(mockedUpload).toHaveBeenCalledTimes(1);

    // sendDm 失敗 → エラーバナー + retry ボタン
    expect(screen.getByRole('alert')).toBeInTheDocument();

    await clickRetry();
    await waitFor(() => expect(mockedSendDm).toHaveBeenCalledTimes(2));

    // 再アップロードされていない (キャッシュヒット)
    expect(mockedUpload).toHaveBeenCalledTimes(1);

    // retry の envelope に初回 upload の ref がそのまま入っている
    const [params] = mockedSendDm.mock.calls[1] as [SendDmParams, SendDmContext];
    const decoded = decodeDmContent(params.body);
    expect(decoded?.media).toHaveLength(1);
    expect(decoded?.media[0].root).toBe(ref.root);
  });

  it('invalidates the cached ref when the attachment is removed before retry', async () => {
    mockedUpload.mockResolvedValue(fakeMediaRef('doc.bin'));
    mockedSendDm
      .mockRejectedValueOnce(new Error('TransactionDropped: dropped'))
      .mockResolvedValueOnce(SEND_RESULT);

    const { container } = render(
      <MessageComposer counterparty={COUNTERPARTY} context={fakeContext()} />,
    );

    fireEvent.change(screen.getByLabelText('dm message body'), {
      target: { value: 'will drop attachment' },
    });
    attachFile(container);

    await clickSend();
    await waitFor(() => expect(mockedSendDm).toHaveBeenCalledTimes(1));

    // エラー後に添付を外す
    const removeBtn = screen.getByRole('button', { name: /doc\.bin/ });
    fireEvent.click(removeBtn);

    await clickRetry();
    await waitFor(() => expect(mockedSendDm).toHaveBeenCalledTimes(2));

    // 外したファイルの ref が envelope に混入しない
    const [params] = mockedSendDm.mock.calls[1] as [SendDmParams, SendDmContext];
    const decoded = decodeDmContent(params.body);
    expect(decoded?.media).toEqual([]);
    // 再アップロードもされない
    expect(mockedUpload).toHaveBeenCalledTimes(1);
  });

  it('clears the cache after a successful send (next message uploads fresh)', async () => {
    mockedUpload.mockResolvedValue(fakeMediaRef('doc.bin'));
    mockedSendDm.mockResolvedValue(SEND_RESULT);

    const { container } = render(
      <MessageComposer counterparty={COUNTERPARTY} context={fakeContext()} />,
    );

    fireEvent.change(screen.getByLabelText('dm message body'), {
      target: { value: 'first' },
    });
    attachFile(container, 'a.bin');

    await clickSend();
    await waitFor(() => expect(mockedSendDm).toHaveBeenCalledTimes(1));
    expect(mockedUpload).toHaveBeenCalledTimes(1);

    // 成功後は files / キャッシュがクリアされ、次の添付は新規 upload になる
    fireEvent.change(screen.getByLabelText('dm message body'), {
      target: { value: 'second' },
    });
    attachFile(container, 'b.bin');

    await clickSend();
    await waitFor(() => expect(mockedSendDm).toHaveBeenCalledTimes(2));
    expect(mockedUpload).toHaveBeenCalledTimes(2);
  });
});

/**
 * T076: opt-out による receipt 抑止テスト (US4 / FR-016b)。
 *
 * Asserts:
 *   - receiptOptOut = true のとき **delivered / read 両方とも** sendDm を呼ばずに null を返す
 *     (delivered だけでも "受信側オンライン時刻" がリークするため、UI ラベル
 *     "Do not send read receipts" の利用者期待 = "受信メタデータを送らない" を満たす)。
 *   - receiptOptOut = false なら両 kind とも送信される。
 *   - 送信時 body は `encodeReceiptBody` と bit-for-bit 一致する (wire format 契約)。
 *
 * Contract: spec.md FR-016b, receipt.ts wire format。
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

jest.mock('@/lib/dm/sender', () => ({
  sendDm: jest.fn(),
}));

import { sendDm } from '@/lib/dm/sender';
import {
  sendDmReceipt,
  encodeReceiptBody,
  decodeReceiptBody,
  RECEIPT_MAGIC,
  RECEIPT_BODY_LENGTH,
} from '../receipt';
import { useDmStore } from '../store';
import type { AccountId } from '../types';
import type { SendDmContext } from '../sender';

const mockedSendDm = sendDm as jest.MockedFunction<typeof sendDm>;

function fakeContext(): SendDmContext {
  return {
    api: {} as any,
    mainSigner: {} as any,
    mainAccountPublicKey: new Uint8Array(32).fill(0xaa),
    chainRpcEndpoint: 'http://localhost:9944',
  };
}

const COUNTERPARTY = '5Alice' as AccountId;

beforeEach(() => {
  jest.clearAllMocks();
  useDmStore.setState({
    conversations: new Map(),
    blockList: new Set(),
    lastScannedBlock: 0n,
    isScanning: false,
    receiptOptOut: false,
  });
  mockedSendDm.mockResolvedValue({
    messageId: 99n,
    blockNumber: 500n,
    recipientStealth: '5stealth' as AccountId,
    merkleRoot: new Uint8Array(32).fill(0xde),
    paddingBucket: 1024,
    totalCostMoral: 10_000_000_000_000n,
  });
});

describe('sendDmReceipt — FR-016b opt-out (T076)', () => {
  it('suppresses kind="read" when receiptOptOut is true (returns null, sendDm not called)', async () => {
    useDmStore.getState().setReceiptOptOut(true);

    const result = await sendDmReceipt(
      { counterparty: COUNTERPARTY, refMessageId: 42n, kind: 'read' },
      fakeContext(),
    );

    expect(result).toBeNull();
    expect(mockedSendDm).not.toHaveBeenCalled();
  });

  it('also suppresses kind="delivered" when receiptOptOut is true (delivered timing is metadata)', async () => {
    useDmStore.getState().setReceiptOptOut(true);

    const result = await sendDmReceipt(
      { counterparty: COUNTERPARTY, refMessageId: 42n, kind: 'delivered' },
      fakeContext(),
    );

    expect(result).toBeNull();
    expect(mockedSendDm).not.toHaveBeenCalled();
  });

  it('sends kind="read" when receiptOptOut is false', async () => {
    useDmStore.getState().setReceiptOptOut(false);

    await sendDmReceipt(
      { counterparty: COUNTERPARTY, refMessageId: 42n, kind: 'read' },
      fakeContext(),
    );

    expect(mockedSendDm).toHaveBeenCalledTimes(1);
  });

  it('encodes body as MAGIC || kind || refMessageId_le and round-trips through decodeReceiptBody', async () => {
    await sendDmReceipt(
      { counterparty: COUNTERPARTY, refMessageId: 0x1234_5678_9abc_def0n, kind: 'read' },
      fakeContext(),
    );

    const [params] = mockedSendDm.mock.calls[0];
    expect(params.body.length).toBe(RECEIPT_BODY_LENGTH);
    // MAGIC prefix 一致。
    for (let i = 0; i < RECEIPT_MAGIC.length; i += 1) {
      expect(params.body[i]).toBe(RECEIPT_MAGIC[i]);
    }

    // 再パースして kind / messageId が戻る。
    const decoded = decodeReceiptBody(params.body);
    expect(decoded).toEqual({ kind: 'read', refMessageId: 0x1234_5678_9abc_def0n });
  });

  it('passes the counterparty straight through as the DM recipient', async () => {
    await sendDmReceipt(
      { counterparty: COUNTERPARTY, refMessageId: 1n, kind: 'delivered' },
      fakeContext(),
    );

    const [params] = mockedSendDm.mock.calls[0];
    expect(params.recipientAccountId).toBe(COUNTERPARTY);
  });
});

describe('receipt wire format (T077)', () => {
  it('encodeReceiptBody / decodeReceiptBody round-trip for delivered', () => {
    const body = encodeReceiptBody('delivered', 0n);
    expect(decodeReceiptBody(body)).toEqual({ kind: 'delivered', refMessageId: 0n });
  });

  it('decodeReceiptBody returns null for non-receipt bodies', () => {
    expect(decodeReceiptBody(new TextEncoder().encode('hello world'))).toBeNull();
    expect(decodeReceiptBody(new Uint8Array(12))).toBeNull(); // too short
    // MAGIC 合致だが kind が 0 = unknown。
    const invalid = new Uint8Array(RECEIPT_BODY_LENGTH);
    invalid.set(RECEIPT_MAGIC, 0);
    invalid[RECEIPT_MAGIC.length] = 0;
    expect(decodeReceiptBody(invalid)).toBeNull();
  });
});

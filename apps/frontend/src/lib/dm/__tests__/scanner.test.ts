/**
 * T033: scanner.scanDmInbox の Jest テスト。
 *
 * Asserts:
 *   - dm_decrypt_scan が Some を返した dispatch が newMessages に入る。
 *   - signature_valid: false の dispatch は除外される (FR-004)。
 *   - scannedToBlock が bestHead に追従する (lastScannedBlock 更新の根拠)。
 *   - 1024 ブロック以上のレンジで dispatches_range が複数回呼ばれる (paging)。
 *
 * Contract: contracts/frontend-ui.md §1.2 / contracts/wasm-engine-dm-api.md §W2
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

// Track decrypt_scan invocations so individual tests can configure responses.
const decryptCalls: Array<{ ciphertext: Uint8Array }> = [];
let decryptImpl: (cipher: Uint8Array) => {
  sender_main_account: Uint8Array;
  timestamp_ms: bigint;
  body: Uint8Array;
  signature_valid: boolean;
} | null = () => null;

jest.mock('anarchy-wasm-engine', () => ({
  // initSync omitted: getWasm() skips the WASM-fetch branch in tests.
  dm_decrypt_scan: jest.fn(
    (
      _scanPriv: Uint8Array,
      _spendPub: Uint8Array,
      _eph: Uint8Array,
      _stealth: Uint8Array,
      ciphertext: Uint8Array,
    ) => {
      decryptCalls.push({ ciphertext });
      return decryptImpl(ciphertext);
    },
  ),
}));

jest.mock('@polkadot-api/substrate-bindings', () => ({
  fromBufferToBase58: () => (bytes: Uint8Array) =>
    `5SS58${Array.from(bytes.slice(0, 4)).join('')}`,
  getSs58AddressInfo: (s: string) => ({
    isValid: true,
    publicKey: new Uint8Array(32).fill(s.length % 256),
  }),
}));

import {
  scanDmInbox,
  initSs58Toolkit,
  SCAN_PAGE_SIZE,
  type ScanContext,
  type DmDispatchWithCiphertext,
} from '../scanner';
import type { AccountId } from '../types';

function makeDispatch(seed: number): DmDispatchWithCiphertext {
  return {
    recipientStealth: `5Stealth${seed}` as AccountId,
    ephemeralPubkey: new Uint8Array(32).fill(seed),
    content: {
      root: new Uint8Array(32).fill(seed ^ 0x55),
      k: 3,
      n: 5,
      ciphertextLen: 1024n,
    },
    ciphertext: new Uint8Array(1024).fill(seed ^ 0xab),
  };
}

interface FakeApiOptions {
  bestHead: bigint;
  pages: Array<Array<[bigint, DmDispatchWithCiphertext[]]>>;
}

function makeFakeApi(opts: FakeApiOptions): {
  api: unknown;
  rangeCalls: Array<[bigint, bigint]>;
} {
  const rangeCalls: Array<[bigint, bigint]> = [];
  let pageIdx = 0;
  const api = {
    apis: {
      DmScanApi: {
        dispatches_range: jest.fn(async (from: bigint, to: bigint) => {
          rangeCalls.push([from, to]);
          const result = opts.pages[pageIdx] ?? [];
          pageIdx += 1;
          return result;
        }),
      },
    },
    query: {
      System: { Number: { getValue: jest.fn(async () => opts.bestHead) } },
    },
  };
  return { api, rangeCalls };
}

beforeAll(async () => {
  await initSs58Toolkit();
});

beforeEach(() => {
  decryptCalls.length = 0;
  decryptImpl = () => null;
  jest.clearAllMocks();
});

describe('scanner.scanDmInbox', () => {
  const ctxBase: Omit<ScanContext, 'api'> = {
    ownScanPriv: new Uint8Array(32).fill(0x11),
    ownSpendPub: new Uint8Array(32).fill(0x22),
    ownMainAccount: '5Self' as AccountId,
    lastScannedBlock: 0n,
  };

  it('includes decrypted dispatches with signature_valid=true', async () => {
    decryptImpl = () => ({
      sender_main_account: new Uint8Array(32).fill(0xaa),
      timestamp_ms: 1700000000000n,
      body: new Uint8Array([1, 2, 3]),
      signature_valid: true,
    });
    const { api } = makeFakeApi({
      bestHead: 5n,
      pages: [[[3n, [makeDispatch(7)]]]],
    });
    const result = await scanDmInbox({ ...ctxBase, api, toBlockOverride: 5n });

    expect(result.scannedFromBlock).toBe(1n);
    expect(result.scannedToBlock).toBe(5n);
    expect(result.newMessages).toHaveLength(1);
    expect(result.newMessages[0].body).toEqual(new Uint8Array([1, 2, 3]));
    expect(result.newMessages[0].direction).toBe('incoming');
    expect(result.newMessages[0].signatureValid).toBe(true);
  });

  it('drops dispatches with signature_valid=false (FR-004)', async () => {
    decryptImpl = () => ({
      sender_main_account: new Uint8Array(32).fill(0xaa),
      timestamp_ms: 1700000000000n,
      body: new Uint8Array([9, 9, 9]),
      signature_valid: false,
    });
    const { api } = makeFakeApi({
      bestHead: 2n,
      pages: [[[2n, [makeDispatch(11)]]]],
    });
    const result = await scanDmInbox({ ...ctxBase, api, toBlockOverride: 2n });

    expect(result.newMessages).toHaveLength(0);
    // 確認: decrypt は呼ばれている (フィルタが動いた結果なので)。
    expect(decryptCalls).toHaveLength(1);
  });

  it('skips dispatches that decrypt returns null for', async () => {
    decryptImpl = () => null;
    const { api } = makeFakeApi({
      bestHead: 1n,
      pages: [[[1n, [makeDispatch(3), makeDispatch(4)]]]],
    });
    const result = await scanDmInbox({ ...ctxBase, api, toBlockOverride: 1n });

    expect(result.newMessages).toHaveLength(0);
    expect(decryptCalls).toHaveLength(2);
  });

  it('returns empty range when bestHead < lastScannedBlock+1', async () => {
    const { api } = makeFakeApi({ bestHead: 5n, pages: [] });
    const result = await scanDmInbox({
      ...ctxBase,
      api,
      lastScannedBlock: 10n,
      toBlockOverride: 5n,
    });
    expect(result.scannedFromBlock).toBe(11n);
    expect(result.scannedToBlock).toBe(10n);
    expect(result.newMessages).toHaveLength(0);
  });

  it('paginates dispatches_range by SCAN_PAGE_SIZE (1024 blocks)', async () => {
    decryptImpl = () => null;
    const head = SCAN_PAGE_SIZE * 2n + 100n; // 2148
    const { api, rangeCalls } = makeFakeApi({
      bestHead: head,
      pages: [[], [], []],
    });
    await scanDmInbox({ ...ctxBase, api, toBlockOverride: head });

    expect(rangeCalls).toHaveLength(3);
    // scanner converts bigints to numbers for PAPI compatibility (metadata v16
    // dispatches_range signature expects u32, not bigint).
    expect(rangeCalls[0]).toEqual([1, Number(SCAN_PAGE_SIZE)]);
    expect(rangeCalls[1]).toEqual([Number(SCAN_PAGE_SIZE + 1n), Number(SCAN_PAGE_SIZE * 2n)]);
    expect(rangeCalls[2]).toEqual([Number(SCAN_PAGE_SIZE * 2n + 1n), Number(head)]);
  });

  it('reads bestHead from query.System.Number when toBlockOverride absent', async () => {
    decryptImpl = () => null;
    const { api } = makeFakeApi({ bestHead: 7n, pages: [[]] });
    const result = await scanDmInbox({ ...ctxBase, api });
    expect(result.scannedToBlock).toBe(7n);
  });
});

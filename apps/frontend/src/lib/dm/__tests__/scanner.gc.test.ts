/**
 * T093 (scanner side): GC 検出時に bodyState='garbage_collected' を立てる。
 *
 * 検証:
 *   - dispatch.ciphertext が undefined かつ storage-node からの再構成にも失敗した場合、
 *     scanner は dispatch を skip ではなく `DmMessageRecord` を生成し、
 *     `bodyState: 'garbage_collected'` で counterparty を 'unknown' プレースホルダ
 *     (= 'gc:<merkle-root-hex>') として返す。
 *   - body は空 Uint8Array (UI 側でプレースホルダに置換)。
 *   - signatureValid は false (envelope を復号できないため)。
 *
 * Contract: spec.md Edge Cases "Message garbage-collected" / FR-009 / FR-018。
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

const decryptCalls: Array<{ ciphertext: Uint8Array }> = [];
let decryptImpl: (cipher: Uint8Array) => any = () => null;

jest.mock('anarchy-wasm-engine', () => ({
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
  type ScanContext,
  type DmDispatchWithCiphertext,
} from '../scanner';
import type { AccountId, DmDispatch } from '../types';

function dispatch(seed: number, withCipher = true): DmDispatchWithCiphertext | DmDispatch {
  const base: DmDispatch = {
    recipientStealth: `5Stealth${seed}` as AccountId,
    ephemeralPubkey: new Uint8Array(32).fill(seed),
    content: {
      root: new Uint8Array(32).fill(seed ^ 0x55),
      k: 3,
      n: 5,
      ciphertextLen: 1024n,
    },
  };
  if (withCipher) {
    return { ...base, ciphertext: new Uint8Array(1024).fill(seed ^ 0xab) };
  }
  return base;
}

function makeApi(bestHead: bigint, page: Array<[bigint, Array<DmDispatch | DmDispatchWithCiphertext>]>): unknown {
  return {
    apis: {
      DmScanApi: {
        dispatches_range: jest.fn(async () => page),
      },
    },
    query: {
      System: { Number: { getValue: jest.fn(async () => bestHead) } },
    },
  };
}

beforeAll(async () => {
  await initSs58Toolkit();
});
beforeEach(() => {
  decryptCalls.length = 0;
  decryptImpl = () => null;
  jest.clearAllMocks();
});

describe('scanner — GC indicator (T093 / FR-018)', () => {
  const ctxBase: Omit<ScanContext, 'api'> = {
    ownScanPriv: new Uint8Array(32).fill(0x11),
    ownSpendPub: new Uint8Array(32).fill(0x22),
    ownMainAccount: '5Self' as AccountId,
    lastScannedBlock: 0n,
  };

  it('emits a placeholder DmMessageRecord with bodyState=garbage_collected when ciphertext is unavailable', async () => {
    const api = makeApi(2n, [[2n, [dispatch(7, /* withCipher */ false)]]]);
    const result = await scanDmInbox({ ...ctxBase, api, toBlockOverride: 2n });

    expect(result.newMessages).toHaveLength(1);
    const m = result.newMessages[0];
    expect(m.bodyState).toBe('garbage_collected');
    expect(m.body.length).toBe(0);
    expect(m.signatureValid).toBe(false);
    // counterparty placeholder: gc:<merkle hex prefix>
    expect(m.counterparty.startsWith('gc:')).toBe(true);
  });

  it('emits no placeholder when ciphertext is present and decrypt returns a real message', async () => {
    decryptImpl = () => ({
      sender_main_account: new Uint8Array(32).fill(0xaa),
      timestamp_ms: 1700000000000n,
      body: new Uint8Array([1, 2]),
      signature_valid: true,
    });
    const api = makeApi(3n, [[3n, [dispatch(11, true)]]]);
    const result = await scanDmInbox({ ...ctxBase, api, toBlockOverride: 3n });

    expect(result.newMessages).toHaveLength(1);
    expect(result.newMessages[0].bodyState).toBe('plaintext');
    expect(result.newMessages[0].counterparty.startsWith('gc:')).toBe(false);
  });
});

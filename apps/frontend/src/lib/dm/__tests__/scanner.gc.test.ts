/**
 * T093 (scanner side, revised 2026-05-03): ciphertext 不取得時は silent skip。
 *
 * 旧仕様: dispatch.ciphertext を再構成できないとき "gc:<root>" placeholder を
 * 全 dispatch について inbox に追加していた。これだと **他人宛** の dispatch も
 * placeholder 化されて inbox を汚染し、後続の delivered receipt 送信が
 * SS58 checksum エラーで死ぬバグの温床になっていた。
 *
 * 新仕様: ciphertext を取れない時点で「自分宛か」を判定する手段がないため、
 * scanner は silent skip する。GC visibility (= 自分宛だったが GC 済み) を
 * 表示したい場合は、wasm に軽量な stealth-match check を追加してから
 * 自分宛確認後にだけ placeholder を出す設計に変更すること (TODO)。
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

describe('scanner — silent skip when ciphertext unavailable (revised T093)', () => {
  const ctxBase: Omit<ScanContext, 'api'> = {
    ownScanPriv: new Uint8Array(32).fill(0x11),
    ownSpendPub: new Uint8Array(32).fill(0x22),
    ownMainAccount: '5Self' as AccountId,
    lastScannedBlock: 0n,
  };

  it('silently skips dispatch when ciphertext cannot be reconstructed (no inbox pollution from non-targeted traffic)', async () => {
    const api = makeApi(2n, [[2n, [dispatch(7, /* withCipher */ false)]]]);
    const result = await scanDmInbox({ ...ctxBase, api, toBlockOverride: 2n });

    expect(result.newMessages).toHaveLength(0);
    expect(decryptCalls).toHaveLength(0);
  });

  it('emits a real plaintext bubble when ciphertext is present and decrypt returns a real message', async () => {
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

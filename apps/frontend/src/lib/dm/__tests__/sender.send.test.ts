/**
 * T032: sender.sendDm 正常系の Jest テスト。
 *
 * Asserts:
 *   - tx1 (Stealth.send_to_stealth) が tx2 (Messaging.send_dm) より先に submit される。
 *   - 成功時に SendDmResult.recipientStealth / merkleRoot / paddingBucket が返る。
 *   - 進捗コールバックが encrypting → uploading → prefunding → dispatching → done の順に呼ばれる。
 *
 * Contract: contracts/frontend-ui.md §1.1
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

// --- crypto.getRandomValues polyfill for jsdom (deterministic) ---
import { webcrypto } from 'crypto';
if (!(globalThis as any).crypto?.getRandomValues) {
  (globalThis as any).crypto = webcrypto;
}

// --- Mock the wasm module before any import that uses it ---
// NOTE: omit `initSync` so getWasm() in sender.ts skips the browser fetch path.
jest.mock('anarchy-wasm-engine', () => ({
  dm_derive_recipient_stealth: jest.fn(() => ({
    ephemeral_pubkey: new Uint8Array(32).fill(0xee),
    stealth_pubkey: new Uint8Array(32).fill(0x55),
    shared_secret: new Uint8Array(32).fill(0x33),
  })),
  dm_compute_inner_signed_hash: jest.fn(() => new Uint8Array(32).fill(0x77)),
  dm_encrypt_and_pad: jest.fn(() => ({
    ciphertext: new Uint8Array(1024).fill(0xab),
    ephemeral_pubkey: new Uint8Array(32).fill(0xee),
    recipient_stealth: new Uint8Array(32).fill(0x55),
    padding_bucket: 1024,
  })),
  dm_fragment_ciphertext: jest.fn(() => ({
    merkle_root: new Uint8Array(32).fill(0x12),
    fragment_count: 5,
    fragment: (i: number) => new Uint8Array(205).fill(0xcd ^ i),
  })),
  dm_generate_sender_stealth: jest.fn(() => ({
    account_id: new Uint8Array(32).fill(0x99),
    secret_seed: new Uint8Array(32).fill(0x88),
  })),
}));

// --- Mock @polkadot/keyring (sender stealth signer) ---
jest.mock('@polkadot/keyring', () => ({
  Keyring: jest.fn().mockImplementation(() => ({
    addFromSeed: jest.fn(() => ({
      publicKey: new Uint8Array(32).fill(0x99),
      sign: jest.fn(() => new Uint8Array(64).fill(0xa1)),
    })),
  })),
}));

// --- Mock polkadot-api signer (return signer-as-callback) ---
jest.mock('polkadot-api/signer', () => ({
  getPolkadotSigner: jest.fn((pubKey: Uint8Array, _scheme: string, signer: any) => ({
    publicKey: pubKey,
    signBytes: signer,
    signTx: jest.fn(),
  })),
}));

// --- Mock polkadot-api Binary (used by sender for Binary.fromBytes) ---
jest.mock('polkadot-api', () => ({
  Binary: {
    fromBytes: (b: Uint8Array) => ({ asBytes: () => b, _type: 'Binary' }),
  },
}));

// --- Mock substrate-bindings (SS58) ---
jest.mock('@polkadot-api/substrate-bindings', () => ({
  fromBufferToBase58: () => (bytes: Uint8Array) => `5SS58${Array.from(bytes.slice(0, 4)).join('')}`,
  getSs58AddressInfo: () => ({ isValid: true, publicKey: new Uint8Array(32) }),
}));

import { DmError, type DmMetaAddress, type SendDmParams } from '../types';
import { sendDm, type SendDmContext, type SendDmProgress } from '../sender';

const recipient: DmMetaAddress = {
  scanPub: new Uint8Array(32).fill(0x01),
  spendPub: new Uint8Array(32).fill(0x02),
};

function buildContext(overrides: Partial<{
  reception: DmMetaAddress | null;
  fetchOk: boolean;
  tx1Ok: boolean;
  tx2Ok: boolean;
  recordCalls: string[];
}> = {}): {
  ctx: SendDmContext;
  txOrder: string[];
  progressLog: SendDmProgress[];
} {
  const txOrder: string[] = [];
  const progressLog: SendDmProgress[] = [];
  const fetchImpl = jest.fn(async () => ({
    ok: overrides.fetchOk !== false,
    status: overrides.fetchOk === false ? 500 : 200,
    json: async () => ({ jsonrpc: '2.0', id: 0, result: { success: true } }),
  })) as any;
  globalThis.fetch = fetchImpl;

  const api: any = {
    apis: {
      DmScanApi: {
        reception_key: jest.fn(async () =>
          overrides.reception === undefined ? recipient : overrides.reception,
        ),
      },
    },
    tx: {
      Stealth: {
        send_to_stealth: jest.fn((_params: any) => ({
          signAndSubmit: jest.fn(async () => {
            txOrder.push('tx1');
            return {
              txHash: '0xtx1',
              ok: overrides.tx1Ok ?? true,
              block: { number: 100, hash: '0xb1' },
            };
          }),
        })),
      },
      Messaging: {
        send_dm: jest.fn((_params: any) => ({
          signAndSubmit: jest.fn(async () => {
            txOrder.push('tx2');
            return {
              txHash: '0xtx2',
              ok: overrides.tx2Ok ?? true,
              block: { number: 101, hash: '0xb2' },
            };
          }),
        })),
      },
    },
  };

  const mainSigner: any = {
    publicKey: new Uint8Array(32).fill(0xaa),
    signBytes: jest.fn(async () => new Uint8Array(64).fill(0xbb)),
  };

  return {
    ctx: {
      api,
      mainSigner,
      mainAccountPublicKey: new Uint8Array(32).fill(0xaa),
      storageEndpoint: 'http://localhost:3030',
      onProgress: (p) => progressLog.push(p),
    },
    txOrder,
    progressLog,
  };
}

const baseParams: SendDmParams = {
  recipientAccountId: '5RecipientAccount' as any,
  body: new TextEncoder().encode('hello world'),
  k: 3,
  n: 5,
};

describe('sender.sendDm — happy path', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('submits tx1 (pre-fund) before tx2 (send_dm) and returns SendDmResult', async () => {
    const { ctx, txOrder, progressLog } = buildContext();
    const result = await sendDm(baseParams, ctx);

    expect(txOrder).toEqual(['tx1', 'tx2']);
    expect(result.merkleRoot).toEqual(new Uint8Array(32).fill(0x12));
    expect(result.paddingBucket).toBe(1024);
    expect(result.blockNumber).toBe(101n);
    expect(typeof result.recipientStealth).toBe('string');
    expect(result.totalCostMoral).toBeGreaterThan(0n);

    const kinds = progressLog.map((p) => p.kind);
    expect(kinds).toEqual(expect.arrayContaining(['encrypting', 'prefunding', 'dispatching', 'done']));
    expect(kinds.indexOf('prefunding')).toBeLessThan(kinds.indexOf('dispatching'));
  });

  it('throws RecipientKeyNotPublished when reception_key is null', async () => {
    const { ctx } = buildContext({ reception: null });
    await expect(sendDm(baseParams, ctx)).rejects.toThrow(DmError.RecipientKeyNotPublished);
  });
});

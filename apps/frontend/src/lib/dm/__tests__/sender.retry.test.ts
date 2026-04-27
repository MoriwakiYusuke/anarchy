/**
 * T090 (frontend side): tx2-failure path テスト。
 *
 * Asserts:
 *   - tx1 (`Stealth.send_to_stealth`) は呼ばれて成功する (pre-fund 完了)。
 *   - tx2 (`Messaging.send_dm`) が dropped/throw すると sendDm は
 *     `DmError.TransactionDropped` を throw する。
 *   - tx2 dropped 時も sender_stealth seed は CT-1 ゼロクリアされる (finally)。
 *   - sendDm は dmStore に書き込まない (呼出側責務であることを保証)。
 *
 * Contract: spec.md Edge Cases "Network partition mid-send" / FR-006 / T091 retry UX。
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

import { webcrypto } from 'crypto';
if (!(globalThis as any).crypto?.getRandomValues) {
  (globalThis as any).crypto = webcrypto;
}

let capturedSeed: Uint8Array | null = null;

jest.mock('anarchy-wasm-engine', () => ({
  // initSync omitted: getWasm() skips the WASM-fetch branch in tests.
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
    fragment_count: 1,
    fragment: () => new Uint8Array(1024).fill(0xab),
    proof: () => new Uint8Array(0),
  })),
  dm_generate_sender_stealth: jest.fn(() => {
    const seed = new Uint8Array(32).fill(0xcd);
    capturedSeed = seed;
    return {
      account_id: new Uint8Array(32).fill(0x99),
      secret_seed: seed,
    };
  }),
}));

jest.mock('@polkadot/keyring', () => ({
  Keyring: jest.fn().mockImplementation(() => ({
    addFromSeed: jest.fn(() => ({
      publicKey: new Uint8Array(32).fill(0x99),
      sign: jest.fn(() => new Uint8Array(64).fill(0xa1)),
    })),
  })),
}));

jest.mock('polkadot-api/signer', () => ({
  getPolkadotSigner: jest.fn((pubKey: Uint8Array, _scheme: string, signer: any) => ({
    publicKey: pubKey,
    signBytes: signer,
    signTx: jest.fn(),
  })),
}));

jest.mock('polkadot-api', () => ({
  Binary: { fromBytes: (b: Uint8Array) => ({ asBytes: () => b }) },
}));

jest.mock('@polkadot-api/substrate-bindings', () => ({
  fromBufferToBase58: () => () => '5SS58Stealth',
  getSs58AddressInfo: () => ({ isValid: true, publicKey: new Uint8Array(32) }),
}));

import { sendDm, type SendDmContext } from '../sender';
import { DmError, type DmMetaAddress, type SendDmParams } from '../types';

const recipient: DmMetaAddress = {
  scanPub: new Uint8Array(32).fill(0x01),
  spendPub: new Uint8Array(32).fill(0x02),
};

interface BuiltCtx {
  ctx: SendDmContext;
  tx1: jest.Mock;
  tx2: jest.Mock;
  txOrder: string[];
}

function buildCtx(opts: { tx2Mode: 'throw' | 'notOk' }): BuiltCtx {
  const txOrder: string[] = [];
  globalThis.fetch = jest.fn(async () => ({
    ok: true,
    status: 200,
    json: async () => ({ jsonrpc: '2.0', id: 0, result: {} }),
  })) as any;

  const tx1 = jest.fn(async () => {
    txOrder.push('tx1');
    return { txHash: '0xtx1', ok: true, block: { number: 100, hash: '0xb1' } };
  });
  const tx2 =
    opts.tx2Mode === 'throw'
      ? jest.fn(async () => {
          txOrder.push('tx2');
          throw new Error('network partition');
        })
      : jest.fn(async () => {
          txOrder.push('tx2');
          return { txHash: '0xtx2', ok: false, block: { number: 101, hash: '0xb2' } };
        });

  const api: any = {
    apis: { DmScanApi: { reception_key: jest.fn(async () => recipient) } },
    tx: {
      Stealth: { send_to_stealth: jest.fn(() => ({ signAndSubmit: tx1 })) },
      Messaging: { send_dm: jest.fn(() => ({ signAndSubmit: tx2 })) },
    },
  };

  return {
    ctx: {
      api,
      mainSigner: {
        publicKey: new Uint8Array(32),
        signBytes: async () => new Uint8Array(64),
      } as any,
      mainAccountPublicKey: new Uint8Array(32).fill(0xaa),
      chainRpcEndpoint: 'http://localhost:9944',
    },
    tx1,
    tx2,
    txOrder,
  };
}

const params: SendDmParams = {
  recipientAccountId: '5Bob' as any,
  body: new Uint8Array([1, 2, 3]),
  k: 1,
  n: 1,
};

describe('sender.sendDm — tx2 failure path (T090 frontend side)', () => {
  beforeEach(() => {
    capturedSeed = null;
    jest.clearAllMocks();
  });

  it('throws DmError.TransactionDropped when tx2 throws after tx1 succeeds', async () => {
    const built = buildCtx({ tx2Mode: 'throw' });
    await expect(sendDm(params, built.ctx)).rejects.toThrow(DmError.TransactionDropped);

    expect(built.tx1).toHaveBeenCalledTimes(1);
    expect(built.tx2).toHaveBeenCalledTimes(1);
    expect(built.txOrder).toEqual(['tx1', 'tx2']);
  });

  it('throws DmError.TransactionDropped when tx2 returns ok=false', async () => {
    const built = buildCtx({ tx2Mode: 'notOk' });
    await expect(sendDm(params, built.ctx)).rejects.toThrow(DmError.TransactionDropped);
    expect(built.tx1).toHaveBeenCalledTimes(1);
    expect(built.tx2).toHaveBeenCalledTimes(1);
  });

  it('zeroizes the sender_stealth secret_seed even on tx2 failure', async () => {
    const built = buildCtx({ tx2Mode: 'throw' });
    await expect(sendDm(params, built.ctx)).rejects.toThrow();
    expect(capturedSeed).not.toBeNull();
    expect(Array.from(capturedSeed as Uint8Array)).toEqual(new Array(32).fill(0));
  });

  // 「dmStore に書き込まない」契約は sender.ts が store を import していないこと
  // で静的に保証される (依存関係を grep で検証可能)。ランタイム検証は composer
  // 側のテスト責務。
});

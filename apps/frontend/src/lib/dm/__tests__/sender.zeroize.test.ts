/**
 * T089: sender.sendDm が `dm_generate_sender_stealth.secret_seed` を tx2 完了後に
 * ゼロクリアすることを確認する。CT-1 / Constitution II 補完テスト。
 *
 * 戦略: WASM mock の `dm_generate_sender_stealth` が返す `secret_seed` Uint8Array
 * への参照をテスト側に保持し、`sendDm` 完了後にその内容が `[0; 32]` であることを
 * assert する (sender が呼出側 buffer を直接 fill(0) する前提)。
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
    // 認識用 0xCD パターンで返却。sender 側の Uint8Array コピーが zeroize 対象。
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
import type { DmMetaAddress, SendDmParams } from '../types';

const recipient: DmMetaAddress = {
  scanPub: new Uint8Array(32).fill(0x01),
  spendPub: new Uint8Array(32).fill(0x02),
};

function ctxOk(): SendDmContext {
  globalThis.fetch = jest.fn(async () => ({
    ok: true,
    status: 200,
    json: async () => ({ jsonrpc: '2.0', id: 0, result: {} }),
  })) as any;
  const api: any = {
    apis: { DmScanApi: { reception_key: jest.fn(async () => recipient) } },
    tx: {
      Stealth: {
        send_to_stealth: jest.fn(() => ({
          signAndSubmit: jest.fn(async () => ({
            txHash: '0x1', ok: true, block: { number: 1, hash: '0x1' },
          })),
        })),
      },
      Messaging: {
        send_dm: jest.fn(() => ({
          signAndSubmit: jest.fn(async () => ({
            txHash: '0x2', ok: true, block: { number: 2, hash: '0x2' },
          })),
        })),
      },
    },
  };
  return {
    api,
    mainSigner: { publicKey: new Uint8Array(32), signBytes: async () => new Uint8Array(64) } as any,
    mainAccountPublicKey: new Uint8Array(32).fill(0xaa),
    chainRpcEndpoint: 'http://localhost:9944',
  };
}

const params: SendDmParams = {
  recipientAccountId: '5Bob' as any,
  body: new Uint8Array([1, 2, 3]),
  k: 1,
  n: 1,
};

describe('sender.sendDm — secret_seed zeroize (T089 / CT-1)', () => {
  beforeEach(() => {
    capturedSeed = null;
    jest.clearAllMocks();
  });

  it('zeroizes the sender_stealth secret_seed buffer after tx2 finalize', async () => {
    await sendDm(params, ctxOk());
    expect(capturedSeed).not.toBeNull();
    const seed = capturedSeed as Uint8Array;
    expect(seed.length).toBe(32);
    // sender は wasm getter から渡された buffer を copy せず直接 fill(0) する設計
    // (CT-1 / FR-021)。mock が返した同一バッファ参照が全 0 になっていることを確認。
    expect(Array.from(seed)).toEqual(new Array(32).fill(0));
  });

  it('zeroizes secret_seed even if tx2 throws (finally block)', async () => {
    const ctx = ctxOk();
    // tx2 で例外を起こす差し替え。
    (ctx.api as any).tx.Messaging.send_dm = jest.fn(() => ({
      signAndSubmit: jest.fn(async () => {
        throw new Error('boom');
      }),
    }));

    await expect(sendDm(params, ctx)).rejects.toThrow();
    expect(capturedSeed).not.toBeNull();
    expect(Array.from(capturedSeed as Uint8Array)).toEqual(new Array(32).fill(0));
  });
});

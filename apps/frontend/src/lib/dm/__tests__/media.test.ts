/**
 * media.ts (uploadDmMedia / fetchDmMedia) の happy-path 単体テスト。
 *
 * 暗号 / フラグメント分割 / RPC は jest.mock で差し替える。検証点:
 *   - upload は dm_media_encrypt → dm_fragment_ciphertext → storage_uploadFragment の順に
 *     呼び、返り値の DmMediaRef に hex な root/key と mime/size を含む。
 *   - fetch は storage_getFragment で n 個取り → concat → dm_media_decrypt → Blob を返す。
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

import { webcrypto } from 'crypto';
if (!(globalThis as any).crypto?.getRandomValues) {
  (globalThis as any).crypto = webcrypto;
}

const fakeCiphertext = new Uint8Array(1024).fill(0xab);
const fakeMerkleRoot = new Uint8Array(32).fill(0x12);

jest.mock('anarchy-wasm-engine', () => ({
  dm_media_encrypt: jest.fn((_key: Uint8Array, _pt: Uint8Array) => fakeCiphertext),
  dm_media_decrypt: jest.fn(() => new Uint8Array([1, 2, 3, 4, 5])),
  dm_fragment_ciphertext: jest.fn(() => ({
    merkle_root: fakeMerkleRoot,
    fragment_count: 5,
    fragment: (i: number) => fakeCiphertext.subarray(i * 205, (i + 1) * 205),
    proof: (_i: number) => new Uint8Array(0),
  })),
}));

jest.mock('@/lib/mediaProcessor', () => ({
  processMediaFile: jest.fn(async (file: File) => ({ file, width: 100, height: 100 })),
}));

import { fetchDmMedia, uploadDmMedia } from '../media';
import type { DmMediaRef } from '../types';

function makeFile(name: string, type: string, bytes: Uint8Array): File {
  // jsdom の File は `arrayBuffer()` を持たないので明示的に被せる。
  const ab = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(ab).set(bytes);
  const file = new File([ab], name, { type });
  if (typeof (file as any).arrayBuffer !== 'function') {
    Object.defineProperty(file, 'arrayBuffer', {
      value: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
    });
  }
  return file;
}

describe('uploadDmMedia', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('encrypts → fragments → uploads, returning a DmMediaRef', async () => {
    const fetchMock = jest.fn(async (_url: string, init?: RequestInit) => {
      const body = JSON.parse((init?.body as string) ?? '{}');
      return {
        ok: true,
        async json() {
          return { jsonrpc: '2.0', id: body.id, result: { success: true } };
        },
      } as Response;
    });
    (global as any).fetch = fetchMock;

    const file = makeFile('a.png', 'image/png', new Uint8Array([0xff, 0xd8, 0xff]));
    const ref = await uploadDmMedia(file, {
      chainRpcEndpoint: 'http://test:9944',
      onProgress: jest.fn(),
    });

    expect(ref.root).toMatch(/^[0-9a-f]{64}$/);
    expect(ref.key).toMatch(/^[0-9a-f]{64}$/);
    expect(ref.mime).toBe('image/png');
    expect(ref.k).toBe(3);
    expect(ref.n).toBe(5);
    expect(ref.ciphertextLen).toBe(fakeCiphertext.length);
    // 5 fragments を chain-node に POST する。
    expect(fetchMock).toHaveBeenCalled();
    const methods = fetchMock.mock.calls.map((c) => {
      const init = c[1] as RequestInit;
      const body = JSON.parse((init.body as string) ?? '{}');
      return body.method;
    });
    expect(new Set(methods)).toEqual(new Set(['storage_uploadFragment']));
  });
});

describe('fetchDmMedia', () => {
  it('fetches fragments + decrypts → Blob', async () => {
    const chunkSize = 205;
    const fetchMock = jest.fn(async (_url: string, init?: RequestInit) => {
      const body = JSON.parse((init?.body as string) ?? '{}');
      const idx = body.params[0].index;
      const part = fakeCiphertext.subarray(idx * chunkSize, (idx + 1) * chunkSize);
      const b64 = Buffer.from(part).toString('base64');
      return {
        ok: true,
        async json() {
          return { jsonrpc: '2.0', id: body.id, result: { data: b64 } };
        },
      } as Response;
    });
    (global as any).fetch = fetchMock;

    const ref: DmMediaRef = {
      root: '12'.repeat(32),
      key: 'aa'.repeat(32),
      mime: 'image/png',
      size: 5,
      k: 3,
      n: 5,
      ciphertextLen: fakeCiphertext.length,
    };
    const blob = await fetchDmMedia(ref, {
      chainRpcEndpoint: 'http://test:9944',
      fetchImpl: fetchMock as any,
    });
    expect(blob.type).toBe('image/png');
    expect(blob.size).toBe(5); // mocked decrypt の固定出力
  });
});

/**
 * sessionStore: ログイン状態 (account + seed) の IndexedDB 永続化。
 * リロードで復帰、切断でクリア。IDB 不可環境では保存せず null を返す。
 */
import 'fake-indexeddb/auto';
import { IDBFactory } from 'fake-indexeddb';

async function load() {
  let mod: typeof import('@/lib/account/sessionStore');
  jest.isolateModules(() => {
    mod = require('@/lib/account/sessionStore');
  });
  return mod!;
}

beforeEach(() => {
  (globalThis as { indexedDB: IDBFactory }).indexedDB = new IDBFactory();
});

const SESSION = { account: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY', seed: 'word1 word2 word3' };

describe('sessionStore', () => {
  test('save した session をモジュール再ロード (= リロード) 後に load できる', async () => {
    const a = await load();
    await a.saveSession(SESSION);
    const b = await load();
    expect(await b.loadSession()).toEqual(SESSION);
  });

  test('未保存なら null', async () => {
    const s = await load();
    expect(await s.loadSession()).toBeNull();
  });

  test('clear 後は null (切断)', async () => {
    const a = await load();
    await a.saveSession(SESSION);
    await a.clearSession();
    const b = await load();
    expect(await b.loadSession()).toBeNull();
  });

  test('IDB が使えない環境では例外を投げず null', async () => {
    (globalThis as { indexedDB?: IDBFactory }).indexedDB = undefined;
    const s = await load();
    await expect(s.saveSession(SESSION)).resolves.toBeUndefined();
    expect(await s.loadSession()).toBeNull();
    await expect(s.clearSession()).resolves.toBeUndefined();
  });
});

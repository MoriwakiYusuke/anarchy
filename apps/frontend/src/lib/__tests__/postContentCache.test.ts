/**
 * postContentCache: Merkle root → 復元済み投稿バイト列 の永続キャッシュ。
 *
 * fake-indexeddb で IDB を差し替え、各テストごとに新しい DB 実装を注入して
 * モジュールスコープのメモリ層と IDB 層の両方をリセットする。
 */
import 'fake-indexeddb/auto';
import { IDBFactory } from 'fake-indexeddb';

const ROOT_A = new Uint8Array(32).fill(0xaa);
const ROOT_B = new Uint8Array(32).fill(0xbb);

// isolateModules でモジュールスコープの Map を毎回作り直す
async function loadCache() {
  let mod: typeof import('@/lib/postContentCache');
  jest.isolateModules(() => {
    mod = require('@/lib/postContentCache');
  });
  return mod!;
}

beforeEach(() => {
  // 毎テストで空の IDB に差し替える
  (globalThis as { indexedDB: IDBFactory }).indexedDB = new IDBFactory();
});

describe('postContentCache', () => {
  test('put した内容を同じ root で get できる', async () => {
    const cache = await loadCache();
    const data = new Uint8Array([1, 2, 3, 4]);
    await cache.putCachedContent(ROOT_A, data);
    const got = await cache.getCachedContent(ROOT_A);
    expect(got).toEqual(data);
  });

  test('未登録の root は null を返す', async () => {
    const cache = await loadCache();
    await cache.putCachedContent(ROOT_A, new Uint8Array([1]));
    expect(await cache.getCachedContent(ROOT_B)).toBeNull();
  });

  test('IDB に永続化される (モジュールを再ロードしても get できる)', async () => {
    const first = await loadCache();
    const data = new Uint8Array([9, 8, 7]);
    await first.putCachedContent(ROOT_A, data);

    // メモリ層を捨てて IDB だけが残った状態を再現
    const second = await loadCache();
    expect(await second.getCachedContent(ROOT_A)).toEqual(data);
  });

  test('IDB が使えない環境ではミス扱いで例外を投げない', async () => {
    (globalThis as { indexedDB?: IDBFactory }).indexedDB = undefined;
    const cache = await loadCache();
    await expect(cache.putCachedContent(ROOT_A, new Uint8Array([1]))).resolves.toBeUndefined();
    // メモリ層には残る
    expect(await cache.getCachedContent(ROOT_A)).toEqual(new Uint8Array([1]));
    // 別ロード (メモリ層なし) では null
    const fresh = await loadCache();
    expect(await fresh.getCachedContent(ROOT_A)).toBeNull();
  });
});

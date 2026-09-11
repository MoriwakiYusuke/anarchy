import { test, expect } from './fixtures/chain';

/**
 * 投稿本文の IndexedDB キャッシュ (lib/postContentCache) E2E。
 *
 * 1. Alice が投稿 → タイムラインに本文が出る (= storage から復元してキャッシュに put された)
 * 2. `storage_getFragment` を全部 abort した状態でリロード
 * 3. それでも同じ本文が描画される = storage RPC も Worker 復元も経由せず IDB から出ている証拠
 *
 * 他テストが残した投稿は別 context で作られており IDB に無いので、リロード後は
 * それらだけ error 表示になる (このテストでは見ない)。
 */
test.describe('Post content cache', () => {
  test('cached post renders after reload with storage RPC blocked', async ({ page, connectDevAccount }) => {
    await connectDevAccount('Alice');

    const textarea = page.getByPlaceholder("What's happening?");
    await expect(textarea).toBeVisible({ timeout: 30_000 });

    const body = `e2e content-cache ${Date.now()}`;
    await textarea.fill(body);
    const submitBtn = page.getByRole('button', { name: /^Post$/, exact: true });
    await expect(submitBtn).toBeEnabled({ timeout: 30_000 });
    await submitBtn.click();
    await expect(page.getByText(/Posted!\s*\(Block #\d+\)/)).toBeVisible({ timeout: 120_000 });

    // タイムラインに復元済み本文が出るまで待つ (storage 経由の初回ロード)
    await expect(page.getByText(body, { exact: true })).toBeVisible({ timeout: 120_000 });

    // put は fire-and-forget なので IDB に載ったことを直接確認する
    await expect
      .poll(
        () =>
          page.evaluate(
            () =>
              new Promise<number>((resolve) => {
                const req = indexedDB.open('anarchy-post-content');
                req.onerror = () => resolve(-1);
                req.onsuccess = () => {
                  const db = req.result;
                  if (!db.objectStoreNames.contains('contents')) return resolve(0);
                  const count = db.transaction('contents').objectStore('contents').count();
                  count.onsuccess = () => resolve(count.result);
                  count.onerror = () => resolve(-1);
                };
              }),
          ),
        { timeout: 30_000 },
      )
      .toBeGreaterThan(0);

    // 以降 storage_getFragment は全部落とす
    let blocked = 0;
    await page.route('**/*', (route) => {
      if (route.request().postData()?.includes('storage_getFragment')) {
        blocked += 1;
        return route.abort();
      }
      return route.continue();
    });

    await page.reload();
    await expect(page.getByText('Connected', { exact: false })).toBeVisible({ timeout: 60_000 });

    // storage を叩けない状態でも、さっきの投稿はキャッシュから描画される
    await expect(page.getByText(body, { exact: true })).toBeVisible({ timeout: 60_000 });
    // 本文が出た時点で、この投稿については getFragment を投げていない (投げていれば abort で error 表示になる)
    // blocked は他の (未キャッシュ) 投稿分で >0 になり得るので数は assert しない
    test.info().annotations.push({ type: 'blocked storage_getFragment', description: String(blocked) });
  });
});

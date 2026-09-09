import path from 'path';
import { test, expect } from './fixtures/chain';

/**
 * 画像付き Post E2E (refactor/full-code-review のメディアアップロード修正の回帰防止)。
 *
 * 検証対象:
 *   - MediaUpload コンポーネント (file input → addFiles → preview) が機能する
 *   - PostForm が画像を postCodec.encodePostContent で本文に同梱し、
 *     useStorage (KZG hybrid_split → chain-node `storage_uploadFragment`) 経由で
 *     fragment を実際に保存する — 存在しない RPC を叩いて偽の成功を報告していた
 *     旧バグの回帰防止 (useMediaUpload.ts も同じ chainRpc.ts 共通層に統一された)
 *   - reload 後に Timeline → PostItem がチェーン+storage から本文を復元し、
 *     画像が data URL として描画される (= fragment が本当に保存されていた証拠)
 *
 * 前提: testnet + storage nodes 起動済み (fixtures/chain.ts 参照)。
 */
test.describe('Post with image attachment', () => {
  test('Alice posts text + PNG, media is stored and rendered after reload', async ({
    page,
    connectDevAccount,
  }) => {
    test.setTimeout(360_000);

    await connectDevAccount('Alice');

    // PostForm に本文を入れる
    const marker = `e2e media-post ${Date.now()}`;
    const textarea = page.getByPlaceholder("What's happening?");
    await expect(textarea).toBeVisible({ timeout: 30_000 });
    await textarea.fill(marker);

    // MediaUpload の hidden file input に PNG を流し込む (PostForm 配下に限定)
    const postForm = page.locator('form').filter({ has: textarea });
    await postForm
      .locator('input[type="file"]')
      .setInputFiles(path.join(__dirname, 'fixtures', 'assets', 'tiny.png'));

    // addFiles 完了 = preview が出る (blob: URL)。
    await expect(postForm.locator('img').first()).toBeVisible({ timeout: 30_000 });

    // Submit。usePostCost の Loading 完了を待ってから押す。
    const submitBtn = page.getByRole('button', { name: /^Post$/, exact: true });
    await expect(submitBtn).toBeEnabled({ timeout: 30_000 });
    await submitBtn.click();

    // KZG split + storage_uploadFragment ×5 + create_post finalize で 120s 超は普通。
    await expect(page.getByText(/Posted!\s*\(Block #\d+\)/).first()).toBeVisible({
      timeout: 180_000,
    });

    // Reload → Timeline がチェーン + storage fragment から本文を再構築する。
    await page.reload();
    await connectDevAccount('Alice');

    const post = page
      .locator('article', { hasText: marker })
      .or(page.locator('[data-testid="post-item"]', { hasText: marker }))
      .first();
    await expect(post).toBeVisible({ timeout: 60_000 });

    // PostItem は media を data URL (mediaToDataUrl) で描画する。
    // fragment 取得 (storage_getFragment ×k) + decode に時間がかかるので長めに待つ。
    await expect(post.locator('img[src^="data:"]').first()).toBeVisible({ timeout: 120_000 });
  });
});

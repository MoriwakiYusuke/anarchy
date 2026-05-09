import { test, expect } from './fixtures/chain';

/**
 * Like/Bad count display regression.
 *
 * Originally caught the bug where `Timeline.tsx` called
 * `unsafeApi.query.Reaction.ReactionStatsStorage.getValue(post.id)` with
 * `post.id: number`, but the storage is keyed by `u64` (= bigint in PAPI).
 * Passing a plain number silently returned null (no decoder match) so the
 * count never showed up after a page reload, even though the chain had it.
 * Fix: cast to `BigInt(post.id)` at the call site.
 *
 * Flow:
 *   1. Connect Alice.
 *   2. Post a text marker.
 *   3. After "Posted!" finalizes, reload + reconnect (session-only keys).
 *   4. Locate the new post; initial Like count should render empty (0).
 *   5. Click Like → PoW mining + extrinsic.
 *   6. Optimistic count → "1".
 *   7. Reload + reconnect again.
 *   8. Count must STILL be "1" — proves the chain → Timeline → ReactionButton
 *      readback path actually works.
 */
test.describe('Reaction display', () => {
  test('Alice posts, then likes, count appears', async ({ page, connectDevAccount }) => {
    test.setTimeout(360_000);

    await connectDevAccount('Alice');

    // Compose a unique post so we can find it in the timeline.
    const marker = `like-debug-${Date.now()}`;
    await page.locator('textarea').fill(marker);
    await page.locator('button:has-text("Post")').click();

    await expect(page.getByText(/Posted!\s*\(Block #\d+\)/).first()).toBeVisible({
      timeout: 120_000,
    });

    // Reload — Timeline re-fetches posts + ReactionStatsStorage.
    // Then reconnect Alice (session is non-persistent — keys are
    // session-memory only per CLAUDE.md security principles).
    // connectDevAccount already waits for the wallet to read Connected.
    await page.reload();
    await connectDevAccount('Alice');

    // Find the post we just made.
    const post = page.locator(`text=${marker}`).first();
    await expect(post).toBeVisible({ timeout: 60_000 });

    // Initial Like count: locate the Like button + log its text.
    const likeBtn = page
      .locator('article', { hasText: marker })
      .or(page.locator('[data-testid="post-item"]', { hasText: marker }))
      .locator('button[aria-label="Like"]')
      .first();
    await expect(likeBtn).toBeVisible({ timeout: 30_000 });

    const before = await likeBtn.textContent();
    console.log(`[reaction-debug] Like button before: ${JSON.stringify(before)}`);
    await page.screenshot({ path: 'test-results/reaction-debug-before.png', fullPage: false });

    // Click Like → triggers PoW mining + extrinsic.
    await likeBtn.click();

    // Wait for the count to bump. Allow up to 3 minutes for PoW + finalize.
    await expect
      .poll(async () => (await likeBtn.textContent()) || '', { timeout: 180_000 })
      .toMatch(/[1-9]/);

    const after = await likeBtn.textContent();
    console.log(`[reaction-debug] Like button after: ${JSON.stringify(after)}`);
    await page.screenshot({ path: 'test-results/reaction-debug-after.png', fullPage: false });

    // Reload one more time to confirm the count is *persisted* on chain
    // (not just an optimistic local update).
    await page.reload();
    await connectDevAccount('Alice');
    await expect(page.locator(`text=${marker}`).first()).toBeVisible({ timeout: 60_000 });

    const persistedBtn = page
      .locator('article', { hasText: marker })
      .or(page.locator('[data-testid="post-item"]', { hasText: marker }))
      .locator('button[aria-label="Like"]')
      .first();
    const persisted = await persistedBtn.textContent();
    console.log(`[reaction-debug] Like button after reload: ${JSON.stringify(persisted)}`);
    await page.screenshot({ path: 'test-results/reaction-debug-after-reload.png', fullPage: false });

    expect(persisted).toMatch(/[1-9]/);
  });
});

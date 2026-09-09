import { test, expect } from './fixtures/chain';

/**
 * Foreground PoW pause/resume E2E (refactor/full-code-review の CRITICAL fix 回帰防止)。
 *
 * 検証対象 — lib/pow/pausableMiner.ts + useReactionMining.ts:
 *   - タブが hidden の間は PoW worker を動かさない (CLAUDE.md Security Principle #4)。
 *     旧実装は「pause 表示だけ」で worker が全速で回り続けていた。
 *   - autoResume: false (reaction) では PowPausedError → hook が status 'paused' を
 *     surface し、UI (ReactionButton) に paused 表示 + Resume ボタンが出る。
 *   - resume() で保存済み nonce から再マイニングし、solution → submit まで完走する。
 *
 * タイミング戦略: reaction の difficulty は dev chain で 16 bits (runtime
 * BaseDifficulty)。pure-JS blake2b では平均 1 秒未満で解けてしまうため、
 * 「mining 中に hide する」レースは flaky になる。代わりに **Like クリック前に
 * hidden にしておく** — pausableMiner は開始時点で hidden なら worker を一切
 * spawn せず即 PowPausedError で 'paused' に遷移する (同一コードパス) ので、
 * 決定的に paused 状態を観測できる。
 */

/** document.hidden / visibilityState を上書きして visibilitychange を発火する。
 *  pausableMiner の defaultVisibility は document.hidden を見る。 */
async function setTabHidden(page: import('@playwright/test').Page, hidden: boolean): Promise<void> {
  await page.evaluate((isHidden) => {
    Object.defineProperty(document, 'hidden', { value: isHidden, configurable: true });
    Object.defineProperty(document, 'visibilityState', {
      value: isHidden ? 'hidden' : 'visible',
      configurable: true,
    });
    document.dispatchEvent(new Event('visibilitychange'));
  }, hidden);
}

test.describe('PoW visibility pause/resume', () => {
  test('reaction mining pauses while tab is hidden and resumes via Resume button', async ({
    page,
    connectDevAccount,
  }) => {
    test.setTimeout(360_000);

    await connectDevAccount('Alice');

    // 対象 post を作る (reaction-display.spec.ts と同じパターン)。
    const marker = `pow-pause-${Date.now()}`;
    await page.locator('textarea').fill(marker);
    await page.locator('button:has-text("Post")').click();
    await expect(page.getByText(/Posted!\s*\(Block #\d+\)/).first()).toBeVisible({
      timeout: 120_000,
    });

    // Reload して Timeline から拾い直す (session-only key なので再接続)。
    await page.reload();
    await connectDevAccount('Alice');

    const post = page
      .locator('article', { hasText: marker })
      .or(page.locator('[data-testid="post-item"]', { hasText: marker }))
      .first();
    await expect(post).toBeVisible({ timeout: 60_000 });

    const likeBtn = post.locator('button[aria-label="Like"]').first();
    await expect(likeBtn).toBeEnabled({ timeout: 30_000 });

    // --- 1. タブを hidden にしてから Like → minePow は worker を spawn せず paused ---
    await setTabHidden(page, true);
    await likeBtn.click();

    // useReactionMining が 'paused' を surface → ReactionButton の paused 表示。
    const paused = post.locator('[data-testid="reaction-mining-paused"]');
    await expect(paused).toBeVisible({ timeout: 30_000 });

    // paused 中は submit に進まない (submitting 表示が出ない) ことも確認。
    await expect(post.locator('[data-testid="reaction-submitting"]')).toHaveCount(0);

    // --- 2. タブを visible に戻して Resume → 保存済み nonce から再開し完走 ---
    await setTabHidden(page, false);
    await post.locator('[data-testid="reaction-mining-resume"]').click();

    // paused 表示が消える = status が 'mining' に遷移した。
    await expect(paused).toHaveCount(0, { timeout: 30_000 });

    // mining 再開 → solution → submitReaction finalize → Like count が 1 以上になる。
    // PoW blocktime 30s + finalize で 3 分まで許容 (reaction-display.spec.ts と同じ)。
    await expect
      .poll(async () => (await likeBtn.textContent()) || '', { timeout: 180_000 })
      .toMatch(/[1-9]/);
  });
});

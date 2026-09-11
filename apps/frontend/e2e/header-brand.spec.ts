import { test, expect } from '@playwright/test';

/**
 * Header ブランド要素の回帰防止。
 *
 * 触った path:
 *   - apps/frontend/src/app/page.tsx — h1 内に inline SVG (A マーク) + "narchy" の構成
 *     "A" 文字がワードマークとロゴで二重表示される regression を阻止する
 *   - apps/frontend/src/app/page.module.css — .title を inline-flex で 1 行レイアウト
 *   - apps/frontend/src/app/icon.svg / favicon.ico — Next.js metadata から自動 inject
 *
 * チェーン接続は不要 (ヘッダーは static markup)。
 */
test.describe('Header brand', () => {
  test('renders Anarchy heading + inline A logo as one wordmark', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('domcontentloaded');

    // 見出しのアクセシブル名は DOM テキストそのものが "Anarchy" (aria-label 頼みではない)。
    // 検索エンジンやコピー&ペーストも "narchy" ではなく "Anarchy" を得る。
    const heading = page.getByRole('heading', { level: 1, name: 'Anarchy' });
    await expect(heading).toBeVisible();
    await expect(heading).toHaveText('Anarchy');

    // ロゴ SVG は h1 内に inline 配置された装飾 (aria-hidden)。先頭の "A" は
    // 視覚的には SVG、DOM 上は visually-hidden な span が担う (二重 A 表示の回帰を阻止)。
    await expect(heading.locator('svg[aria-hidden="true"]')).toBeVisible();
    await expect(heading).not.toContainText('AAnarchy');
  });

  test('viewport allows pinch zoom (no maximum-scale / user-scalable=no)', async ({ page }) => {
    await page.goto('/');
    const content = await page.locator('meta[name="viewport"]').getAttribute('content');
    expect(content).toContain('width=device-width');
    expect(content).not.toMatch(/maximum-scale/);
    expect(content).not.toMatch(/user-scalable=no/);
  });

  test('favicon and icon.svg are served by Next.js metadata', async ({ page, baseURL }) => {
    const favicon = await page.request.get(`${baseURL}/favicon.ico`);
    expect(favicon.status()).toBe(200);
    expect(favicon.headers()['content-type'] || '').toMatch(/image\/(x-icon|vnd\.microsoft\.icon)/);

    const iconSvg = await page.request.get(`${baseURL}/icon.svg`);
    expect(iconSvg.status()).toBe(200);
    const body = await iconSvg.text();
    expect(body).toContain('<svg');
    expect(body).toContain('Anarchy');
  });
});

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

    // h1 全体は aria-label="Anarchy" で 1 つのワードマークとしてアクセシブル
    const heading = page.getByRole('heading', { level: 1, name: 'Anarchy' });
    await expect(heading).toBeVisible();

    // ロゴ SVG は h1 内に inline 配置されている (二重 A 表示の回帰を阻止)
    await expect(heading.locator('svg[aria-label="A"]')).toBeVisible();

    // 末尾のテキスト部分は "narchy" のみ (ワードマークが "A Anarchy" に戻っていない)
    await expect(heading).toContainText('narchy');
    await expect(heading).not.toContainText('AAnarchy');
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

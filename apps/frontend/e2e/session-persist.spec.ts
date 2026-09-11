import { test, expect } from './fixtures/chain';

/**
 * ログイン状態の永続化 (lib/account/sessionStore) E2E。
 *
 * 1. Alice で接続 → reload → 何も操作せず Alice のまま接続済み (IndexedDB から復帰)
 * 2. Disconnect → reload → 未接続 (session がクリアされている)
 */
const ALICE = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY';

test.describe('Session persistence', () => {
  test('connection survives reload and is cleared by Disconnect', async ({ page, connectDevAccount }) => {
    await connectDevAccount('Alice');
    const wallet = page.locator('aside');
    await expect(wallet.locator('code')).toHaveText(ALICE);

    // reload → 自動復帰 (select も Connect ボタンも出ない)
    await page.reload();
    await expect(wallet.getByText('Connected', { exact: false }).first()).toBeVisible({ timeout: 30_000 });
    await expect(wallet.locator('code')).toHaveText(ALICE);
    await expect(wallet.locator('select')).toHaveCount(0);

    // 投稿フォームが使える = signer も復帰している
    await expect(page.getByPlaceholder("What's happening?")).toBeVisible({ timeout: 30_000 });

    // Disconnect → session クリア → reload しても未接続
    await wallet.getByRole('button', { name: 'Disconnect' }).click();
    await expect(wallet.locator('select')).toBeVisible({ timeout: 10_000 });
    await page.reload();
    await expect(page.getByText('Connected', { exact: false }).first()).toBeVisible({ timeout: 60_000 });
    await expect(wallet.locator('select')).toBeVisible({ timeout: 10_000 });
    await expect(wallet.locator('code')).toHaveCount(0);
  });
});

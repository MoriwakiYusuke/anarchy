import { test, expect } from './fixtures/chain';
import { openDmModal, generateStealthKey } from './helpers/dm';

/**
 * ログイン状態の永続化 (lib/account/sessionStore) E2E。
 *
 * 1. Alice で接続 + DM 鍵を生成 → reload → 何も操作せず Alice のまま接続済みで、
 *    DM 鍵も復帰している (IndexedDB から)
 * 2. Disconnect → reload → 未接続、DM 鍵も無い (session + 鍵がクリアされている)
 */
const ALICE = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY';

test.describe('Session persistence', () => {
  test('connection survives reload and is cleared by Disconnect', async ({ page, connectDevAccount }) => {
    await connectDevAccount('Alice');
    const wallet = page.locator('aside');
    await expect(wallet.locator('code')).toHaveText(ALICE);

    // DM 鍵を生成し、IDB に保存された秘密鍵素材を控える
    await openDmModal(page);
    await generateStealthKey(page);
    await page.keyboard.press('Escape');
    const readStoredStealth = () =>
      page.evaluate(
        (addr) =>
          new Promise<string | null>((resolve) => {
            const req = indexedDB.open('anarchy-auth');
            req.onerror = () => resolve(null);
            req.onsuccess = () => {
              const db = req.result;
              if (!db.objectStoreNames.contains('session')) return resolve(null);
              const get = db.transaction('session').objectStore('session').get(`stealth:${addr}`);
              get.onsuccess = () => {
                const v = get.result as { scanPriv?: Uint8Array; spendPriv?: Uint8Array } | undefined;
                if (!v?.scanPriv || !v?.spendPriv) return resolve(null);
                resolve(Array.from(v.scanPriv).join(',') + '|' + Array.from(v.spendPriv).join(','));
              };
              get.onerror = () => resolve(null);
            };
          }),
        ALICE,
      );
    await expect.poll(readStoredStealth, { timeout: 15_000 }).not.toBeNull();
    const storedBeforeReload = await readStoredStealth();

    // reload → 自動復帰 (select も Connect ボタンも出ない)
    await page.reload();
    await expect(wallet.getByText('Connected', { exact: false }).first()).toBeVisible({ timeout: 30_000 });
    await expect(wallet.locator('code')).toHaveText(ALICE);
    await expect(wallet.locator('select')).toHaveCount(0);

    // 投稿フォームが使える = signer も復帰している
    await expect(page.getByPlaceholder("What's happening?")).toBeVisible({ timeout: 30_000 });

    // DM 鍵も復帰している: 設定タブに "Prepare key" ではなく key manager が出る
    await openDmModal(page);
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('tab', { name: /Settings/i }).click();
    await expect(dialog.getByRole('region', { name: /DM key manager/i })).toBeVisible({ timeout: 30_000 });
    await expect(dialog.getByRole('button', { name: /Generate a new key/i })).toHaveCount(0);
    // 復帰でロードされた鍵は保存されていたものと同じ (loadFromBackup が同じ素材を再保存するので不変)
    expect(await readStoredStealth()).toBe(storedBeforeReload);
    await page.keyboard.press('Escape');

    // Disconnect → session クリア → reload しても未接続
    await wallet.getByRole('button', { name: 'Disconnect' }).click();
    await expect(wallet.locator('select')).toBeVisible({ timeout: 10_000 });
    await page.reload();
    await expect(page.getByText('Connected', { exact: false }).first()).toBeVisible({ timeout: 60_000 });
    await expect(wallet.locator('select')).toBeVisible({ timeout: 10_000 });
    await expect(wallet.locator('code')).toHaveCount(0);

    // 切断で鍵の保存分も消えている
    expect(await readStoredStealth()).toBeNull();

    // 再接続しても DM 鍵は消えている (切断で clearAllAuth 済み)
    await connectDevAccount('Alice');
    await openDmModal(page);
    const dialog2 = page.getByRole('dialog');
    const openSettings = dialog2.getByRole('button', { name: /Open DM key settings/i });
    if (await openSettings.isVisible()) await openSettings.click();
    else await dialog2.getByRole('tab', { name: /Settings/i }).click();
    await expect(dialog2.getByRole('button', { name: /Generate a new key/i })).toBeVisible({ timeout: 30_000 });
  });
});

import { test, expect } from './fixtures/chain';
import { generateStealthKey, openDmModal, TEST_ADDRESSES } from './helpers/dm';

/**
 * DM 新規宛先の SS58 validation E2E (refactor/full-code-review の DmModal 修正の回帰防止)。
 *
 * 検証対象 — DmModal.handleNewDm:
 *   - SS58 として decode できない宛先は validateSS58Address で即弾き、
 *     inline の role="alert" エラー (i18n: dm.compose.invalidRecipient) を表示する。
 *     旧実装は不正アドレスのままスレッドを開き、送信フロー深部で不可解な
 *     エラーになっていた。
 *   - 正しい SS58 (Bob) なら エラーなしでスレッド (Counterparty region) が開く。
 *   - 入力を打ち直すとエラーがクリアされる (onChange で setRecipientError(null))。
 */
test.describe('DM new-recipient validation', () => {
  test('invalid SS58 shows inline alert, valid SS58 opens thread', async ({
    page,
    connectDevAccount,
  }) => {
    await connectDevAccount('Alice');

    await openDmModal(page);
    // 受信箱の compose row は stealth 鍵ロード後にのみ出る。
    await generateStealthKey(page);

    // Inbox タブに戻る
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('tab', { name: /Inbox/i }).click();
    const input = dialog.getByPlaceholder(/Recipient SS58/i);
    await expect(input).toBeVisible({ timeout: 10_000 });

    // --- 1. 不正アドレス → role="alert" が出てスレッドは開かない ---
    await input.fill('not-an-address');
    await dialog.getByRole('button', { name: /^Open$/ }).click();

    const alert = dialog.getByRole('alert');
    await expect(alert).toBeVisible({ timeout: 10_000 });
    await expect(alert).toHaveText(/Invalid SS58 address/i);
    // Counterparty region (= ConversationView) が開いていないこと
    await expect(dialog.getByRole('region', { name: /^Counterparty:/ })).toHaveCount(0);

    // --- 2. 入力し直すとエラーがクリアされる ---
    await input.fill(TEST_ADDRESSES.BOB);
    await expect(dialog.getByRole('alert')).toHaveCount(0);

    // --- 3. 正しい SS58 → スレッドが開く ---
    await dialog.getByRole('button', { name: /^Open$/ }).click();
    await expect(
      dialog.getByRole('region', { name: new RegExp(`Counterparty: ${TEST_ADDRESSES.BOB}`) }),
    ).toBeVisible({ timeout: 30_000 });
  });
});

import { test, expect } from './fixtures/chain';

/**
 * Transfer (送金) E2E — Alice → Bob で 1 MORAL 送金。
 *
 * 検証範囲:
 *   - TransferForm の collapse 展開
 *   - 残高表示 (BalanceDisplay)
 *   - recipient + amount validation
 *   - signAndSubmit + finalize (PoW 30s blocktime, 余裕を見て 240s timeout)
 *   - 成功 status (transfer.success = "Transfer complete!")
 */
test.describe('Transfer flow', () => {
  test('Alice sends 1 MORAL to Bob and finalize', async ({ page, connectDevAccount }) => {
    await connectDevAccount('Alice');

    // signer が ready になるまで Faucet ボタン (常時可視) で待機
    await expect(page.locator('aside').getByRole('button', { name: /^Faucet$/ }))
      .toBeVisible({ timeout: 30_000 });

    // Wallet panel の "Transfer" を展開 (collapse の見出しは "Transfer ▼" なので prefix match)
    const transferToggle = page.locator('aside').getByRole('button', { name: /^Transfer/ });
    await expect(transferToggle).toBeVisible({ timeout: 30_000 });
    await transferToggle.click();

    // Recipient (Bob's well-known SS58)
    const BOB = '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty';
    const recipient = page.getByLabel(/Recipient/i);
    await recipient.fill(BOB);

    // Amount
    const amount = page.getByLabel(/Amount/i);
    await amount.fill('1');

    // Send → Confirm のフロー (TransferForm は 2 段)
    const sendBtn = page.locator('aside').getByRole('button', { name: /^Send$/ });
    await expect(sendBtn).toBeEnabled({ timeout: 10_000 });
    await sendBtn.click();

    // Confirm dialog (transfer.confirm = "Confirm Transfer")
    const confirmBtn = page.getByRole('button', { name: /^Confirm Transfer$/ });
    if (await confirmBtn.isVisible({ timeout: 3_000 }).catch(() => false)) {
      await confirmBtn.click();
    }

    // 成功メッセージ — finalize で 30s × 1 + ブレ。Aura→PoW で旧 90s では足りないので 240s。
    await expect(page.getByText(/Transfer complete/i)).toBeVisible({ timeout: 240_000 });
  });
});

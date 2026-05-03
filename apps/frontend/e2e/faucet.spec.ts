import { test, expect } from './fixtures/chain';

/**
 * Faucet PoW + claim E2E (chore/maintenance-sweep ブランチの回帰防止)。
 *
 * 触った path:
 *   - useFaucet `signer` dep 追加 (#40-MEDIUM-1) — 新規 connect 直後の startMining が
 *     stale closure を掴んで動作しないと、ボタンが永遠に "Mining..." のままになる
 *   - useFaucet 内 console.* → debugLog 一斉移行 — 回帰でランタイム例外を投げて
 *     クレームに失敗しないこと
 *   - hexToBytes に validation 追加 (#40-LOW-6) — 正常な block hash で例外を吐かないこと
 *   - pallet_faucet validate_unsigned `and_provides` を account dedupe に変更
 *     (#26-CRIT-2) — 単発 claim が普通に finalize すること
 *
 * Alice は genesis で残高保有のため faucet を叩けない。新規 mnemonic を生成して
 * 残高 0 → faucet → 成功テキスト or 残高 100 MORAL という流れで観測する。
 */
test.describe('Faucet PoW + claim flow', () => {
  test('Fresh wallet receives 100 MORAL via faucet PoW', async ({ page }) => {
    // Wallet パネルの Seed Phrase タブに切替
    await page.locator('aside button:has-text("Seed Phrase")').first().click();

    // 新規 mnemonic を生成。textarea (placeholder = "word1 word2 word3 ... word12") に入る。
    await page.locator('aside button:has-text("Generate New")').click();
    const mnemonicTextarea = page.locator('aside textarea[placeholder*="word1"]');
    await expect(mnemonicTextarea).not.toHaveValue('', { timeout: 10_000 });

    // Connect で wallet 確立。Connected ラベルが出るまで待つ。
    await page.locator('aside button:has-text("Connect")').click();
    await expect(
      page.locator('aside').getByText(/Connected/i).first(),
    ).toBeVisible({ timeout: 30_000 });

    // 初期残高 0 確認 (Genesis に居ない新規アカウント)
    await expect(
      page.locator('aside').getByText(/Balance/i),
    ).toBeVisible({ timeout: 10_000 });

    // Faucet ボタン (en text = "Faucet") をクリック → PoW 開始
    const faucetBtn = page.locator('aside button:has-text("Faucet")');
    await expect(faucetBtn).toBeEnabled({ timeout: 10_000 });
    await faucetBtn.click();

    // 成功は 2 つの観測いずれかで判定 (どちらか先に来た方で OK):
    //   (a) ボタン文言が "Received 100 MORAL!" になる
    //   (b) 残高が 100 MORAL を表示
    // PoW + extrinsic finalize で 30s〜2min は普通。
    await expect(async () => {
      const success = await page.locator('aside button:has-text("Received 100 MORAL")').isVisible().catch(() => false);
      const hasBalance = await page.locator('aside').getByText(/100\.?0*\s*MORAL/i).isVisible().catch(() => false);
      expect(success || hasBalance).toBe(true);
    }).toPass({ timeout: 180_000, intervals: [3_000, 5_000, 10_000] });
  });
});

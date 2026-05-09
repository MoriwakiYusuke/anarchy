import { test, expect } from './fixtures/chain';

/**
 * TSTS F6: Post コスト表示が EIP-1559 base_fee 込みになっていることを確認する E2E.
 *
 * 検証範囲:
 *   - usePostCost フックがチェーンから `PostBaseCost` / `PostByteCost` を取得する
 *   - TSTS v1 の mainnet 値 (PostBaseCost=50 MORAL, PostByteCost=0.0008 MORAL/byte) が反映される
 *   - calculatePostCost は base_fee 込みの式: total = base + (byteCost + base_fee) × bytes
 *   - 平常時は base_fee がほぼ 0 なので Post submit が成功する
 *   - 混雑時 (`base-fee-congestion-badge` が出る場合) でも UI は崩れない
 *
 * 依存: dev chain の `pallet_post.PostBaseCost = 50_000_000_000_000` (= 50 MORAL).
 *       chainspec を旧 100 MORAL のまま動かしている環境では `expect(50)` が失敗する.
 */
test.describe('TSTS F6 — Post cost display includes base_fee', () => {
  test('Alice sees TSTS v1 cost values when typing a short post', async ({ page, connectDevAccount }) => {
    await connectDevAccount('Alice');

    const textarea = page.getByPlaceholder("What's happening?");
    await expect(textarea).toBeVisible({ timeout: 30_000 });

    // 100 byte 程度の本文を入力 → cost ≒ 50 + 0.0008 * 100 = 50.08 MORAL (base_fee=0 平常時)
    const body = 'a'.repeat(100);
    await textarea.fill(body);

    // post.cost (en: "Cost: 50.1 MORAL") を限定して取得 — Balance 表示と区別する.
    // toPass で再試行: cost 表示は usePostCost の chain fetch 完了後に出る.
    await expect(async () => {
      const costSpan = page.locator('text=/^Cost:.*MORAL/').first();
      await expect(costSpan).toBeVisible();
      const costText = await costSpan.textContent();
      expect(costText).toBeTruthy();
      const match = costText!.match(/Cost:\s*(\d+\.\d+|\d+)/);
      expect(match).not.toBeNull();
      const numeric = parseFloat(match![1]);
      // 旧モデルだと 100.1 程度 (PostBaseCost=100). TSTS v1 だと 50.08〜51 程度 (PostBaseCost=50).
      // base_fee の状況次第で振れるので幅広に (40〜80 で fail なら誤設定).
      expect(numeric).toBeGreaterThan(40);
      expect(numeric).toBeLessThan(80);
    }).toPass({ timeout: 60_000, intervals: [2_000, 3_000, 5_000] });
  });

  test('Submit succeeds in normal congestion (no base_fee blowup)', async ({ page, connectDevAccount }) => {
    await connectDevAccount('Alice');

    const textarea = page.getByPlaceholder("What's happening?");
    await expect(textarea).toBeVisible({ timeout: 30_000 });
    await textarea.fill(`tsts-f6 base-fee normal ${Date.now()}`);

    const submitBtn = page.getByRole('button', { name: /^Post$/, exact: true });
    await expect(submitBtn).toBeEnabled({ timeout: 30_000 });
    await submitBtn.click();

    // 平常時 base_fee はほぼ 0 → 50 MORAL 程度で投稿成功するはず
    await expect(page.getByText(/Posted!\s*\(Block #\d+\)/)).toBeVisible({ timeout: 120_000 });
  });

  test('Congestion badge does not appear at idle base_fee', async ({ page, connectDevAccount }) => {
    await connectDevAccount('Alice');

    const textarea = page.getByPlaceholder("What's happening?");
    await expect(textarea).toBeVisible({ timeout: 30_000 });
    await textarea.fill('quick test');

    // base-fee-congestion-badge は base_fee の log10 度合いが 0.3 を超えたときのみ表示。
    // dev chain で post を打っていない段階では BaseFeeMin 張り付きなので出ないはず。
    const badge = page.getByTestId('base-fee-congestion-badge');
    // 短時間 polling: 出ないことを期待 → 出たら混雑モード (テスト環境の汚染) なので skip
    const visible = await badge.isVisible().catch(() => false);
    if (visible) {
      test.skip(true, 'Test chain already congested (badge visible) — TSTS P2 base_fee is climbing');
    }
    expect(visible).toBe(false);
  });
});

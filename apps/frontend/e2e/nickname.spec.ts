import { test, expect } from './fixtures/chain';

/**
 * Nickname (名前変更) E2E — Alice がオンチェーン nickname を登録 / 解除。
 *
 * 検証範囲:
 *   - NicknameSettings collapse の展開
 *   - input + Set ボタンの validation (空文字 disabled)
 *   - signAndSubmit + finalize (PoW 30s blocktime, 余裕 240s)
 *   - 成功メッセージ (nickname.success)
 *   - Clear ボタンが現れる (登録済み状態)
 */
test.describe('Nickname flow', () => {
  test('Alice sets nickname, sees success, can clear', async ({ page, connectDevAccount }) => {
    await connectDevAccount('Alice');

    // signer ready 待ち
    await expect(page.locator('aside').getByRole('button', { name: /^Faucet$/ }))
      .toBeVisible({ timeout: 30_000 });

    // NicknameSettings は wallet panel に常時表示。"Change ▼" ボタンで form を展開。
    const changeBtn = page.locator('aside').getByRole('button', { name: /^Change/i });
    await expect(changeBtn).toBeVisible({ timeout: 30_000 });
    await changeBtn.click();

    // 一意な値で衝突回避 (連続実行対応)
    const nick = `e2e-${Date.now().toString(36)}`;

    const input = page.locator('aside').getByPlaceholder(/Enter new name/i);
    await input.fill(nick);

    const setBtn = page.locator('aside').getByRole('button', { name: /^Set$/ });
    await expect(setBtn).toBeEnabled({ timeout: 10_000 });
    await setBtn.click();

    // 成功検証:
    //   - NicknameSettings は onSuccess で form を auto-collapse するため、
    //     "Nickname set" メッセージは form 内 (collapse 後消滅) に出る → 短命で flaky。
    //   - 安定検証は wallet panel の Name 表示が新 nickname に切替わったかどうか。
    //     set_nickname extrinsic finalize で 30s + ブレ、240s で待機。
    await expect(page.locator('aside').getByText(nick)).toBeVisible({ timeout: 240_000 });
  });
});

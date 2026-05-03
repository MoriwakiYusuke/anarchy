import { test, expect } from './fixtures/chain';
import {
  generateStealthKey,
  openDmModal,
} from './helpers/dm';

/**
 * チェーンに古い DM 鍵が残った状態で新しいセッションを開いたときの UX 回帰テスト。
 *
 * 背景: 過去セッションで stealth 鍵を生成 + publish → ブラウザを閉じる
 * (session-only なので local 鍵は消える) → 別端末で復元せずに新規生成すると、
 * チェーン上の old meta と local meta が食い違う。修正前の DmKeyManager は
 * 古いセッションの "Status: Published" を信じて Publish ボタンを disabled に
 * してしまい、ユーザーが republish できないまま silent decrypt failure に陥る。
 *
 * このテストは:
 *   - 鍵生成直後 (まだ publish していない) に "Publish" or "Republish" ボタンが
 *     enabled になっていること
 *   - publish 後に "Status: Published" になることを確認する。
 *
 * テスト名 (date-suffix) を変えて再実行してもチェーン状態が残る。spec 内では
 * "publish → 状態 = Published" の遷移を見るため idempotent。
 */
test.describe('DM key republish UX (stale-key recovery)', () => {
  test('Generated key shows actionable Publish/Republish button (not stuck disabled)', async ({
    page,
    connectDevAccount,
  }) => {
    await connectDevAccount('Alice');
    await openDmModal(page);
    await generateStealthKey(page);

    const dialog = page.getByRole('dialog');
    const publishBtn = dialog.getByRole('button', { name: /^(Publish|Republish)$/ });

    // どちらの状態でも Publish/Republish ボタンが押せるようになっていること。
    await expect(publishBtn).toBeEnabled({ timeout: 30_000 });

    await publishBtn.click();
    await expect(dialog.getByText(/^Status:\s*Published$/)).toBeVisible({ timeout: 60_000 });
  });
});

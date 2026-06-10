import { test, expect } from './fixtures/chain';
import {
  generateStealthKey,
  openDmModal,
  openThread,
  publishOrRepublishDmKey,
  sendDmText,
  expectOutgoingBubble,
  expectIncomingBubble,
  TEST_ADDRESSES,
} from './helpers/dm';

/**
 * シングルユーザー DM フル E2E。
 *
 * 検証範囲:
 *   1. wallet 接続 + chain ready
 *   2. DM modal 起動 + stealth 鍵生成 + chain への publish
 *   3. 自分宛 DM の送信 (encrypt → submit_dm → finalize)
 *   4. Scanner が own dispatch を pickup → incoming バブル表示
 *
 * Self-DM は multi-user の縮約版で、scanner / decrypt パスが正しく動くことを
 * (鍵管理を 1 セッション内で完結させたまま) 検証できる最小フロー。
 *
 * ⚠️  multi-user (Alice ↔ Bob) は別 spec で 2 contexts を使ってテスト予定だが、
 * chain client per-tab + AccountContext per-tab の制約上、現時点 (2026-05-03)
 * では本 self-DM が "send → on-chain → scan → decrypt → display" 全パスを
 * 通すゴールドパスとして十分機能する。
 */
test.describe('DM single-user flow', () => {
  test('Alice publishes key, sends self-DM, scanner picks it up', async ({
    page,
    connectDevAccount,
  }) => {
    // publish (~120s) + send 2-extrinsic finalize (~360s) + scanner pickup (~180s)
    // の合計がデフォルト 360s を超えるため個別に拡張 (PoW 32s/block 実測ベース)。
    test.setTimeout(900_000);

    await connectDevAccount('Alice');

    await openDmModal(page);
    await generateStealthKey(page);
    await publishOrRepublishDmKey(page);

    await openThread(page, TEST_ADDRESSES.ALICE);

    const body = `e2e self-dm ${Date.now()}`;
    await sendDmText(page, body);

    await expectOutgoingBubble(page, body);
    await expectIncomingBubble(page, body);

    // Inbox sidebar に未読カウントが付与される。
    const thread = page.getByRole('dialog').getByRole('button', {
      name: new RegExp(TEST_ADDRESSES.ALICE),
    });
    await expect(thread.getByLabel(/Unread count/i)).toHaveText('1', { timeout: 60_000 });
  });
});

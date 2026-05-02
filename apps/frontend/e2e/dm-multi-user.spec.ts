import { test as base, expect, type BrowserContext, type Page } from '@playwright/test';
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
 * マルチユーザー DM E2E (Alice ↔ Bob)。
 *
 * 設計:
 *  - Playwright `browser.newContext()` で 2 つの独立した browser context を立てる。
 *    各 context は独自の localStorage / IndexedDB / smoldot light client を持つので、
 *    実機 2 台でテストするのと等価な分離が得られる。
 *  - workers: 1 のため 1 ファイル内では順次実行。smoldot per-tab singleton 制約を尊重。
 *
 * フロー:
 *  1. Alice context: 接続 + stealth 鍵生成 + Publish_or_Republish (chain に current key を確定)
 *  2. Bob   context: 接続 + stealth 鍵生成 (送信者 stealth として使う) + Alice 宛 DM 送信
 *  3. Alice context に戻る: scanner が Bob の dispatch を pickup → incoming bubble 表示 + unread badge
 *
 * 既知の制約:
 *  - smoldot は context 起動から finalized block を fetch するまで時間がかかる
 *    (testnet 起動直後の cold start で 30-60s)。fixture が baseURL に
 *    "Connected" を待つので、起動済みの testnet がある前提。
 */

interface MultiUserFixtures {
  noConsoleErrors: void;
}

const test = base.extend<MultiUserFixtures>({
  // dm-self.spec で使う chain.ts fixture と同じ哲学だが、ここでは
  // 複数 context を扱うため fixture を spec 内で再定義する。
  noConsoleErrors: [
    async ({}, use) => {
      // 各 page から拾った error をまとめる。
      const errors: string[] = [];

      const attach = (page: Page, label: string): void => {
        page.on('console', (msg) => {
          if (msg.type() === 'error') errors.push(`[${label}] ${msg.text()}`);
        });
        page.on('pageerror', (err) => {
          errors.push(`[${label}] pageerror: ${err.message}`);
        });
      };
      // attach は context.newPage() の前後で呼ぶ責務をテスト本体に委ねる。
      // attach 関数を渡すために fixture を関数型にしたい所だが、Playwright の
      // fixture API では戻り値で渡せないので、ここでは page.on を expose せず、
      // 代わりに globalThis に attach を生やす + テスト終了時に errors を assert。
      (globalThis as unknown as { __anarchyTestAttach__?: typeof attach }).__anarchyTestAttach__ = attach;
      await use();
      expect(errors, `Unexpected console errors:\n${errors.join('\n')}`).toEqual([]);
    },
    { auto: true },
  ],
});

async function bootstrapPage(context: BrowserContext, label: string): Promise<Page> {
  const page = await context.newPage();
  const attach = (globalThis as unknown as { __anarchyTestAttach__?: (p: Page, label: string) => void })
    .__anarchyTestAttach__;
  attach?.(page, label);
  await page.goto('/');
  await expect(page.getByText('Connected', { exact: false })).toBeVisible({ timeout: 60_000 });
  return page;
}

async function connectAndOpenDm(page: Page, account: 'Alice' | 'Bob'): Promise<void> {
  await page.locator('aside select').selectOption(`//${account}`);
  await page.locator('aside button:has-text("Connect")').click();
  await expect(page.locator('aside').getByText('Connected', { exact: false }).first()).toBeVisible({
    timeout: 30_000,
  });
  await openDmModal(page);
}

test.describe('DM multi-user (Alice ↔ Bob, 2 contexts)', () => {
  test('Bob sends DM to Alice — Alice scanner picks it up and renders incoming bubble', async ({
    browser,
  }) => {
    // ---- Alice setup ----
    const aliceContext = await browser.newContext();
    const alicePage = await bootstrapPage(aliceContext, 'alice');
    await connectAndOpenDm(alicePage, 'Alice');
    await generateStealthKey(alicePage);
    await publishOrRepublishDmKey(alicePage);
    // Alice はモーダルを開いたまま放置 (scanner ループが動き続ける)。

    // ---- Bob setup ----
    const bobContext = await browser.newContext();
    const bobPage = await bootstrapPage(bobContext, 'bob');
    await connectAndOpenDm(bobPage, 'Bob');
    await generateStealthKey(bobPage);
    // Bob は publish 不要 (送信側だけならチェーンに鍵は要らない)。

    // ---- Bob → Alice 送信 ----
    await openThread(bobPage, TEST_ADDRESSES.ALICE);
    const body = `multi-user e2e ${Date.now()}`;
    await sendDmText(bobPage, body);
    await expectOutgoingBubble(bobPage, body);

    // ---- Alice 側で受信確認 ----
    // 会話リストは Inbox タブでのみ描画されるので Inbox に切替。
    // (publishOrRepublishDmKey は Settings タブで終わっている)
    await alicePage.getByRole('dialog').getByRole('tab', { name: /Inbox/i }).click();
    // Alice の inbox に Bob の thread が追加されるのを待つ。SS58 で識別。
    // smoldot per-tab cold sync + 6s blocktime + scanner 15s interval を考慮して timeout 大きめ。
    await expect(async () => {
      const threads = alicePage
        .getByRole('dialog')
        .getByRole('button', { name: new RegExp(TEST_ADDRESSES.BOB) });
      expect(await threads.count()).toBeGreaterThan(0);
    }).toPass({ timeout: 180_000, intervals: [3_000, 5_000, 10_000] });

    // Bob のスレッドを開いて incoming bubble を確認。
    const bobThread = alicePage
      .getByRole('dialog')
      .getByRole('button', { name: new RegExp(TEST_ADDRESSES.BOB) })
      .first();
    await bobThread.click();
    // multi-user では incoming bubble は 1 件のみ (Alice 側に outgoing は出ない)
    await expectIncomingBubble(alicePage, body, 1);

    await aliceContext.close();
    await bobContext.close();
  });
});

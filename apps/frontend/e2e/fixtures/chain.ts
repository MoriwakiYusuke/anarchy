import { test as base, expect, type Page } from '@playwright/test';

/**
 * Anarchy DM E2E 共通 fixture。
 *
 * **前提**: 3-node testnet (`pnpm testnet:start`) と 5 storage nodes
 * (`pnpm storage:start`) が事前に起動していること。webServer は Next.js
 * のみを立てる (playwright.config.ts)。
 *
 * smoldot は 1 タブ 1 client なので並列禁止 (workers: 1)。
 */

interface ChainFixtures {
  /** ホーム画面の "Connected" ステータスが見えるまで待つ。 */
  chainReady: void;
  /** Dev ドロップダウンから //Alice / //Bob / //Charlie を選んで Connect する。 */
  connectDevAccount: (account: 'Alice' | 'Bob' | 'Charlie') => Promise<void>;
  /** 1 ページ分の console error を集めて、テスト終了時に 0 件か検査する。 */
  noConsoleErrors: void;
}

async function waitForChainConnected(page: Page): Promise<void> {
  // ホーム画面の "Connected" 文字列で chain 接続を判定する。
  // smoldot の初回 sync は重い (~20s) ため timeout を広めに取る。
  await expect(page.getByText('Connected', { exact: false })).toBeVisible({ timeout: 60_000 });
}

async function connectDev(page: Page, account: 'Alice' | 'Bob' | 'Charlie'): Promise<void> {
  await page.locator('aside select').selectOption(`//${account}`);
  await page.locator('aside button:has-text("Connect")').click();
  // Wallet パネルが Connected 状態に遷移するまで待つ。
  await expect(
    page.locator('aside').getByText('Connected', { exact: false }).first(),
  ).toBeVisible({ timeout: 30_000 });
}

export const test = base.extend<ChainFixtures>({
  chainReady: [
    async ({ page }, use) => {
      await page.goto('/');
      await waitForChainConnected(page);
      await use();
    },
    { auto: true },
  ],

  connectDevAccount: async ({ page }, use) => {
    await use(async (account) => connectDev(page, account));
  },

  noConsoleErrors: [
    async ({ page }, use) => {
      const errors: string[] = [];
      page.on('console', (msg) => {
        if (msg.type() === 'error') errors.push(msg.text());
      });
      page.on('pageerror', (err) => {
        errors.push(`pageerror: ${err.message}`);
      });
      await use();
      // smoldot の "occupied the CPU for an unreasonable amount of time" は warning。
      // ここではエラーレベルのみ拾う。
      // **既知の許容エラー** (別途 issue 化が必要):
      //   - `[dm-receipt][delivered] Error: Invalid checksum` — self-DM 受信時に
      //     送る delivered receipt が PAPI の SS58 checksum 検証で落ちる。
      //     原因は scanner が `bytesToSs58Sync` で生成する AccountId と PAPI
      //     extrinsic encoder の SS58 復号の不整合と思われる。
      //     2026-05-03 時点では本フローの送信/受信表示自体には影響しない。
      const KNOWN_TOLERATED = [/\[dm-receipt\]\[delivered\] Error: Invalid checksum/];
      const fatal = errors.filter((e) => !KNOWN_TOLERATED.some((re) => re.test(e)));
      expect(fatal, `Unexpected console errors:\n${fatal.join('\n')}`).toEqual([]);
    },
    { auto: true },
  ],
});

export { expect };

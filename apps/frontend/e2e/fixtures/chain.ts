import { test as base, expect, type Page } from '@playwright/test';

/**
 * Anarchy DM E2E 共通 fixture。
 *
 * **前提**: 3-node testnet (`pnpm testnet:start`) と 5 storage nodes
 * (`pnpm storage:start`) が事前に起動していること。webServer は Next.js
 * のみを立てる (playwright.config.ts)。
 *
 * chain client は per-tab singleton なので並列禁止 (workers: 1)。
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
  // WS provider の初期 handshake + 最初の System.Number クエリで数秒、
  // PoW dev node の起動直後だと余裕をみて 60s 待つ。
  await expect(page.getByText('Connected', { exact: false })).toBeVisible({ timeout: 60_000 });
}

async function connectDev(page: Page, account: 'Alice' | 'Bob' | 'Charlie'): Promise<void> {
  const wallet = page.locator('aside');
  const select = wallet.locator('select');
  // reload 後は IndexedDB の session から自動復帰するので select が出ない。
  // その場合は Connected になるのを待つだけ (復帰中は "Connecting..." が出ている)。
  const restored = await wallet
    .getByText('Connected', { exact: false })
    .first()
    .waitFor({ state: 'visible', timeout: 5_000 })
    .then(() => true)
    .catch(() => false);
  if (!restored) {
    await select.selectOption(`//${account}`);
    await wallet.locator('button:has-text("Connect")').click();
  }
  // Wallet パネルが Connected 状態に遷移するまで待つ。
  await expect(wallet.getByText('Connected', { exact: false }).first()).toBeVisible({
    timeout: 30_000,
  });
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
      // 致命的でない warning (e.g. WS reconnect) はここではスルー。エラーレベルのみ拾う。
      expect(errors, `Unexpected console errors:\n${errors.join('\n')}`).toEqual([]);
    },
    { auto: true },
  ],
});

export { expect };

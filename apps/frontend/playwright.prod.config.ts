import { defineConfig, devices } from '@playwright/test'

/**
 * 本番 (https://anarchy2026.org) に対する疎通 E2E。
 *
 * 通常の playwright.config.ts との違い:
 *   - webServer を立てない (デプロイ済みの本番を叩く)
 *   - baseURL が本番ドメイン
 *   - testDir が e2e-prod (ローカル前提の spec を巻き込まない)
 *
 * 実行:
 *   pnpm exec playwright test -c playwright.prod.config.ts
 *
 * 注意: 本番チェーンに実際に extrinsic を投げる。MORAL を消費し、
 * 投稿はチェーンに残る。
 */
export default defineConfig({
  testDir: './e2e-prod',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: 'list',

  // PoW のブロック間隔は 30s 目標。upload + finalize で数分かかる。
  timeout: 420_000,
  expect: { timeout: 60_000 },

  use: {
    baseURL: 'https://anarchy2026.org',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },

  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
})

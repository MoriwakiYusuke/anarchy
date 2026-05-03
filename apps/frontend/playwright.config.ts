import { defineConfig, devices } from '@playwright/test'

const PORT = Number(process.env.E2E_PORT ?? 3000)

// Anarchy frontend (Next.js 14 + smoldot + anarchy-wasm-engine) 用の Playwright 設定。
// dev node が別途起動されている前提。webServer で next dev のみ立ち上げる。
export default defineConfig({
  testDir: './e2e',
  // smoldot client は singleton。並列実行で複数ブラウザが同じ light client を奪い合うと flaky になる。
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : 'list',

  // smoldot 初期同期 + Wasm init + extrinsic finalize で 1 分超のテストが普通にあるので長めに。
  timeout: 120_000,
  expect: { timeout: 30_000 },

  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    launchOptions: {
      args: ['--enable-features=SharedArrayBuffer'],
    },
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  webServer: {
    command: 'pnpm dev',
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    // predev で chainspec を再生成 + Next.js dev サーバ初回コンパイルで時間がかかる
    timeout: 180_000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
})

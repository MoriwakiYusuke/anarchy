import { defineConfig, devices } from '@playwright/test'

const PORT = Number(process.env.E2E_PORT ?? 3000)

// Anarchy frontend (Next.js 14 + WebSocket PAPI + anarchy-wasm-engine) 用の Playwright 設定。
// dev node が別途起動されている前提。webServer で next dev のみ立ち上げる。
//
// Phase B 移行: smoldot を撤去し WS provider に統一。SharedArrayBuffer / chainspec
// regenerate が不要になり、起動が軽くなった。
export default defineConfig({
  testDir: './e2e',
  // PAPI client は singleton (chain-client.ts)。同一プロセス内並列で WS 接続を奪い合う
  // 可能性があるので念のため workers=1 を維持。
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : 'list',

  // Wasm init + 複数 extrinsic finalize (PoW 30s blocktime) でテストごと 5 分超もあり得る。
  timeout: 360_000,
  expect: { timeout: 30_000 },

  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
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
    timeout: 180_000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
})

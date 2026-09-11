import { test, expect, type Page } from '@playwright/test'

/**
 * 本番デプロイに対する投稿フローの一気通貫検証。
 *
 * これが通れば以下が全部動いている証明になる:
 *   ブラウザ
 *     → Cloudflare Workers (静的フロント)
 *     → wss://rpc.anarchy2026.org/rpc (nginx + Let's Encrypt)
 *     → gateway のチェーン (GCP)
 *     → Tor Hidden Service
 *     → core のストレージ (さくら) / gateway 自身のストレージ
 *     → core のチェーンで採掘・ファイナライズ
 *
 * 注意: 本番チェーンに実際に extrinsic を投げる。MORAL を消費し投稿が残る。
 */

async function connectDevAccount(page: Page, account: string): Promise<void> {
  await page.locator('aside select').selectOption(`//${account}`)
  await page.locator('aside button:has-text("Connect")').click()
  await expect(
    page.locator('aside').getByText('Connected', { exact: false }).first(),
  ).toBeVisible({ timeout: 60_000 })
}

test.describe('本番デプロイの投稿フロー', () => {
  test('チェーンに接続できる', async ({ page }) => {
    const consoleErrors: string[] = []
    page.on('console', (m) => m.type() === 'error' && consoleErrors.push(m.text()))
    page.on('pageerror', (e) => consoleErrors.push(`pageerror: ${e.message}`))

    await page.goto('/')

    // ヘッダの接続状態。WS handshake + 最初のクエリで数秒。
    await expect(page.getByText('Connected', { exact: false }).first())
      .toBeVisible({ timeout: 90_000 })

    // バンドルが構文エラーで死んでいないことの確認も兼ねる
    // (minifier が @scure/sr25519 を壊すと、ここで pageerror が出る)
    expect(consoleErrors, `コンソールエラー:\n${consoleErrors.join('\n')}`).toEqual([])
  })

  test('投稿してタイムラインに反映される', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByText('Connected', { exact: false }).first())
      .toBeVisible({ timeout: 90_000 })

    await connectDevAccount(page, 'Alice')

    const textarea = page.getByPlaceholder("What's happening?")
    await expect(textarea).toBeVisible({ timeout: 60_000 })

    const body = `e2e prod ${Date.now()}`
    await textarea.fill(body)

    const submitBtn = page.getByRole('button', { name: /^Post$/, exact: true })
    await expect(submitBtn).toBeEnabled({ timeout: 60_000 })
    await submitBtn.click()

    // KZG split → storage upload (Tor 経由) → create_post extrinsic → finalize。
    // Tor の connect が 1.4-3.9s、PoW のブロック間隔が 30s 目標なので長めに取る。
    await expect(page.getByText(/Posted!\s*\(Block #\d+\)/))
      .toBeVisible({ timeout: 300_000 })

    // 投稿本文がタイムラインに出る = ストレージから読み戻せている
    await expect(page.getByText(body)).toBeVisible({ timeout: 120_000 })
  })
})

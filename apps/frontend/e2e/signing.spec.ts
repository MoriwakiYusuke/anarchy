import { test, expect, type Page, type BrowserContext } from '@playwright/test'

/**
 * E2E Tests for WebAuthn Signed Post Flow
 *
 * These tests verify the WYSIWYS (What You See Is What You Sign) flow
 * for creating posts with WebAuthn signatures.
 *
 * Prerequisites:
 * - Local blockchain node running (ws://localhost:9944)
 * - Frontend dev server (started automatically by Playwright)
 * - User must be registered (identity created)
 */

test.describe('WebAuthn Signed Post Flow', () => {
  let authenticatorId: string

  // Setup: Register a user before testing signing
  test.beforeEach(async ({ context, page }: { context: BrowserContext; page: Page }) => {
    // Create CDP session to set up virtual authenticator
    const cdpSession = await context.newCDPSession(page)

    // Enable WebAuthn virtual authenticator
    await cdpSession.send('WebAuthn.enable', {})

    // Add a virtual authenticator
    const result = await cdpSession.send('WebAuthn.addVirtualAuthenticator', {
      options: {
        protocol: 'ctap2',
        transport: 'internal',
        hasResidentKey: true,
        hasUserVerification: true,
        isUserVerified: true,
        automaticPresenceSimulation: true,
      },
    })
    authenticatorId = result.authenticatorId

    // Go to page and register first
    await page.goto('/')

    // Wait for blockchain connection
    await expect(page.getByText('接続済み')).toBeVisible({ timeout: 30000 })

    // Complete registration
    const registerButton = page.getByRole('button', { name: /パスキーで登録/i })
    if (await registerButton.isVisible()) {
      await registerButton.click()

      // Wait for success
      await expect(
        page.getByText(/登録完了|成功|Identity ID/i)
      ).toBeVisible({ timeout: 30000 })
    }
  })

  test('should show post form after registration', async ({ page }) => {
    // Post form should be visible
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    await expect(contentInput).toBeVisible({ timeout: 10000 })

    // Post button should be visible
    const postButton = page.getByRole('button', { name: /署名.*投稿|投稿/i })
    await expect(postButton).toBeVisible()
  })

  test('should display byte count and estimated cost', async ({ page }) => {
    // Find and type in the content input
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    await contentInput.fill('Hello, Anarchy!')

    // Check for byte count display
    await expect(
      page.getByText(/\d+\s*bytes?/i)
    ).toBeVisible()

    // Check for cost display
    await expect(
      page.getByText(/コスト|cost|\$moral/i)
    ).toBeVisible()
  })

  test('should submit post with WebAuthn signature', async ({ page }) => {
    // Type content
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    await contentInput.fill('My first WebAuthn-signed post! 🎉')

    // Click post button
    const postButton = page.getByRole('button', { name: /署名.*投稿|投稿/i })
    await postButton.click()

    // Wait for WebAuthn authentication (automatic with virtual authenticator)
    // Then wait for transaction submission

    // Check for success message
    await expect(
      page.getByText(/投稿.*完了|成功/i)
    ).toBeVisible({ timeout: 30000 })
  })

  test('should show post in timeline after submission', async ({ page }) => {
    const testContent = `Test post ${Date.now()}`

    // Type unique content
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    await contentInput.fill(testContent)

    // Submit
    const postButton = page.getByRole('button', { name: /署名.*投稿|投稿/i })
    await postButton.click()

    // Wait for success
    await expect(
      page.getByText(/投稿.*完了|成功/i)
    ).toBeVisible({ timeout: 30000 })

    // Check that post appears in timeline
    await expect(
      page.getByText(testContent)
    ).toBeVisible({ timeout: 10000 })
  })

  test('should clear form after successful post', async ({ page }) => {
    // Type content
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    await contentInput.fill('Content to be cleared')

    // Submit
    const postButton = page.getByRole('button', { name: /署名.*投稿|投稿/i })
    await postButton.click()

    // Wait for success
    await expect(
      page.getByText(/投稿.*完了|成功/i)
    ).toBeVisible({ timeout: 30000 })

    // Check that input is cleared
    await expect(contentInput).toHaveValue('')
  })

  test('should disable submit button while submitting', async ({ page }) => {
    // Type content
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    await contentInput.fill('Test post')

    // Click post button
    const postButton = page.getByRole('button', { name: /署名.*投稿|投稿/i })
    await postButton.click()

    // Button should be disabled during submission
    await expect(postButton).toBeDisabled()

    // Wait for completion
    await expect(
      page.getByText(/投稿.*完了|成功/i)
    ).toBeVisible({ timeout: 30000 })

    // Button should be enabled again
    await expect(postButton).toBeEnabled()
  })

  test('should show status updates during submission', async ({ page }) => {
    // Type content
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    await contentInput.fill('Status test post')

    // Click post button
    const postButton = page.getByRole('button', { name: /署名.*投稿|投稿/i })
    await postButton.click()

    // Should show signing status
    await expect(
      page.getByText(/署名中|準備中|送信中|確認中/i)
    ).toBeVisible({ timeout: 5000 })

    // Wait for completion
    await expect(
      page.getByText(/投稿.*完了|成功/i)
    ).toBeVisible({ timeout: 30000 })
  })
})

test.describe('Post Validation', () => {
  test.beforeEach(async ({ context, page }: { context: BrowserContext; page: Page }) => {
    const cdpSession = await context.newCDPSession(page)
    await cdpSession.send('WebAuthn.enable', {})
    await cdpSession.send('WebAuthn.addVirtualAuthenticator', {
      options: {
        protocol: 'ctap2',
        transport: 'internal',
        hasResidentKey: true,
        hasUserVerification: true,
        isUserVerified: true,
        automaticPresenceSimulation: true,
      },
    })

    await page.goto('/')
    await expect(page.getByText('接続済み')).toBeVisible({ timeout: 30000 })

    const registerButton = page.getByRole('button', { name: /パスキーで登録/i })
    if (await registerButton.isVisible()) {
      await registerButton.click()
      await expect(
        page.getByText(/登録完了|成功|Identity ID/i)
      ).toBeVisible({ timeout: 30000 })
    }
  })

  test('should disable submit button when content is empty', async ({ page }) => {
    const postButton = page.getByRole('button', { name: /署名.*投稿|投稿/i })

    // Button should be disabled when content is empty
    await expect(postButton).toBeDisabled()
  })

  test('should enable submit button when content is entered', async ({ page }) => {
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    const postButton = page.getByRole('button', { name: /署名.*投稿|投稿/i })

    // Type content
    await contentInput.fill('Test')

    // Button should be enabled
    await expect(postButton).toBeEnabled()
  })

  test('should show character/byte limit warning for long content', async ({ page }) => {
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })

    // Type very long content
    const longContent = 'あ'.repeat(500) // 1500 bytes in UTF-8

    await contentInput.fill(longContent)

    // Should show warning or limit indicator
    await expect(
      page.getByText(/超過|上限|長すぎ|\d+.*bytes/i)
    ).toBeVisible()
  })
})

test.describe('Timeline Integration', () => {
  test.beforeEach(async ({ context, page }: { context: BrowserContext; page: Page }) => {
    const cdpSession = await context.newCDPSession(page)
    await cdpSession.send('WebAuthn.enable', {})
    await cdpSession.send('WebAuthn.addVirtualAuthenticator', {
      options: {
        protocol: 'ctap2',
        transport: 'internal',
        hasResidentKey: true,
        hasUserVerification: true,
        isUserVerified: true,
        automaticPresenceSimulation: true,
      },
    })

    await page.goto('/')
    await expect(page.getByText('接続済み')).toBeVisible({ timeout: 30000 })

    const registerButton = page.getByRole('button', { name: /パスキーで登録/i })
    if (await registerButton.isVisible()) {
      await registerButton.click()
      await expect(
        page.getByText(/登録完了|成功|Identity ID/i)
      ).toBeVisible({ timeout: 30000 })
    }
  })

  test('should show WebAuthn badge on signed posts', async ({ page }) => {
    // Create a post
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    await contentInput.fill('WebAuthn signed post')

    const postButton = page.getByRole('button', { name: /署名.*投稿|投稿/i })
    await postButton.click()

    // Wait for success
    await expect(
      page.getByText(/投稿.*完了|成功/i)
    ).toBeVisible({ timeout: 30000 })

    // Check for WebAuthn indicator/badge in timeline
    // This depends on how the Timeline component displays WebAuthn posts
    await expect(
      page.getByText(/WebAuthn|🔐|署名済み|verified/i)
    ).toBeVisible({ timeout: 10000 })
  })

  test('should display identity-based posts', async ({ page }) => {
    // Create a post
    const testContent = `Identity test ${Date.now()}`
    const contentInput = page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    await contentInput.fill(testContent)

    const postButton = page.getByRole('button', { name: /署名.*投稿|投稿/i })
    await postButton.click()

    // Wait for success
    await expect(
      page.getByText(/投稿.*完了|成功/i)
    ).toBeVisible({ timeout: 30000 })

    // Post should show in timeline with identity info
    const post = page.locator(`text=${testContent}`).first()
    await expect(post).toBeVisible()
  })
})

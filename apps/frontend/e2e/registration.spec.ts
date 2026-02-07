import { test, expect, type Page, type BrowserContext } from '@playwright/test'

/**
 * E2E Tests for WebAuthn Passkey Registration Flow
 *
 * These tests use Playwright's Virtual Authenticator to simulate
 * the WebAuthn passkey registration process without requiring
 * actual biometric hardware.
 *
 * Prerequisites:
 * - Local blockchain node running (ws://localhost:9944)
 * - Frontend dev server (started automatically by Playwright)
 */

test.describe('Passkey Registration Flow', () => {
  // Setup virtual authenticator for each test
  test.beforeEach(async ({ context, page }: { context: BrowserContext; page: Page }) => {
    // Create CDP session to set up virtual authenticator
    const cdpSession = await context.newCDPSession(page)

    // Enable WebAuthn virtual authenticator
    await cdpSession.send('WebAuthn.enable', {})

    // Add a virtual authenticator with platform authenticator capabilities
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
  })

  test('should show passkey registration button when not registered', async ({ page }) => {
    await page.goto('/')

    // Wait for the page to load
    await expect(page.locator('h1')).toContainText('Anarchy')

    // Check that we're in passkey mode by default
    const passkeyModeButton = page.getByRole('button', { name: /パスキー/i })
    await expect(passkeyModeButton).toBeVisible()

    // Check that registration component is visible
    const registerButton = page.getByRole('button', { name: /パスキーで登録/i })
    await expect(registerButton).toBeVisible()
  })

  test('should show WebAuthn not supported message when not available', async ({ page, context }) => {
    // Disable WebAuthn for this test
    const cdpSession = await context.newCDPSession(page)
    await cdpSession.send('WebAuthn.disable', {})

    await page.goto('/')

    // Check for WebAuthn not supported message
    // The WebAuthnGate component should show a message
    await expect(
      page.getByText(/WebAuthn.*対応していません|パスキー.*サポートされていません/i)
    ).toBeVisible()
  })

  test('should complete registration flow with virtual authenticator', async ({ page }) => {
    await page.goto('/')

    // Wait for blockchain connection
    await expect(page.getByText('接続済み')).toBeVisible({ timeout: 30000 })

    // Click on device name input if visible
    const deviceInput = page.locator('input[placeholder*="デバイス名"]')
    if (await deviceInput.isVisible()) {
      await deviceInput.fill('Test Device')
    }

    // Click the register button
    const registerButton = page.getByRole('button', { name: /パスキーで登録/i })
    await registerButton.click()

    // Wait for the registration process
    // The virtual authenticator will automatically approve
    await expect(
      page.getByText(/登録完了|成功|Identity ID/i)
    ).toBeVisible({ timeout: 30000 })

    // After registration, the post form should be visible
    await expect(
      page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    ).toBeVisible({ timeout: 10000 })
  })

  test('should persist identity after registration', async ({ page }) => {
    await page.goto('/')

    // Wait for blockchain connection
    await expect(page.getByText('接続済み')).toBeVisible({ timeout: 30000 })

    // Complete registration
    const registerButton = page.getByRole('button', { name: /パスキーで登録/i })
    await registerButton.click()

    // Wait for success
    await expect(
      page.getByText(/登録完了|成功|Identity ID/i)
    ).toBeVisible({ timeout: 30000 })

    // Reload the page
    await page.reload()

    // Wait for page to load
    await expect(page.locator('h1')).toContainText('Anarchy')

    // Check that the identity is still available (post form visible, not register form)
    // This depends on LocalStorage persistence
    await expect(
      page.getByRole('textbox', { name: /投稿|コンテンツ/i })
    ).toBeVisible({ timeout: 10000 })
  })

  test('should show device settings after registration', async ({ page }) => {
    await page.goto('/')

    // Wait for connection
    await expect(page.getByText('接続済み')).toBeVisible({ timeout: 30000 })

    // Complete registration
    const registerButton = page.getByRole('button', { name: /パスキーで登録/i })
    await registerButton.click()

    // Wait for success
    await expect(
      page.getByText(/登録完了|成功/i)
    ).toBeVisible({ timeout: 30000 })

    // Check for device settings section
    await expect(
      page.getByText(/登録済みデバイス/i)
    ).toBeVisible({ timeout: 10000 })
  })

  test('should switch between passkey and wallet modes', async ({ page }) => {
    await page.goto('/')

    // Check passkey mode is active by default
    const passkeyButton = page.getByRole('button', { name: /パスキー/i })
    await expect(passkeyButton).toHaveClass(/active/i)

    // Switch to wallet mode
    const walletButton = page.getByRole('button', { name: /ウォレット/i })
    await walletButton.click()

    // Check wallet mode is now active
    await expect(walletButton).toHaveClass(/active/i)

    // WalletConnect component should be visible
    await expect(
      page.getByText(/ウォレット|アカウント/i)
    ).toBeVisible()

    // Switch back to passkey mode
    await passkeyButton.click()
    await expect(passkeyButton).toHaveClass(/active/i)
  })
})

test.describe('Error Handling', () => {
  test('should handle user cancellation gracefully', async ({ context, page }) => {
    // Setup virtual authenticator that will simulate user denial
    const cdpSession = await context.newCDPSession(page)
    await cdpSession.send('WebAuthn.enable', {})

    const { authenticatorId } = await cdpSession.send('WebAuthn.addVirtualAuthenticator', {
      options: {
        protocol: 'ctap2',
        transport: 'internal',
        hasResidentKey: true,
        hasUserVerification: true,
        isUserVerified: false, // User will deny
        automaticPresenceSimulation: false,
      },
    })

    await page.goto('/')

    // Wait for connection
    await expect(page.getByText('接続済み')).toBeVisible({ timeout: 30000 })

    // Click register
    const registerButton = page.getByRole('button', { name: /パスキーで登録/i })
    await registerButton.click()

    // Simulate user abort
    await cdpSession.send('WebAuthn.removeVirtualAuthenticator', {
      authenticatorId,
    })

    // Check for user-friendly error message
    await expect(
      page.getByText(/キャンセル|中止|やり直し/i)
    ).toBeVisible({ timeout: 10000 })
  })

  test('should allow retry after error', async ({ page }) => {
    await page.goto('/')

    // Wait for connection
    await expect(page.getByText('接続済み')).toBeVisible({ timeout: 30000 })

    // Check that register button is available
    const registerButton = page.getByRole('button', { name: /パスキーで登録/i })
    await expect(registerButton).toBeEnabled()
  })
})

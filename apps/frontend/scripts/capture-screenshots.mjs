#!/usr/bin/env node
/**
 * Capture README screenshots from a running frontend.
 *
 * Pre-req:
 *   pnpm stack:start         # chain (3-node) + storage (5-node) + frontend
 *
 * Usage:
 *   BASE_URL=http://127.0.0.1:3000 node scripts/capture-screenshots.mjs
 *
 * Output: assets/screenshot-*.png in the repo root.
 *
 * Per .claude/skills/playwright-e2e: Playwright MCP is unsupported on this
 * WSL2 env. The CLI Chromium ships with @playwright/test and works headless.
 * We use the same Dev-account flow as e2e/fixtures/chain.ts to reach a
 * Connected state and capture the real UI.
 */
import { chromium } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const BASE_URL = process.env.BASE_URL || 'http://127.0.0.1:3000';
const __dir = fileURLToPath(new URL('.', import.meta.url));
const REPO_ROOT = resolve(__dir, '../../..');
const OUT_DIR = resolve(REPO_ROOT, 'assets');

async function waitForConnected(page) {
  await page.getByText('Connected', { exact: false }).first().waitFor({
    state: 'visible',
    timeout: 60_000,
  });
}

async function connectAsAlice(page) {
  await page.locator('aside select').selectOption('//Alice');
  await page.locator('aside button:has-text("Connect")').click();
  await page
    .locator('aside')
    .getByText('Connected', { exact: false })
    .first()
    .waitFor({ state: 'visible', timeout: 30_000 });
}

async function main() {
  await mkdir(OUT_DIR, { recursive: true });
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
    colorScheme: 'dark',
  });
  const page = await ctx.newPage();

  // 1. Disconnected landing
  console.log('-> /  (disconnected)');
  await page.goto(BASE_URL + '/', { waitUntil: 'networkidle', timeout: 60_000 }).catch(() => {});
  await page.waitForTimeout(2000); // settle matrix bg + hydration
  await page.screenshot({
    path: resolve(OUT_DIR, 'screenshot-home-disconnected.png'),
    fullPage: false,
  });
  console.log('   saved screenshot-home-disconnected.png');

  // 2. Connect as Alice + connected timeline
  console.log('-> connecting as //Alice');
  await waitForConnected(page); // chain status banner
  await connectAsAlice(page);
  // give the timeline a beat to fetch + render reactions/balance
  await page.waitForTimeout(3000);
  await page.screenshot({
    path: resolve(OUT_DIR, 'screenshot-home.png'),
    fullPage: false,
  });
  console.log('   saved screenshot-home.png (connected)');

  // 3. Stealth page (if it loads)
  console.log('-> /stealth');
  await page.goto(BASE_URL + '/stealth', { waitUntil: 'networkidle', timeout: 60_000 }).catch(() => {});
  await page.waitForTimeout(2500);
  await page.screenshot({
    path: resolve(OUT_DIR, 'screenshot-stealth.png'),
    fullPage: false,
  });
  console.log('   saved screenshot-stealth.png');

  await browser.close();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});

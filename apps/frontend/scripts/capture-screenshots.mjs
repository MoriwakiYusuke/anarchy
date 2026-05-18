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
 *
 * The script also forces locale=en via localStorage before page load so the
 * screenshots are uniformly English (the /stealth page has Japanese strings
 * embedded for some labels otherwise).
 *
 * To keep the timeline portfolio-clean, the script posts a few realistic
 * English messages as Alice so they show on top of any older debug content.
 */
import { chromium } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const BASE_URL = process.env.BASE_URL || 'http://127.0.0.1:3000';
const __dir = fileURLToPath(new URL('.', import.meta.url));
const REPO_ROOT = resolve(__dir, '../../..');
const OUT_DIR = resolve(REPO_ROOT, 'assets');

const SHOWCASE_POSTS = [
  'Just synced from genesis over Tor. No IP metadata leaked, validator stays anonymous end-to-end.',
  'Storage node pinned 5 GB of fragments. KZG-VSS proofs verified, MORAL rewards landing each epoch.',
  'Reaction mining is foreground-only — Page Visibility API stops the worker the moment you tab away.',
  'Seed phrase lives in session memory and clears on tab close. Your keys, your terminal.',
];

async function setLocaleEn(ctx) {
  // Inject into both localStorage and as initial-script so it survives reloads
  await ctx.addInitScript(() => {
    try {
      localStorage.setItem('anarchy-locale', 'en');
    } catch {}
  });
}

async function waitForGlobalConnected(page) {
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

async function postOnce(page, body) {
  const textarea = page.getByPlaceholder("What's happening?");
  await textarea.waitFor({ state: 'visible', timeout: 30_000 });
  await textarea.fill(body);

  const submitBtn = page.getByRole('button', { name: /^Post$/, exact: true });
  // Wait until usePostCost finishes loading and the button becomes enabled
  await submitBtn.waitFor({ state: 'visible', timeout: 30_000 });
  for (let i = 0; i < 60; i++) {
    if (await submitBtn.isEnabled().catch(() => false)) break;
    await page.waitForTimeout(500);
  }
  await submitBtn.click();

  // create_post finalize → "Posted! (Block #N)". PoW dev = 30s blocktime.
  await page.getByText(/Posted!\s*\(Block #\d+\)/).waitFor({
    state: 'visible',
    timeout: 180_000,
  });
}

async function main() {
  await mkdir(OUT_DIR, { recursive: true });
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
    colorScheme: 'dark',
  });
  await setLocaleEn(ctx);
  const page = await ctx.newPage();

  // 1. Disconnected landing
  console.log('-> /  (disconnected, en locale)');
  await page.goto(BASE_URL + '/', { waitUntil: 'networkidle', timeout: 60_000 }).catch(() => {});
  await waitForGlobalConnected(page); // chain ready banner
  await page.waitForTimeout(1500);
  await page.screenshot({
    path: resolve(OUT_DIR, 'screenshot-home-disconnected.png'),
    fullPage: false,
  });
  console.log('   saved screenshot-home-disconnected.png');

  // 2. Connect Alice and post realistic showcase content
  console.log('-> connecting as //Alice');
  await connectAsAlice(page);
  await page.waitForTimeout(2000);

  if (process.env.SKIP_POSTS !== '1') {
    for (const body of SHOWCASE_POSTS) {
      console.log(`-> posting: ${body.slice(0, 50)}...`);
      await postOnce(page, body);
      // settle UI / let success banner fade so next post can re-enter clean
      await page.waitForTimeout(1500);
    }
  }

  // Scroll to top so freshly-posted content is at the visible viewport
  await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'instant' }));
  await page.waitForTimeout(1500);
  await page.screenshot({
    path: resolve(OUT_DIR, 'screenshot-home.png'),
    fullPage: false,
  });
  console.log('   saved screenshot-home.png (connected, fresh timeline)');

  // 3. Stealth page in English
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

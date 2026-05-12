#!/usr/bin/env node
/**
 * Capture README screenshots from a running frontend.
 * Usage: BASE_URL=http://127.0.0.1:3000 node scripts/capture-screenshots.mjs
 */
import { chromium } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const BASE_URL = process.env.BASE_URL || 'http://127.0.0.1:3000';
const __dir = fileURLToPath(new URL('.', import.meta.url));
const REPO_ROOT = resolve(__dir, '../../..');
const OUT_DIR = resolve(REPO_ROOT, 'assets');

const SHOTS = [
  { name: 'screenshot-home.png', path: '/' },
];

async function main() {
  await mkdir(OUT_DIR, { recursive: true });
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
    colorScheme: 'dark',
  });
  const page = await ctx.newPage();
  for (const { name, path } of SHOTS) {
    const url = BASE_URL + path;
    console.log(`-> ${url}`);
    await page.goto(url, { waitUntil: 'networkidle', timeout: 60_000 }).catch(() => {});
    await page.waitForTimeout(2000); // settle matrix bg / hydration
    const out = resolve(OUT_DIR, name);
    await page.screenshot({ path: out, fullPage: false });
    console.log(`   saved ${out}`);
  }
  await browser.close();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});

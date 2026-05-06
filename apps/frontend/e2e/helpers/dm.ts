import { expect, type Page } from '@playwright/test';

/**
 * DM E2E ヘルパ。
 *
 * 一連の操作 (鍵生成 → publish → スレッド開く → 送信 → 受信検証) を関数化して、
 * spec ファイルでは "what" だけを書くようにする。各関数は失敗時に
 * 明確なメッセージを出すため、内部で `expect` を呼ぶ。
 */

const ALICE_SS58 = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY';
const BOB_SS58 = '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty';

export const TEST_ADDRESSES = { ALICE: ALICE_SS58, BOB: BOB_SS58 };

/** Wallet サイドバーの "Messages →" を押して DM modal を開く。 */
export async function openDmModal(page: Page): Promise<void> {
  await page.locator('aside button:has-text("Messages")').click();
  await expect(page.getByRole('dialog', { name: /Direct Messages/i })).toBeVisible();
}

/** 設定タブで stealth 鍵を生成する。既存の鍵があってもボタンは押せる前提。 */
export async function generateStealthKey(page: Page): Promise<void> {
  // "DM key not loaded" notice → "Open DM key settings" 経由で settings タブへ。
  const openSettings = page.getByRole('dialog').getByRole('button', { name: /Open DM key settings/i });
  if (await openSettings.isVisible()) {
    await openSettings.click();
  } else {
    await page.getByRole('dialog').getByRole('tab', { name: /Settings/i }).click();
  }
  await page.getByRole('dialog').getByRole('button', { name: /Generate a new key/i }).click();
  // 生成完了は keyManager region が表示されることで判定する。
  await expect(page.getByRole('dialog').getByRole('region', { name: /DM key manager/i })).toBeVisible({
    timeout: 30_000,
  });
}

/**
 * チェーンに DM 鍵を publish する。
 *
 * "Publish" ボタンが有効なら押す。すでに current key が published 済みなら
 * 何もしない。stale (チェーン上に古い key) の場合は "Republish" として有効化
 * されているので同じボタンを押す (実装は publish_dm_key の上書き)。
 */
export async function publishOrRepublishDmKey(page: Page): Promise<void> {
  const publish = page.getByRole('dialog').getByRole('button', { name: /^(Publish|Republish)$/ });
  if (await publish.isEnabled().catch(() => false)) {
    await publish.click();
    await expect(
      page.getByRole('dialog').getByText(/^Status:\s*Published$/),
    ).toBeVisible({ timeout: 60_000 });
  } else {
    // Already published with current key → no action.
    await expect(
      page.getByRole('dialog').getByText(/^Status:\s*Published$/),
    ).toBeVisible({ timeout: 30_000 });
  }
}

/** Inbox タブを開いて、指定の SS58 宛にスレッドを作る。 */
export async function openThread(page: Page, recipient: string): Promise<void> {
  await page.getByRole('dialog').getByRole('tab', { name: /Inbox/i }).click();
  const input = page.getByRole('dialog').getByPlaceholder(/Recipient SS58/i);
  await input.fill(recipient);
  await page.getByRole('dialog').getByRole('button', { name: /^Open$/ }).click();
  await expect(
    page.getByRole('dialog').getByRole('region', { name: new RegExp(`Counterparty: ${recipient}`) }),
  ).toBeVisible();
}

/** 開いているスレッドに本文を入れて Send。送信完了は "Sent" status を待つ。 */
export async function sendDmText(page: Page, body: string): Promise<void> {
  const composer = page.getByRole('dialog').getByRole('region', { name: 'DM compose form' });
  await composer.getByRole('textbox', { name: 'dm message body' }).fill(body);
  await composer.getByRole('button', { name: /^Send$/ }).click();
  // PoW 後の DM 送信は 2 extrinsic finalize: stealth pre-funding (transfer) + DM dispatch
  // 30s/block × 2 + ブレで 3 分くらい余裕で要する。Aura 6s 時代は 90s で足りていたが、
  // PoW 移行後は 240s 以上見込むのが現実的。
  await expect(composer.getByRole('status')).toHaveText(/Sent/i, { timeout: 240_000 });
}

/** 開いているスレッドの outgoing バブルに本文が見えることを assert。 */
export async function expectOutgoingBubble(page: Page, body: string): Promise<void> {
  const region = page.getByRole('dialog').getByRole('region', { name: /^Counterparty:/ });
  await expect(region.getByText(body, { exact: true }).first()).toBeVisible();
}

/**
 * 開いているスレッドに incoming バブル (= scanner が pickup) が出ることを assert。
 * scanner foreground interval は 15s なので最大 90s 待つ。
 *
 * @param body 期待されるメッセージ本文
 * @param minCount  期待最少 listitem 数。
 *   - self-DM: 2 (outgoing + incoming で同じ本文が 2 つ並ぶ)
 *   - multi-user (相手送信): 1 (incoming のみ)
 */
export async function expectIncomingBubble(
  page: Page,
  body: string,
  minCount = 2,
): Promise<void> {
  const region = page.getByRole('dialog').getByRole('region', { name: /^Counterparty:/ });
  await expect(async () => {
    const items = region.getByRole('listitem').getByText(body, { exact: true });
    expect(await items.count()).toBeGreaterThanOrEqual(minCount);
  }).toPass({ timeout: 180_000, intervals: [3_000, 5_000, 10_000] });
}

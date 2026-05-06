import { test, expect } from './fixtures/chain';

/**
 * PoW 移行後 (Phase B) の chain sync 回帰確認 E2E。
 *
 * ⚠️ **STATUS: SKIPPED (Phase B の既知ブロッカー)**
 *
 * Phase B mainnet smoke 中に判明した既知の互換性問題 — smoldot light client が
 * PoW chain の block announcement を decode できない:
 *
 *   network protocol-error; error=BadBlockAnnounce(DecodeBlockAnnounceError(Verify))
 *
 * smoldot は内部の consensus enum で `Babe`, `Aura`, `AllAuthorized` (PoA) のみを
 * サポートしており、PoW (sc_consensus_pow + RandomX) で生成されたブロックヘッダの
 * digest 構造を verify できない。結果として:
 *   - smoldot は genesis から進めず "Connecting..." のまま固まる
 *   - frontend は chain と通信できず Wallet panel の Connect ボタンが disabled
 *   - 既存 E2E (post-create.spec / dm-* / faucet 等) も全て同根本原因で失敗する
 *
 * **影響範囲**: frontend は production 投入できない。chain 単独 (RPC 経由) は健全。
 *
 * **対応案 (Phase C)**:
 *   1. frontend を smoldot から `getWsProvider` (WebSocket 経由 full client RPC) に
 *      切替える → 軽量だが Anarchy の anonymity 原則 (Tor/I2P 経由 P2P) が後退
 *   2. smoldot upstream に PoW consensus 対応を提案 / fork
 *   3. 独自 light client を Wasm で実装 (重)
 *
 * 暫定的な production 投入路: chain は PoW で稼働、frontend は WebSocket fallback で
 * ws://chain-node:9944 に接続 (Tor hidden service 経由でアクセスする運用)
 *
 * 本 spec は Phase C で smoldot 互換性が解決した時点で `.skip` を外して有効化する。
 *
 * 前提: 3-node testnet (`pnpm testnet:start`) + storage nodes (`pnpm storage:start`)
 */

test.describe.skip('PoW chain sync (BLOCKED on smoldot+PoW compat — see header)', () => {
  test('smoldot connects to PoW chain and best block advances', async ({ page }) => {
    await page.goto('/');

    // 1. smoldot 接続成功 (Aura → PoW 切替後も初期 sync が回ること)
    //    chain.ts fixture の chainReady と同じ判定だが、明示的に独立検証する。
    await expect(page.getByText('Connected', { exact: false })).toBeVisible({ timeout: 60_000 });

    // 2. best block 番号を取得 (UI のヘッダー / status バー)。
    //    Anarchy の HomeLayout は ChainStatus に "Block: #N" 形式で出している想定。
    //    実装詳細は frontend に依存するので、文字列正規表現でゆるく拾う。
    const blockBefore = await readBestBlockNumber(page);
    test.info().annotations.push({ type: 'info', description: `Initial best block: #${blockBefore}` });

    // 3. 30 秒 + 余裕 (45 秒) 待って block height が前進していることを確認。
    //    PoW target 30s なので最低 1 ブロックは入るはず。
    //    LWMA-3 のブレで 1 ブロックも来ないケースに備えて poll で 45 秒粘る。
    await expect
      .poll(
        async () => readBestBlockNumber(page),
        {
          timeout: 90_000,
          intervals: [3_000, 5_000, 10_000],
          message: `block did not advance from #${blockBefore} within 90s`,
        },
      )
      .toBeGreaterThan(blockBefore);

    const blockAfter = await readBestBlockNumber(page);
    test.info().annotations.push({ type: 'info', description: `Final best block: #${blockAfter}` });
    expect(blockAfter).toBeGreaterThan(blockBefore);
  });

  test('Wallet connect → PoW chain で extrinsic 関連の RPC が叩ける', async ({
    page,
    connectDevAccount,
  }) => {
    await connectDevAccount('Alice');

    // PostForm が表示される = Wallet 経由で chain RPC (system_account / runtime API) が
    // 全部応答している証拠。Aura 関連 RuntimeApi を runtime から削除した後も壊れていないか。
    const textarea = page.getByPlaceholder("What's happening?");
    await expect(textarea).toBeVisible({ timeout: 30_000 });

    // 残高表示 (BalanceDisplay) もマウントされていること = system_account が叩けた
    // (PoW migration で frame_system::Config を触っていないが念のため)
    const walletPanel = page.locator('aside');
    await expect(walletPanel.getByText(/MORAL/i).first()).toBeVisible({ timeout: 30_000 });
  });
});

/**
 * 画面上の "best block #N" 表記から数値を抽出する。
 *
 * フロントの ChainStatus / Header どこに居ても拾えるよう、ページ全体テキストを
 * 走査して最大の "#数字" を best block と見做す。
 */
async function readBestBlockNumber(page: import('@playwright/test').Page): Promise<number> {
  return page.evaluate(() => {
    const text = document.body.innerText;
    const matches = text.matchAll(/#(\d{1,8})\b/g);
    let max = 0;
    for (const m of matches) {
      const n = Number(m[1]);
      if (Number.isFinite(n) && n > max) max = n;
    }
    return max;
  });
}
